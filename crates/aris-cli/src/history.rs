//! Cross-session REPL history persistence (`~/.config/aris/history`).
//!
//! v0.4.16 Track A — **purely additive**. This module owns only the on-disk
//! side of REPL history. The in-memory `LineEditor.history: Vec<String>`
//! semantics (used by Up/Down navigation) are unchanged: every entry the REPL
//! pushes still enters memory verbatim. We merely:
//!
//! * LOAD the file at startup (best-effort; missing/corrupt → empty start), and
//! * APPEND one line per submitted entry, per-entry (not save-on-exit, because
//!   Ctrl+C/Ctrl+D exits skip any flush).
//!
//! ## File format
//! Plain text, one entry per line, oldest-first (matches the in-memory Vec
//! order so Up/Down indices reproduce exactly on reload). Entries are stored
//! with the same trim/blank filter as `LineEditor::push_history` — a blank
//! (whitespace-only) entry is never written; a non-blank entry is written
//! verbatim. Because entries are flattened to a single line by the editor,
//! a stored line never contains an embedded newline.
//!
//! ## Security (continues the v0.4.14 S9 redaction lineage)
//! * File perms are 0600 (owner rw only) on unix.
//! * **disk-only secret-skip**: an entry that looks like it carries a secret
//!   is NOT appended to disk. Crucially this affects *only* the disk append —
//!   the entry still enters `self.history` in memory, so the in-session Up/Down
//!   experience is byte-identical to today.
//! * `ARIS_NO_HISTORY` (truthy) is a kill-switch: load and save both become
//!   no-ops; nothing is read from or written to disk. In-memory accumulation is
//!   unaffected.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const CONFIG_DIR: &str = ".config/aris";
const HISTORY_FILE: &str = "history";

/// Only load at most this many of the most recent lines, to bound memory and
/// startup cost for very long-lived shells.
const LOAD_CAP: usize = 1000;

/// Resolve the on-disk history path: `~/.config/aris/history`.
#[must_use]
pub fn history_path() -> PathBuf {
    PathBuf::from(runtime::home_dir())
        .join(CONFIG_DIR)
        .join(HISTORY_FILE)
}

/// True when the `ARIS_NO_HISTORY` kill-switch is set to a truthy value.
///
/// Truthy = any value other than empty / `0` / `false` / `no` / `off`
/// (case-insensitive). An unset variable is falsy.
#[must_use]
pub fn history_disabled() -> bool {
    match std::env::var("ARIS_NO_HISTORY") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "no" | "off")
        }
        Err(_) => false,
    }
}

/// Load history lines from `path`, applying the same blank filter the editor
/// uses (`!entry.trim().is_empty()`) and capping at the most recent
/// [`LOAD_CAP`] entries. Returns oldest-first.
///
/// Best-effort: a missing file, an unreadable file, or invalid UTF-8 all yield
/// an empty Vec (silent empty start), mirroring `config.load().unwrap_or_default`.
/// Honors the `ARIS_NO_HISTORY` kill-switch (returns empty without touching disk).
#[must_use]
pub fn load_history(path: &PathBuf) -> Vec<String> {
    if history_disabled() {
        return Vec::new();
    }
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = Vec::new();
    for line in reader.lines() {
        // A line with invalid UTF-8 ends the read silently (best-effort).
        let Ok(raw) = line else { break };
        if !raw.trim().is_empty() {
            lines.push(raw);
        }
    }
    if lines.len() > LOAD_CAP {
        let start = lines.len() - LOAD_CAP;
        lines.drain(..start);
    }
    lines
}

/// Append a single entry to the history file (oldest-first, one line each).
///
/// No-op when:
/// * `ARIS_NO_HISTORY` is set (kill-switch),
/// * the entry is blank (same `trim().is_empty()` filter as `push_history`), or
/// * the entry looks like it carries a secret ([`looks_like_secret`]).
///
/// All disk I/O is best-effort: any error is swallowed so a write failure can
/// never break the REPL (the in-memory history already has the entry).
/// On unix the file is created with mode 0600 and defensively re-chmod'd 0600
/// after a successful write.
pub fn append_entry(path: &PathBuf, entry: &str) {
    if history_disabled() {
        return;
    }
    if entry.trim().is_empty() {
        return;
    }
    // disk-only secret-skip: the entry still lives in memory (caller pushed it
    // already); we just refuse to persist it.
    if looks_like_secret(entry) {
        return;
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let Ok(mut file) = opts.open(path) else {
        return;
    };
    // One entry per line; entries are already single-line (editor flattens).
    let _ = writeln!(file, "{entry}");
    let _ = file.flush();

    // Defensive: re-assert 0600 in case the file pre-existed with looser perms.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
}

/// Heuristic: does this submitted line look like it carries a secret/credential
/// that should not be persisted to disk?
///
/// Best-effort (disclosed as imperfect — new secret shapes may slip through;
/// since this only gates the *disk* copy, a miss never affects in-session
/// history and a false-positive only drops one line from the on-disk file).
/// Detects:
/// * an `sk-<key>` OpenAI-style API key (`sk-` at a word boundary followed by
///   >=16 key chars — the boundary + length avoid words like `ask-`/`task-`),
/// * well-known credential env names (`ANTHROPIC_API_KEY` / `AUTH_TOKEN` /
///   `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`),
/// * a credential keyword (`api[_-]?key` / `secret[_-]?key` / `access[_-]?key`
///   / `private[_-]?key` / `client[_-]?secret` / `password` / `passwd` /
///   `token` / `secret`) followed by `=`/`:` + value, or a `--flag value` form,
/// * a run of >=32 high-entropy `[A-Za-z0-9_-]` characters (a bare token).
#[must_use]
pub fn looks_like_secret(entry: &str) -> bool {
    let lower = entry.to_ascii_lowercase();

    // Well-known credential env var names (these literals don't occur in
    // ordinary prose, so a bare substring match is safe).
    if lower.contains("anthropic_api_key")
        || lower.contains("auth_token")
        || lower.contains("aws_access_key_id")
        || lower.contains("aws_secret_access_key")
    {
        return true;
    }

    // `sk-<key>` API-key shape (boundary + length, not a bare `sk-` substring).
    if has_sk_api_key(&lower) {
        return true;
    }

    // `api_key=`, `password: ...`, `--api-key value` shapes: a credential
    // keyword followed by `=`/`:` (or a flag-style space) and a non-empty value.
    if has_credential_assignment(&lower) {
        return true;
    }

    // A bare high-entropy token: >=32 chars from [A-Za-z0-9_-] in one run.
    if has_long_token_run(entry) {
        return true;
    }

    false
}

/// `sk-` at a word boundary (start of string or after a non-alphanumeric byte)
/// followed by at least 16 `[A-Za-z0-9_-]` chars — the OpenAI API-key shape.
/// The boundary requirement rejects ordinary words like `ask-`, `task-`,
/// `disk-`, `risk-` (where `sk` is preceded by a letter).
fn has_sk_api_key(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 3 <= n {
        if &bytes[i..i + 3] == b"sk-" {
            let boundary_before = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            if boundary_before {
                let mut run = 0usize;
                let mut j = i + 3;
                while j < n
                    && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'-')
                {
                    run += 1;
                    j += 1;
                }
                if run >= 16 {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// True if `lower` (already lowercased) contains a credential keyword followed
/// by an assignment to a non-empty value: `name=value` / `name: value`, or a
/// CLI flag form `--name value` (keyword immediately preceded by `-`).
fn has_credential_assignment(lower: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "api_key",
        "api-key",
        "apikey",
        "secret_key",
        "secret-key",
        "access_key",
        "access-key",
        "private_key",
        "private-key",
        "client_secret",
        "client-secret",
        "password",
        "passwd",
        "token",
        "secret",
    ];
    for kw in KEYWORDS {
        let mut from = 0usize;
        while let Some(rel) = lower[from..].find(kw) {
            let idx = from + rel;
            let after = idx + kw.len();
            // `=`/`:` assignment in any context.
            if assignment_follows(&lower[after..]) {
                return true;
            }
            // `--name value` CLI-flag form: only when the keyword is part of a
            // flag (immediately preceded by `-`), so plain prose like
            // "the token is high" is not flagged.
            if idx >= 1 && lower.as_bytes()[idx - 1] == b'-' && space_value_follows(&lower[after..])
            {
                return true;
            }
            from = after;
        }
    }
    false
}

/// Does `rest` begin with optional spaces, then `=` or `:`, then optional
/// spaces, then at least one non-space value char?
fn assignment_follows(rest: &str) -> bool {
    let trimmed = rest.trim_start_matches([' ', '\t']);
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    if first != '=' && first != ':' {
        return false;
    }
    let value = trimmed[first.len_utf8()..].trim_start_matches([' ', '\t']);
    value.chars().next().is_some_and(|c| !c.is_whitespace())
}

/// Does `rest` begin with at least one space/tab and then a non-space value
/// char? Used only for the `--flag value` CLI form.
fn space_value_follows(rest: &str) -> bool {
    let trimmed = rest.trim_start_matches([' ', '\t']);
    // Require at least one space was actually consumed (so `--api-keyx` with no
    // separator is not treated as a flag-value).
    trimmed.len() < rest.len() && trimmed.chars().next().is_some_and(|c| !c.is_whitespace())
}

/// True if `entry` contains an unbroken run of >=32 chars all in
/// `[A-Za-z0-9_-]` (a bare high-entropy token).
fn has_long_token_run(entry: &str) -> bool {
    let mut run = 0usize;
    for ch in entry.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            run += 1;
            if run >= 32 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests that mutate the process-global `ARIS_NO_HISTORY` env var serialize
    // via the crate-wide `crate::env_test_guard()` (same lock the config /
    // openai_executor env tests use) so no env-mutating test can race
    // (codex Phase-1 consistency nit).

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "aris_history_test_{}_{}_{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_file(&p);
        p
    }

    // ── secret-skip predicate ────────────────────────────────────────────────

    #[test]
    fn secret_detects_sk_api_key() {
        // `sk-` at a boundary + >=16 key chars = OpenAI key shape.
        assert!(looks_like_secret(
            "OPENAI_API_KEY=sk-proj-abcdefghij1234567890"
        ));
        assert!(looks_like_secret("sk-abcdefghijklmnopqrstuvwxyz"));
        // Boundary: at start of string.
        assert!(looks_like_secret("sk-0123456789abcdef0123"));
    }

    #[test]
    fn secret_does_not_flag_ordinary_sk_words() {
        // codex Phase-1 fix: `sk-` is no longer a bare substring match — `sk`
        // preceded by a letter (ask/task/disk/risk) is NOT a key, and a short
        // `sk-xx` run is below the 16-char key threshold.
        assert!(!looks_like_secret("ask-me about the results"));
        assert!(!looks_like_secret("the task-list for today"));
        assert!(!looks_like_secret("check disk-space usage"));
        assert!(!looks_like_secret("a risk-free approach"));
        assert!(!looks_like_secret("sk-short")); // <16 chars after sk-
    }

    #[test]
    fn secret_detects_known_env_names() {
        assert!(looks_like_secret("ANTHROPIC_API_KEY=foo"));
        assert!(looks_like_secret("set AUTH_TOKEN now"));
        assert!(looks_like_secret("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE"));
        assert!(looks_like_secret("AWS_SECRET_ACCESS_KEY is set"));
    }

    #[test]
    fn secret_detects_credential_assignment() {
        assert!(looks_like_secret("api_key=hunter2"));
        assert!(looks_like_secret("api-key : value"));
        assert!(looks_like_secret("apikey=x"));
        assert!(looks_like_secret("token: abcdef"));
        assert!(looks_like_secret("secret=swordfish"));
        // codex Phase-1 fix: previously-missed credential names (these would
        // have been PERSISTED to disk — a real leak).
        assert!(looks_like_secret("password=hunter2"));
        assert!(looks_like_secret("passwd: letmein"));
        assert!(looks_like_secret("private_key=MIIabc"));
        assert!(looks_like_secret("access_key=AKIA123"));
        assert!(looks_like_secret("secret_key: shh"));
        assert!(looks_like_secret("client_secret=xyz"));
        // CLI flag form `--name value` (keyword preceded by `-`).
        assert!(looks_like_secret("aris --api-key mytoken123"));
        assert!(looks_like_secret("curl --password hunter2 example.com"));
    }

    #[test]
    fn secret_detects_long_token_run() {
        // 40-char [A-Za-z0-9_-] run.
        assert!(looks_like_secret("ghp_0123456789ABCDEFabcdef0123456789ABCD"));
    }

    #[test]
    fn secret_allows_ordinary_text() {
        assert!(!looks_like_secret("/research-review the latest results"));
        assert!(!looks_like_secret("what is the keyboard shortcut"));
        assert!(!looks_like_secret("explain tokenization to me"));
        assert!(!looks_like_secret("the api key is missing")); // no =/: assignment
        assert!(!looks_like_secret("the token is high")); // prose "token", no flag/assignment
        assert!(!looks_like_secret("short-id-1234")); // <32 char run
        assert!(!looks_like_secret(""));
    }

    // ── append / load round-trip + format ────────────────────────────────────

    #[test]
    fn append_then_load_round_trips_oldest_first() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("roundtrip");

        append_entry(&path, "first");
        append_entry(&path, "second");
        append_entry(&path, "third");

        let loaded = load_history(&path);
        assert_eq!(loaded, vec!["first", "second", "third"]);

        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw, "first\nsecond\nthird\n");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn append_skips_blank_like_push_history() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("blank");

        append_entry(&path, "   ");
        append_entry(&path, "");
        append_entry(&path, "\t\t");
        // File should not even exist (nothing was written).
        assert!(!path.exists());

        append_entry(&path, "real");
        assert_eq!(load_history(&path), vec!["real"]);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn append_preserves_raw_untrimmed_value() {
        // Non-blank entries are stored verbatim (surrounding whitespace kept),
        // matching push_history's in-memory contract.
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("untrimmed");

        append_entry(&path, "  hello world  ");
        let loaded = load_history(&path);
        assert_eq!(loaded, vec!["  hello world  "]);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn append_does_not_dedup() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("dedup");

        append_entry(&path, "/status");
        append_entry(&path, "/status");
        assert_eq!(load_history(&path), vec!["/status", "/status"]);

        let _ = fs::remove_file(&path);
    }

    /// v0.4.17 A5.4 — slash commands now enter history (deliberately flipped
    /// in v0.4.17 per maintainer decision 2026-06-05; v0.4.16 had the REPL
    /// `continue` past the history calls for slash input, a contract that
    /// existed only at the `run_repl` call-site, with no test locking it).
    /// This locks the disk half of the new behavior: a slash command line
    /// persists via `append_entry`, round-trips through `load_history`, and
    /// the secret-skip still applies to slash entries carrying credentials.
    #[test]
    fn append_persists_slash_commands() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("slash");

        append_entry(&path, "/model gpt-5.5");
        append_entry(&path, "plain question");

        let loaded = load_history(&path);
        assert_eq!(loaded, vec!["/model gpt-5.5", "plain question"]);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("/model gpt-5.5\n"), "slash line must be on disk");

        // Secret-skip is entry-shape based, so it guards slash commands too:
        // a credential-bearing slash line stays OFF disk.
        append_entry(&path, "/setup --api-key sk-abcdefghijklmnopqrstuvwxyz");
        assert_eq!(
            load_history(&path),
            vec!["/model gpt-5.5", "plain question"],
            "credential-bearing slash command must not persist"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_filters_blank_lines_from_file() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("loadblank");
        // Hand-craft a file with blank lines interspersed.
        fs::write(&path, "alpha\n\n   \nbeta\n\t\ngamma\n").unwrap();
        assert_eq!(load_history(&path), vec!["alpha", "beta", "gamma"]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("missing");
        assert!(!path.exists());
        assert!(load_history(&path).is_empty());
    }

    #[test]
    fn load_caps_at_most_recent_entries() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("cap");
        let mut content = String::new();
        for i in 0..(LOAD_CAP + 50) {
            content.push_str(&format!("entry{i}\n"));
        }
        fs::write(&path, content).unwrap();
        let loaded = load_history(&path);
        assert_eq!(loaded.len(), LOAD_CAP);
        // Oldest kept is entry50; newest is the last.
        assert_eq!(loaded.first().unwrap(), "entry50");
        assert_eq!(loaded.last().unwrap(), &format!("entry{}", LOAD_CAP + 49));
        let _ = fs::remove_file(&path);
    }

    // ── disk-only secret-skip (does NOT affect in-memory, only disk) ──────────

    #[test]
    fn append_skips_secret_on_disk() {
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("secret");

        append_entry(&path, "ANTHROPIC_API_KEY=sk-secret-value-123");
        append_entry(&path, "plain question");
        // Only the non-secret line lands on disk.
        assert_eq!(load_history(&path), vec!["plain question"]);

        let _ = fs::remove_file(&path);
    }

    // ── ARIS_NO_HISTORY kill-switch (load + save both no-op) ──────────────────

    #[test]
    fn no_history_env_disables_save_and_load() {
        let _g = crate::env_test_guard();
        let path = tmp_path("killswitch");

        // First write a real line with the switch OFF so the file exists.
        std::env::remove_var("ARIS_NO_HISTORY");
        append_entry(&path, "before-switch");
        assert!(path.exists());

        // Now flip the kill-switch: save is a no-op (no new line).
        std::env::set_var("ARIS_NO_HISTORY", "1");
        append_entry(&path, "should-not-persist");
        // File unchanged: still just the pre-switch line on disk.
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw, "before-switch\n");
        // load is also a no-op regardless of file contents.
        assert!(load_history(&path).is_empty());

        std::env::remove_var("ARIS_NO_HISTORY");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn no_history_recognizes_falsy_values() {
        let _g = crate::env_test_guard();
        for falsy in ["", "0", "false", "no", "off", "FALSE", "Off"] {
            std::env::set_var("ARIS_NO_HISTORY", falsy);
            assert!(!history_disabled(), "{falsy:?} should be falsy");
        }
        for truthy in ["1", "true", "yes", "on", "anything"] {
            std::env::set_var("ARIS_NO_HISTORY", truthy);
            assert!(history_disabled(), "{truthy:?} should be truthy");
        }
        std::env::remove_var("ARIS_NO_HISTORY");
    }

    // ── 0600 perms on unix ────────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn append_sets_0600_perms() {
        use std::os::unix::fs::PermissionsExt;
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("perms");

        append_entry(&path, "line");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "history file must be owner-rw only");

        let _ = fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn append_rechmods_preexisting_loose_perms() {
        use std::os::unix::fs::PermissionsExt;
        let _g = crate::env_test_guard();
        std::env::remove_var("ARIS_NO_HISTORY");
        let path = tmp_path("loose");

        // Pre-create the file world-readable.
        fs::write(&path, "old\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        append_entry(&path, "new");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "append must defensively re-assert 0600");

        let _ = fs::remove_file(&path);
    }
}
