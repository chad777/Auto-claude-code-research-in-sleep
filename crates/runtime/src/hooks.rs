use std::collections::BTreeSet;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::json;

use crate::config::{RuntimeFeatureConfig, RuntimeHookConfig, RuntimeHookSpec};

/// v0.4.17 Phase 2 (TIMEOUT-1): default per-hook timeout in seconds, used
/// when a hook spec carries no `timeout` field.
const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 30;
/// Clamp bounds for the per-hook `timeout` field (seconds).
const HOOK_TIMEOUT_MIN_SECS: u64 = 1;
const HOOK_TIMEOUT_MAX_SECS: u64 = 600;
/// Max characters of the hook command echoed into the timeout warning.
const HOOK_COMMAND_DISPLAY_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
}

impl HookEvent {
    fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunResult {
    denied: bool,
    messages: Vec<String>,
}

impl HookRunResult {
    #[must_use]
    pub fn allow(messages: Vec<String>) -> Self {
        Self {
            denied: false,
            messages,
        }
    }

    #[must_use]
    pub fn is_denied(&self) -> bool {
        self.denied
    }

    #[must_use]
    pub fn messages(&self) -> &[String] {
        &self.messages
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookRunner {
    config: RuntimeHookConfig,
}

impl HookRunner {
    #[must_use]
    pub fn new(config: RuntimeHookConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn from_feature_config(feature_config: &RuntimeFeatureConfig) -> Self {
        Self::new(feature_config.hooks().clone())
    }

    #[must_use]
    pub fn run_pre_tool_use(&self, tool_name: &str, tool_input: &str) -> HookRunResult {
        self.run_commands(
            HookEvent::PreToolUse,
            self.config.pre_tool_use(),
            tool_name,
            tool_input,
            None,
            false,
        )
    }

    #[must_use]
    pub fn run_post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &str,
        tool_output: &str,
        is_error: bool,
    ) -> HookRunResult {
        self.run_commands(
            HookEvent::PostToolUse,
            self.config.post_tool_use(),
            tool_name,
            tool_input,
            Some(tool_output),
            is_error,
        )
    }

    fn run_commands(
        &self,
        event: HookEvent,
        specs: &[RuntimeHookSpec],
        tool_name: &str,
        tool_input: &str,
        tool_output: Option<&str>,
        is_error: bool,
    ) -> HookRunResult {
        if specs.is_empty() {
            return HookRunResult::allow(Vec::new());
        }

        let payload = json!({
            "hook_event_name": event.as_str(),
            "tool_name": tool_name,
            "tool_input": parse_tool_input(tool_input),
            "tool_input_json": tool_input,
            "tool_output": tool_output,
            "tool_result_is_error": is_error,
        })
        .to_string();

        let mut messages = Vec::new();

        for spec in specs {
            // v0.4.17 Phase 2 (SCHEMA-3): matcher filtering. A spec whose
            // matcher does not cover this tool name is skipped silently.
            if !spec_matches_tool(spec.matcher.as_deref(), tool_name) {
                continue;
            }
            match self.run_command(
                spec,
                event,
                tool_name,
                tool_input,
                tool_output,
                is_error,
                &payload,
            ) {
                HookCommandOutcome::Allow { message } => {
                    if let Some(message) = message {
                        messages.push(message);
                    }
                }
                HookCommandOutcome::Deny { message } => {
                    let message = message.unwrap_or_else(|| {
                        format!("{} hook denied tool `{tool_name}`", event.as_str())
                    });
                    messages.push(message);
                    return HookRunResult {
                        denied: true,
                        messages,
                    };
                }
                HookCommandOutcome::Warn { message } => messages.push(message),
            }
        }

        HookRunResult::allow(messages)
    }

    /// Run one hook command synchronously with a per-hook timeout.
    ///
    /// v0.4.17 Phase 2 (TIMEOUT-1/2): the hook child process is driven by a
    /// throwaway `current_thread` tokio runtime + `block_on` — the same
    /// sync→async bridge `bash.rs:103` has used in production since v0.4.x.
    /// SPIKE-A verified this call stack (`run_turn` → hook dispatch) never
    /// sits inside an existing tokio runtime, so the nested-runtime panic
    /// cannot trigger. tokio was chosen over a std `try_wait` polling loop
    /// because (a) stdin write + stdout/stderr drain + wait can run
    /// concurrently inside one `tokio::time::timeout` (a std implementation
    /// would need three helper threads to avoid pipe-full deadlocks), and
    /// (b) `Child::kill().await` both signals AND reaps the child (v0.4.10
    /// M3 pattern, no zombies / orphans).
    ///
    /// Timeout semantics: Warn, not Deny (plan D4) — aligned with the
    /// existing non-0/2 exit handling. A user who wants a blocking hook
    /// must make it `exit 2` within its timeout.
    fn run_command(
        &self,
        spec: &RuntimeHookSpec,
        event: HookEvent,
        tool_name: &str,
        tool_input: &str,
        tool_output: Option<&str>,
        is_error: bool,
        payload: &str,
    ) -> HookCommandOutcome {
        let command = spec.command.as_str();
        let timeout_secs = effective_hook_timeout_secs(spec.timeout_secs);

        let mut std_command = shell_command(command);
        std_command.stdin(std::process::Stdio::piped());
        std_command.stdout(std::process::Stdio::piped());
        std_command.stderr(std::process::Stdio::piped());
        std_command.env("HOOK_EVENT", event.as_str());
        std_command.env("HOOK_TOOL_NAME", tool_name);
        std_command.env("HOOK_TOOL_INPUT", tool_input);
        std_command.env("HOOK_TOOL_IS_ERROR", if is_error { "1" } else { "0" });
        if let Some(tool_output) = tool_output {
            std_command.env("HOOK_TOOL_OUTPUT", tool_output);
        }

        let failed_to_start = |error: &dyn std::fmt::Display| HookCommandOutcome::Warn {
            message: format!(
                "{} hook `{command}` failed to start for `{tool_name}`: {error}",
                event.as_str()
            ),
        };

        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => return failed_to_start(&error),
        };

        runtime.block_on(async {
            let mut tokio_command = tokio::process::Command::from(std_command);
            // Belt-and-braces: if this future is ever dropped without the
            // explicit kill below, the child is still killed on drop.
            tokio_command.kill_on_drop(true);
            let mut child = match tokio_command.spawn() {
                Ok(child) => child,
                Err(error) => return failed_to_start(&error),
            };

            let bounded = tokio::time::timeout(
                Duration::from_secs(timeout_secs),
                wait_with_collected_output(&mut child, payload.as_bytes()),
            )
            .await;

            match bounded {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let message = (!stdout.is_empty()).then_some(stdout);
                    match output.status.code() {
                        Some(0) => HookCommandOutcome::Allow { message },
                        Some(2) => HookCommandOutcome::Deny { message },
                        Some(code) => HookCommandOutcome::Warn {
                            message: format_hook_warning(
                                command,
                                code,
                                message.as_deref(),
                                stderr.as_str(),
                            ),
                        },
                        None => HookCommandOutcome::Warn {
                            message: format!(
                                "{} hook `{command}` terminated by signal while handling `{tool_name}`",
                                event.as_str()
                            ),
                        },
                    }
                }
                // Pre-Phase-2 `output_with_stdin` surfaced any IO error
                // (spawn, stdin write, output collection) through this same
                // message — preserved verbatim.
                Ok(Err(error)) => failed_to_start(&error),
                Err(_elapsed) => {
                    // TIMEOUT-2: kill().await signals AND reaps the direct
                    // child (v0.4.10 M3 pattern). Grandchildren the hook
                    // script forked/backgrounded itself are NOT killed — we
                    // do not promise process-tree teardown (codex R14).
                    let _ = child.kill().await;
                    let message = format!(
                        "{} hook `{}` timed out after {timeout_secs}s while handling `{tool_name}`; hook killed, allowing tool execution to continue",
                        event.as_str(),
                        truncate_for_display(command),
                    );
                    eprintln!("warning: {message}");
                    HookCommandOutcome::Warn { message }
                }
            }
        })
    }
}

/// Resolve the effective per-hook timeout: `None` means the 30s default; an
/// explicit value is clamped to 1..=600 seconds (TIMEOUT-1).
fn effective_hook_timeout_secs(spec_timeout: Option<u64>) -> u64 {
    match spec_timeout {
        None => DEFAULT_HOOK_TIMEOUT_SECS,
        Some(secs) => secs.clamp(HOOK_TIMEOUT_MIN_SECS, HOOK_TIMEOUT_MAX_SECS),
    }
}

/// SCHEMA-3 matcher semantics (mirrors Claude Code): the matcher is a regex
/// evaluated against the FULL tool name — `"Bash"` matches only the Bash
/// tool (not `MultiBash`), `"Edit|Write"` matches either, `"Notebook.*"`
/// matches any Notebook tool. We anchor the user pattern as `^(?:pat)$` and
/// use `Regex::is_match` to get that full-name semantics. `None` and `""`
/// both match every tool (string-style hooks and `aris init`'s
/// `matcher: ""` keep firing unconditionally — the pre-Phase-2 behavior).
///
/// A pattern that fails to compile is reported on stderr ONCE per pattern
/// per process and then falls back to a literal whole-string comparison, so
/// a typo never silently disables (or silently broadens) a user hook.
fn spec_matches_tool(matcher: Option<&str>, tool_name: &str) -> bool {
    let Some(pattern) = matcher else {
        return true;
    };
    if pattern.is_empty() {
        return true;
    }
    match regex::Regex::new(&format!("^(?:{pattern})$")) {
        Ok(re) => re.is_match(tool_name),
        Err(error) => {
            warn_invalid_matcher_once(pattern, &error);
            pattern == tool_name
        }
    }
}

/// Warn about an uncompilable matcher regex at most once per pattern for
/// the lifetime of the process.
fn warn_invalid_matcher_once(pattern: &str, error: &regex::Error) {
    static WARNED_PATTERNS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    let warned = WARNED_PATTERNS.get_or_init(|| Mutex::new(BTreeSet::new()));
    let mut warned = match warned.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if warned.insert(pattern.to_string()) {
        eprintln!(
            "warning: hook matcher `{pattern}` is not a valid regex ({error}); falling back to literal exact match"
        );
    }
}

/// Truncate a hook command for display in warning messages (char-boundary
/// safe; commands can embed multi-byte text).
fn truncate_for_display(command: &str) -> String {
    if command.chars().count() <= HOOK_COMMAND_DISPLAY_MAX_CHARS {
        return command.to_string();
    }
    let mut truncated: String = command.chars().take(HOOK_COMMAND_DISPLAY_MAX_CHARS).collect();
    truncated.push('…');
    truncated
}

/// Async equivalent of the old `output_with_stdin`: write the JSON payload
/// to the hook's stdin (closing it afterwards so `cat`-style hooks see
/// EOF), then drain stdout/stderr concurrently while waiting for exit.
/// Concurrency matters: a hook that emits more than a pipe buffer of output
/// before reading stdin would deadlock a sequential write-then-read
/// implementation.
async fn wait_with_collected_output(
    child: &mut tokio::process::Child,
    payload: &[u8],
) -> std::io::Result<std::process::Output> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let stdin = child.stdin.take();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let stdin_fut = async move {
        if let Some(mut stdin) = stdin {
            stdin.write_all(payload).await?;
            // `stdin` drops here (success or error), closing the write end
            // so the child sees EOF.
        }
        Ok::<(), std::io::Error>(())
    };
    let stdout_fut = async {
        let mut buffer = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            pipe.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    };
    let stderr_fut = async {
        let mut buffer = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            pipe.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    };

    let (stdin_result, stdout_result, stderr_result, status) =
        tokio::join!(stdin_fut, stdout_fut, stderr_fut, child.wait());
    let status = status?;
    stdin_result?;
    Ok(std::process::Output {
        status,
        stdout: stdout_result?,
        stderr: stderr_result?,
    })
}

enum HookCommandOutcome {
    Allow { message: Option<String> },
    Deny { message: Option<String> },
    Warn { message: String },
}

fn parse_tool_input(tool_input: &str) -> serde_json::Value {
    serde_json::from_str(tool_input).unwrap_or_else(|_| json!({ "raw": tool_input }))
}

fn format_hook_warning(command: &str, code: i32, stdout: Option<&str>, stderr: &str) -> String {
    let mut message =
        format!("Hook `{command}` exited with status {code}; allowing tool execution to continue");
    if let Some(stdout) = stdout.filter(|stdout| !stdout.is_empty()) {
        message.push_str(": ");
        message.push_str(stdout);
    } else if !stderr.is_empty() {
        message.push_str(": ");
        message.push_str(stderr);
    }
    message
}

/// Build the platform shell invocation for a hook command. Returned as a
/// `std::process::Command` and converted to `tokio::process::Command` at
/// the spawn site (`From` impl), so the platform-specific shell selection
/// stays in one place.
fn shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut command_builder = Command::new("cmd");
        command_builder.arg("/C").arg(command);
        command_builder
    }

    #[cfg(not(windows))]
    {
        let mut command_builder = Command::new("sh");
        command_builder.arg("-lc").arg(command);
        command_builder
    }
}

#[cfg(test)]
mod tests {
    use super::{effective_hook_timeout_secs, spec_matches_tool, HookRunResult, HookRunner};
    use crate::config::{RuntimeFeatureConfig, RuntimeHookConfig, RuntimeHookSpec};

    /// Spec for a bare command (string-style upgrade): no matcher, no
    /// timeout — fires for every tool with the 30s default timeout.
    fn spec(script: &str) -> RuntimeHookSpec {
        RuntimeHookSpec::from_command(shell_snippet(script))
    }

    fn spec_with(
        script: &str,
        matcher: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> RuntimeHookSpec {
        RuntimeHookSpec {
            matcher: matcher.map(str::to_string),
            command: shell_snippet(script),
            timeout_secs,
            async_flag: None,
        }
    }

    #[test]
    fn allows_exit_code_zero_and_captures_stdout() {
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec("printf 'pre ok'")],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("Read", r#"{"path":"README.md"}"#);

        assert_eq!(result, HookRunResult::allow(vec!["pre ok".to_string()]));
    }

    #[test]
    fn denies_exit_code_two() {
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec("printf 'blocked by hook'; exit 2")],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("Bash", r#"{"command":"pwd"}"#);

        assert!(result.is_denied());
        assert_eq!(result.messages(), &["blocked by hook".to_string()]);
    }

    #[test]
    fn warns_for_other_non_zero_statuses() {
        let runner = HookRunner::from_feature_config(&RuntimeFeatureConfig::default().with_hooks(
            RuntimeHookConfig::new(vec![spec("printf 'warning hook'; exit 1")], Vec::new()),
        ));

        let result = runner.run_pre_tool_use("Edit", r#"{"file":"src/lib.rs"}"#);

        assert!(!result.is_denied());
        assert!(result
            .messages()
            .iter()
            .any(|message| message.contains("allowing tool execution to continue")));
    }

    // ---------------------------------------------------------------
    // v0.4.17 Phase 0 — CHARACTERIZATION TESTS (hooks execution semantics)
    //
    // Phase 2 (SCHEMA-3 / TIMEOUT-1/2) landed matcher filtering and the
    // per-hook timeout, deliberately flipping the matcher and slow-hook
    // tests below (each flip is annotated). The Warn-continue gap test is
    // unchanged in meaning — only the spec constructor is new.
    // ---------------------------------------------------------------

    /// deliberately flipped in v0.4.17 Phase 2: matcher filtering added.
    /// Unchanged half: a spec WITHOUT a matcher (string-style upgrade)
    /// still fires for every tool name. Flipped half: a spec WITH a
    /// matcher now only fires when the regex covers the tool name (the
    /// pre-Phase-2 runner ignored the matcher and fired unconditionally).
    #[test]
    fn char_pre_tool_use_fires_for_any_tool_name_no_matcher_filter() {
        // No matcher → fires even for a never-configured tool name.
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec("printf 'fired'")],
            Vec::new(),
        ));
        let result = runner.run_pre_tool_use("SomeNeverConfiguredToolXYZ", r#"{"k":"v"}"#);
        assert_eq!(result, HookRunResult::allow(vec!["fired".to_string()]));

        // Matcher "Bash" → fires for Bash, skipped for Read.
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec_with("printf 'fired'", Some("Bash"), None)],
            Vec::new(),
        ));
        let skipped = runner.run_pre_tool_use("Read", r#"{"path":"x"}"#);
        assert_eq!(
            skipped,
            HookRunResult::allow(Vec::new()),
            "matcher `Bash` must skip tool `Read`"
        );
        let fired = runner.run_pre_tool_use("Bash", r#"{"command":"pwd"}"#);
        assert_eq!(fired, HookRunResult::allow(vec!["fired".to_string()]));
    }

    /// deliberately flipped in v0.4.17 Phase 2: matcher filtering added
    /// (`PostToolUse` side). Matcher-less hooks still fire unconditionally;
    /// an `Edit|Write` alternation fires for Write but no longer for Read.
    #[test]
    fn char_post_tool_use_fires_for_any_tool_name_no_matcher_filter() {
        // No matcher → unconditional firing (unchanged).
        let runner = HookRunner::new(RuntimeHookConfig::new(
            Vec::new(),
            vec![spec("printf 'post-fired'")],
        ));
        let result = runner.run_post_tool_use("ZZZ_unknown_tool", r#"{"k":"v"}"#, "out", false);
        assert_eq!(result, HookRunResult::allow(vec!["post-fired".to_string()]));

        // Matcher "Edit|Write" → fires for Write, skipped for Read.
        let runner = HookRunner::new(RuntimeHookConfig::new(
            Vec::new(),
            vec![spec_with("printf 'post-fired'", Some("Edit|Write"), None)],
        ));
        let skipped = runner.run_post_tool_use("Read", r#"{"k":"v"}"#, "out", false);
        assert_eq!(
            skipped,
            HookRunResult::allow(Vec::new()),
            "matcher `Edit|Write` must skip tool `Read`"
        );
        let fired = runner.run_post_tool_use("Write", r#"{"k":"v"}"#, "out", false);
        assert_eq!(fired, HookRunResult::allow(vec!["post-fired".to_string()]));
    }

    /// Gap test for the non-0/2 Warn-continue contract (hooks.rs:179): a
    /// hook exiting with code 1 produces a Warn (not a Deny) AND does not
    /// prevent a subsequent hook from running and contributing its message.
    #[test]
    fn char_non_zero_non_two_exit_warns_and_continues_to_next_hook() {
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![
                spec("printf 'warn-hook'; exit 1"),
                spec("printf 'ran-after-warn'"),
            ],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("Read", r#"{"path":"x"}"#);

        assert!(!result.is_denied(), "exit 1 must NOT deny");
        // The warning from the first hook is present...
        assert!(
            result
                .messages()
                .iter()
                .any(|m| m.contains("allowing tool execution to continue")),
            "warning message missing: {:?}",
            result.messages()
        );
        // ...and the SECOND hook still ran (Warn does not short-circuit).
        assert!(
            result.messages().iter().any(|m| m == "ran-after-warn"),
            "second hook should still run after a Warn, got {:?}",
            result.messages()
        );
    }

    /// deliberately flipped in v0.4.17 Phase 2: timeout enforced. A hook
    /// that exceeds its per-hook timeout is killed (Warn, not Deny) instead
    /// of being awaited forever, and execution continues with the next
    /// hook. Unix-only: relies on `sleep`.
    #[cfg(not(windows))]
    #[test]
    fn char_slow_hook_is_awaited_with_no_timeout() {
        use std::time::Instant;

        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![
                spec_with("sleep 5; printf 'never-emitted'", None, Some(1)),
                spec("printf 'ran-after-timeout'"),
            ],
            Vec::new(),
        ));

        let start = Instant::now();
        let result = runner.run_pre_tool_use("Bash", r#"{"command":"true"}"#);
        let elapsed = start.elapsed();

        // Killed at ~1s — far below the 5s the hook would have slept.
        assert!(
            elapsed.as_millis() < 4000,
            "timed-out hook must be killed well before its 5s sleep; took {}ms",
            elapsed.as_millis()
        );
        assert!(
            elapsed.as_millis() >= 900,
            "timeout should still wait ~1s before killing; took {}ms",
            elapsed.as_millis()
        );
        assert!(!result.is_denied(), "timeout is Warn, not Deny");
        assert!(
            result
                .messages()
                .iter()
                .any(|m| m.contains("timed out after 1s")
                    && m.contains("allowing tool execution to continue")),
            "timeout warning missing: {:?}",
            result.messages()
        );
        // The killed hook's stdout never surfaces as a captured message
        // (an Allow/stdout message is pushed verbatim; the only place
        // "never-emitted" may appear is echoed inside the warning text).
        assert!(
            !result.messages().iter().any(|m| m == "never-emitted"),
            "killed hook must not contribute stdout: {:?}",
            result.messages()
        );
        // The NEXT hook still runs (Warn-continue semantics).
        assert!(
            result.messages().iter().any(|m| m == "ran-after-timeout"),
            "subsequent hook should run after a timeout, got {:?}",
            result.messages()
        );
    }

    // ---------------------------------------------------------------
    // v0.4.17 Phase 2 — NEW TESTS (matcher semantics + timeout plumbing)
    // ---------------------------------------------------------------

    /// `matcher: ""` (what `aris init` writes for `meta_opt` hooks) matches
    /// every tool, exactly like an absent matcher.
    #[test]
    fn matcher_empty_string_matches_all_tools() {
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec_with("printf 'meta-opt'", Some(""), None)],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("AnyToolAtAll", r#"{"k":"v"}"#);

        assert_eq!(result, HookRunResult::allow(vec!["meta-opt".to_string()]));
    }

    /// The matcher regex is anchored to the FULL tool name (Claude Code
    /// semantics): `Edit` must not fire for `MultiEdit`, while an explicit
    /// wildcard pattern still works.
    #[test]
    fn matcher_regex_is_anchored_to_full_tool_name() {
        assert!(spec_matches_tool(Some("Edit"), "Edit"));
        assert!(
            !spec_matches_tool(Some("Edit"), "MultiEdit"),
            "`Edit` must not substring-match `MultiEdit`"
        );
        assert!(spec_matches_tool(Some("Notebook.*"), "NotebookEdit"));
        assert!(spec_matches_tool(Some("Edit|Write"), "Write"));
        assert!(!spec_matches_tool(Some("Edit|Write"), "Read"));
        // None / empty: match-all.
        assert!(spec_matches_tool(None, "Anything"));
        assert!(spec_matches_tool(Some(""), "Anything"));
    }

    /// An uncompilable matcher falls back to a literal whole-string
    /// comparison (warned once on stderr) — it neither disables the hook
    /// nor broadens it to match-all.
    #[test]
    fn invalid_matcher_regex_falls_back_to_literal_exact_match() {
        // "Bash[" is an invalid regex (unclosed character class).
        assert!(
            spec_matches_tool(Some("Bash["), "Bash["),
            "literal fallback must match the exact pattern text"
        );
        assert!(
            !spec_matches_tool(Some("Bash["), "Bash"),
            "literal fallback must not loosely match other tools"
        );

        // End-to-end: the hook with the broken matcher is skipped for a
        // non-matching tool instead of firing or erroring.
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec_with("printf 'fired'", Some("Bash["), None)],
            Vec::new(),
        ));
        let result = runner.run_pre_tool_use("Bash", r#"{"command":"pwd"}"#);
        assert_eq!(result, HookRunResult::allow(Vec::new()));
    }

    /// TIMEOUT-1 clamp table: default 30s when absent; explicit values
    /// clamped to [1, 600] seconds.
    #[test]
    fn effective_hook_timeout_secs_defaults_and_clamps() {
        assert_eq!(effective_hook_timeout_secs(None), 30);
        assert_eq!(effective_hook_timeout_secs(Some(0)), 1);
        assert_eq!(effective_hook_timeout_secs(Some(1)), 1);
        assert_eq!(effective_hook_timeout_secs(Some(45)), 45);
        assert_eq!(effective_hook_timeout_secs(Some(600)), 600);
        assert_eq!(effective_hook_timeout_secs(Some(601)), 600);
        assert_eq!(effective_hook_timeout_secs(Some(u64::MAX)), 600);
    }

    /// `timeout: 0` clamps UP to 1s end-to-end: the hook is still given a
    /// real (1s) window rather than being killed instantly, and is then
    /// killed instead of sleeping out its full 5s.
    #[cfg(not(windows))]
    #[test]
    fn timeout_zero_clamps_to_one_second_end_to_end() {
        use std::time::Instant;

        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![spec_with("sleep 5; printf 'never'", None, Some(0))],
            Vec::new(),
        ));

        let start = Instant::now();
        let result = runner.run_pre_tool_use("Bash", r#"{"command":"true"}"#);
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() >= 900 && elapsed.as_millis() < 4000,
            "timeout 0 must clamp to ~1s; took {}ms",
            elapsed.as_millis()
        );
        assert!(!result.is_denied());
        assert!(
            result
                .messages()
                .iter()
                .any(|m| m.contains("timed out after 1s")),
            "expected clamped 1s timeout warning: {:?}",
            result.messages()
        );
    }

    /// TIMEOUT-2: the timed-out hook's child process is actually dead
    /// (killed AND reaped) after the runner returns — `kill -0 <pid>` must
    /// fail. The hook writes its own PID (kept across `exec sleep`) to a
    /// temp file before blocking.
    #[cfg(not(windows))]
    #[test]
    fn timed_out_hook_child_process_is_killed() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let pid_file = std::env::temp_dir().join(format!(
            "aris-hook-timeout-pid-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let command = format!("echo $$ > '{}'; exec sleep 30", pid_file.display());
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec![RuntimeHookSpec {
                matcher: None,
                command,
                timeout_secs: Some(1),
                async_flag: None,
            }],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("Bash", r#"{"command":"true"}"#);

        assert!(!result.is_denied());
        assert!(
            result.messages().iter().any(|m| m.contains("timed out")),
            "expected timeout warning: {:?}",
            result.messages()
        );
        let pid = std::fs::read_to_string(&pid_file)
            .expect("hook should have written its pid before sleeping")
            .trim()
            .to_string();
        let _ = std::fs::remove_file(&pid_file);
        assert!(!pid.is_empty(), "pid file must not be empty");
        let probe = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("kill -0 {pid}"))
            .stderr(std::process::Stdio::null())
            .status()
            .expect("kill -0 probe should run");
        assert!(
            !probe.success(),
            "hook child PID {pid} is still alive after the timeout kill"
        );
    }

    #[cfg(windows)]
    fn shell_snippet(script: &str) -> String {
        script.replace('\'', "\"")
    }

    #[cfg(not(windows))]
    fn shell_snippet(script: &str) -> String {
        script.to_string()
    }
}
