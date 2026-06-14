# 🌙 ARIS-Code — Auto Research in Sleep

```
    ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
    ░  █████╗ ██████╗ ██╗███████╗            ░
    ░ ██╔══██╗██╔══██╗██║██╔════╝            ░
    ░ ███████║██████╔╝██║███████╗            ░
    ░ ██╔══██║██╔══██╗██║╚════██║            ░
    ░ ██║  ██║██║  ██║██║███████║            ░
    ░ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝╚══════╝           ░
    ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
         🟦 [Claude]    🟩 [GPT 🕶️]
         executor  ←→  reviewer
         Let AI do research while you sleep
```

![ARIS-Code Screenshot](docs/screenshot.png)

> **Adversarial · Multi-Agent Research Automation CLI**
> Executor acts · Reviewer critiques · Iterate to excellence

[![GitHub Release](https://img.shields.io/github/v/release/wanshuiyin/Auto-claude-code-research-in-sleep?style=flat-square)](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/releases)
[![Downloads](https://img.shields.io/github/downloads/wanshuiyin/Auto-claude-code-research-in-sleep/total?style=flat-square&color=brightgreen)](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%20|%20Linux%20|%20Windows-black?style=flat-square)](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)


## 📰 What's New

> **v0.4.19** (2026-06-14) — **Honesty / guardrails patch** (theme proposed by a Codex fresh-eyes audit; no behavior change for healthy setups). **🔴 MCP protocol-version negotiation guard** — the real bug this fixes: the stdio handshake requested `2025-03-26` but **never read the version the server negotiated back**, so a server agreeing on a version ARIS can't speak was silently accepted and later `tools/list` / `tools/call` ran on an incompatible protocol with opaque failures. ARIS now validates the negotiated version against a supported set (`2025-11-25` / `2025-06-18` / `2025-03-26` / `2024-11-05`) and, on an unsupported one, terminates the connection + degrades that server with a clear reason (`aris doctor` shows it) — per the MCP lifecycle spec. The request stays `2025-03-26`, so **healthy servers are unaffected** (verified: the real Codex MCP server still spawns + initializes). **🧹 Papercuts**: the OpenAI-family subagent fail-loud message dropped its stale "lands in v0.4.18" marker (now version-agnostic + actionable); OpenAI upstream error bodies are now truncated + credential-redacted (`sk-…` / `Bearer …`, including the compact-JSON shape a proxy can reflect back) instead of splatted verbatim; the system-prompt hook summary now counts only the hooks the runtime actually executes. Tests (CI mode): api 32 / runtime 204 / tools 67 / aris-cli 167 / commands 5, all green. Codex MCP (gpt-5.5 xhigh): design GO → impl NO-GO (compact-secret miss + command-string strictness) → GO after fixes.

> **v0.4.18** (2026-06-14) — **Default model → Claude Opus 4.8**, with correct pricing and a safety net. **🆕 Opus 4.8 default** across the model picker, the `opus` alias, `aris setup`, and subagents — with an **availability fallback**: if your account lacks 4.8 (the API returns `404 not_found_error`), ARIS auto-falls-back to `claude-opus-4-7` for the session, rebuilds the system-prompt model identity so it stays coherent (you're never told you're 4.8 while served 4.7), warns once, and retries — for the main session (text + JSON) and subagents. It fires only on that precise 404 (never on 400 / rate-limit / auth), latches against loops, and the text path rebuilds from a pre-turn snapshot so the retry never double-sends your message; accounts **with** 4.8 are byte-identical to a plain bump. **💰 Pricing corrected** (was a 3–5× over-estimate): current Opus 4.5–4.8 = `$5/$25` (deprecated Opus 4/4.1 keep `$15/$75`, split by word-boundary so a future `opus-4-10` isn't mis-tiered); Sonnet 4.x = `$3/$15` (Haiku was already right). **🧹 Backlog**: `aris setup` option 10 pins the Codex MCP reviewer to `model_reasoning_effort="xhigh"` (deterministic for new setups, independent of `~/.codex/config.toml`); a startup + `aris doctor` **misconfig hint** ([#259](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/issues/259)) for a silently-ignored or misplaced config (malformed JSON, or a stray `~/.aris/config.yaml`); the system-prompt hook summary now marks parsed-but-never-fired events **"PARSED ONLY … will NOT run"** instead of implying dead hooks run (full event expansion deferred). Tests (CI mode): api 32 / runtime 202 / tools 67 / aris-cli 166 / commands 5, all green; live smoke returns `model=claude-opus-4-8` end-to-end. Codex MCP (gpt-5.5 xhigh) reviewed the design + both implementation batches (REWORK→GO, NO-GO→GO, GO).

> **v0.4.17** (2026-06-10) — **The MCP release**: `mcpServers` in settings.json finally drive **real tool dispatch**. **🆕 MCP wiring (M1/M2)**: configured stdio servers are spawned at startup, their tools advertised as `mcp__<server>__<tool>` on both provider paths (Anthropic + OpenAI-family) and dispatched end-to-end; per-server failures degrade softly (healthy servers keep working); `aris doctor` shows real per-server status. Untrusted MCP tools **prompt for approval even under danger-full-access** (they're external processes the sandbox can't cover) — `mcpServers.<name>.trust: true` or a session-level "always for this server" skips it; `--allowedTools` accepts `mcp__` names. **🔴 NDJSON framing fix**: our stdio transport spoke LSP-style `Content-Length:` framing, but the MCP spec (and `codex mcp-server`) use newline-delimited JSON-RPC — discovery silently timed out against real servers (fake-server tests passed because they spoke the same wrong dialect; only real-machine e2e caught it). Writes are now NDJSON, reads auto-detect both dialects, verified end-to-end against codex. Plus: spec-mandated `notifications/initialized`, concurrent write/read (no large-payload pipe deadlock, the [#286](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/issues/286) failure mode). **🆕 `aris setup` option 10 — Codex MCP reviewer, zero API key**: one guided step detects the codex CLI, writes an idempotent `mcpServers.codex` entry (atomic + backup, never clobbers), asks consent for `trust: true`, and optionally keeps an API reviewer as **fallback** (new `reviewer_fallback_provider`; MCP stays primary). Cross-model adversarial review on a ChatGPT subscription — no OpenAI API key. **🆕 Hooks**: object-style schema preserved (matcher/timeout/async no longer dropped); anchored-regex matcher filtering; ⚠️ hooks are now **killed after 30 s by default** (was: wait forever; per-hook `timeout` 1–600 s overrides; timeout warns, never denies). **🧹 Long tail**: `ARIS_DISABLE_KEYCHAIN` escape hatch (api tests green locally for the first time since v0.4.15), Anthropic `stop_reason` clean-EOF symmetry (CL2), OpenAI tool-call id-fallback (OE6), slash commands enter history. Tests (CI mode): runtime 199 / aris-cli 165 / tools 67 / api 30 / commands 5, all green. Codex MCP (gpt-5.5 xhigh) reviewed each phase: 16 rounds (R1–R16), 7 NO-GOs all resolved. Deferred to v0.4.18: P8 full OpenAI subagent routing, hook async execution, protocolVersion bump.

> **v0.4.16** (2026-05-30) — **REPL UX + provider hardening**, on a zero-regression discipline: 64 characterization ("golden") tests were written first to lock the *current* provider-routing / pricing / reviewer / subagent / REPL behavior, then kept green through every change. **🆕 Command history ([#274](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/issues/274))**: prompts now persist to `~/.config/aris/history` (0600) and reload on startup; `ARIS_NO_HISTORY` kill-switch; a **disk-only** secret-skip refuses to write credential-looking lines (they still stay in in-session history). **🆕 Ctrl+R reverse search** (`(reverse-i-search)`, bash-style; CJK-aware single-line render; no new dependency; no existing key binding changed). **🔒 OpenAI-family subagents fail loud**: an OpenAI-family main session (Kimi/GLM/Gemini/MiniMax/…) spawning a sub-agent previously **silently billed the user's Anthropic OAuth/Keychain credential**; it now returns a clear error (no credential names) instead — Anthropic-family executors are unaffected. Full OpenAI sub-agent *routing* is a cross-crate change deferred to v0.4.17; this closes the credential-leak window now. **🧱 Provider groundwork (no behavior change)**: the 3 byte-identical word-boundary matchers consolidate into one canonical `runtime::word_match` (callers forward, truth values unchanged); a new pure `runtime::ProviderFamily` classifier (unwired). Tests (CI mode): runtime 164 / aris-cli 128 / tools 49 / commands 5 — all green incl. the 64 golden tests; the dangerous code (config env-writing, order-sensitive pricing chain, reviewer routing, `provider_match`, `push_history`, every key binding) is byte-identical. Codex MCP (gpt-5.5 xhigh) reviewed each phase + a final integration pass. Deferred to v0.4.17: full OpenAI subagent routing, hook-schema + MCP wiring, `api` test isolation.

> **v0.4.15** (2026-05-29) — **OpenAI-compatible streaming robustness** hotfix. Closes [#249](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/issues/249): MiniMax (and other OpenAI-compatible providers / proxies) were effectively unusable because the clean-EOF completion check treated the `data: [DONE]` SSE sentinel as the *only* authoritative signal. **🔴 #249**: a non-empty `choices[].finish_reason` is the Chat Completions spec's terminal-chunk marker; `[DONE]` is a transport convention some compatible providers never emit (MiniMax sends `finish_reason: "stop"` then closes without `[DONE]`). The clean-EOF decision is now a pure, unit-tested `stream_eof_action(...)` that completes on EITHER `[DONE]` OR a non-empty `finish_reason`; reads are NOT stopped early at finish_reason (a trailing `include_usage` usage-only chunk is still consumed), genuine truncation still hard-errors, and a pre-output proxy abort still restarts. **OE7**: `finish_reason` is read before the `delta` guard so a terminal choice with only finish_reason and no delta is recognized. **OE2**: pending tool calls flush on *any* non-empty finish_reason (`length`/`content_filter`/`max_output`/`sensitive`), preserving ordering + per-tool rendering. **OE4**: a mid-stream error envelope (top-level non-null `error`, no `choices`) now hard-errors instead of being silently dropped (closes the regression window where an error after a finish_reason would be misjudged a success). **OE3**: SSE `data:` parsing tolerates a missing space (`data:{...}`, W3C-legal, emitted by some compat providers). +5 unit tests (77→82) extract the previously-untested SSE completion logic into pure helpers. Anthropic SSE path untouched. Cross-reviewed by Codex MCP (gpt-5.5 xhigh) across 3 rounds (GO-WITH-NITS → GO-WITH-NITS → **GO**); deferred to v0.4.16: CL2 (Anthropic stop_reason symmetry), OE6/OE5/OE8, ProviderFamily (P7) + Subagent parity (P8).

> **v0.4.14** (2026-05-25) — **Security-hygiene release** closing the top items from the v0.4.13 codex audit (gpt-5.5 xhigh, 6/10 NEEDS-REWORK verdict). **🔴 S9 (P0): system-prompt config redaction** — before v0.4.14, `render_config_section()` dumped the merged `settings.json` value verbatim into the system prompt sent to the LLM provider, leaking `env` maps, `mcpServers.<name>.headers.Authorization` Bearer tokens, hook command env, signed-URL query params, `apiKey` fields and the like to the model. The new renderer whitelists top-level fields (`model`/`permissionMode`/`theme`/`outputStyle`/`permissions`/`sandbox` with recursive redaction inside), recursively redacts sensitive keys (`apikey`/`token`/`secret`/`password`/`authorization`/`headers`/`env`/`_KEY`/`_SECRET`/`_TOKEN`), replaces `mcpServers.<name>.command` with `<configured>`/`<empty>`/`<unrecognized shape>` placeholders, reduces `mcpServers.<name>.url` to a strict `<scheme://host[:port]>` origin (scheme allow-list `http`/`https`/`ws`/`wss`, ASCII host, digit-only port, IPv6 brackets), and drops hook command strings entirely (replaces with hook count). Regression test covers 9 distinct leak surfaces. **🟡 P9 (P1): DeepSeek help line** — `aris --help` now points at `aris setup` option 7 (the actual `anthropic-compat` menu entry) instead of an `EXECUTOR_PROVIDER=anthropic-compat` env-var path that the resolver never honored. **🟡 M1/M2 (P1) doc**: `aris doctor` prints a yellow experimental warning whenever `mcpServers.len() > 0` because `McpServerManager` is not yet wired into `CliToolExecutor` tool dispatch (planned for v0.4.16); README + README_CN gain matching callouts. **🟢 C11 (P2) stream idle timeout** — both Anthropic `MessageStream` and the OpenAI SSE loop wrap `response.chunk().await` in `tokio::time::timeout` (`ARIS_STREAM_IDLE_TIMEOUT_SECS`, default 120, clamp `[10, 1800]`, 0/negative disables). On idle the stream takes the same retry path as a mid-body abort. Closes the "aris hangs forever with no output" symptom when an upstream HTTPS proxy holds a connection without keepalives. **🟢 H11 (P2)**: `tools/sync_main_skills.sh` version hint bumped from v0.4.11 to v0.4.13. Cross-reviewed by Codex MCP (gpt-5.5 xhigh) across 4 rounds (NO-GO + 4 findings → GO-WITH-NITS + 3 → NO-GO + 1 port-smuggling → **GO**).

> **v0.4.13** (2026-05-25) — **Residue-cleanup release** closing every codex-audit P1 left over from v0.4.10–v0.4.12 plus the long-tail regression tests. **🟡 v0.4.10 P1.D per-server MCP timeout** — `mcpServers.<name>.requestTimeoutSecs` override > `MCP_REQUEST_TIMEOUT_SECS` env > 300s default (clamped 1..=1800), so one Codex MCP agent can take 5 min while filesystem MCP errors in 5 s. **🟡 v0.4.10 known limitation closed** — `McpStdioProcess::request()` now skips JSON-RPC notifications (id absent/null) and keeps reading until the correlated response, so `notifications/log` / `notifications/progress` no longer kill the channel. **🟢 meta_opt hook deploy via `aris init`** — `tools/meta_opt/{log_event,check_ready}.sh` bundle into the binary and `aris init` writes ARIS-namespaced **`aris-meta-opt-log-event.sh`** / **`aris-meta-opt-check-ready.sh`** to `~/.claude/hooks/` (can never clobber user hooks per codex round-1 #1); settings.json updates are idempotent, backups hard-fail, and the final rewrite is atomic via tempfile + rename. **🧪 9 v0.4.12 targeted regression tests** for sandbox.strictMode (3) + parse strictMode + provider_match pricing + has_word o-series + stream_options 400 + meaningful-content classification + premature-EOF retry truth table (codex round-1 #3 — `should_retry_on_premature_eof()` extracted to pure fn, 7-row test). **📦 Bundle**: 76 skills, **54 helpers** (was 52; +2 meta_opt scripts). **📦 Skills source-of-truth fix on main** (`fedf361`): `gemini-search` / `research-lit` `auto-gemini-3` alias now in main so future syncs stay correct. Cross-reviewed by Codex MCP (gpt-5.5 xhigh) across 3 rounds (NO-GO + 3 hook/atomic/test findings → NO-GO + release-metadata-not-bumped → GO).

> **v0.4.12** (2026-05-22) — **Bug-fix + small-feature release**. **🚨 #238 `sandbox.strictMode`** — `SandboxConfig` adds `strict_mode: Option<bool>` (parsed from `settings.json` as `sandbox.strictMode`); when `true`, **all** LLM-supplied overrides are ignored, closing the gap where `dangerouslyDisableSandbox: true` could silently bypass user-configured sandbox policy. `aris doctor` reports effective sandbox state; bash tool schema documents the strict-mode behaviour. **#232 DeepSeek deprecation** — `auto-review-loop-llm` SKILL.md + setup UI updated from legacy `deepseek-chat` / `deepseek-reasoner` to `deepseek-v4-flash` / `deepseek-v4-pro` (legacy aliases deprecate 2026-07-24; reasoner models reject `tool_choice`). **v0.4.10 codex audit P1 follow-ups**: P1.A Anthropic stream retry now gates on `has_emitted_meaningful_content` (was raw `events_emitted`), so a stream that only sent `MessageStart` before EOF is retry-eligible; P1.B `supports_reasoning_effort` switched to word-boundary match so `openai/o3-mini` / `proxy:o4` provider-prefixed names route through reasoning-effort path (reviewer mirror at `tools/lib.rs` also updated); P1.C `stream_options.include_usage` proxy fallback retries once without `stream_options` when a 400 actually fingers it as unknown field; P2 pricing match precision via new `provider_match` helper so `qwen3.6-plus` / `kimi-k2.5` / `glm-4-plus` route correctly while rejecting mid-word matches like `my-kimi-clone`. **Skills sync**: `/interview-cheatsheet` + `/render-html` newly bundled (76 skills total, 52 helpers; `build.rs` ALLOWED_EXTS gains `html` for render-html templates). **v0.4.11 follow-ups**: `EXCLUDED_SKILL_PREFIXES` exact-list → `starts_with("skills-codex")`; CI workflow `fetch-depth: 0` so drift-test ancestor check runs. Cross-reviewed by Codex MCP (gpt-5.5 xhigh) across 4 rounds (GO-WITH-CAUTION + 8 findings → GO-WITH-CAUTION + 3 precision findings → NO-GO + 5 blockers → GO after fixes).

> **v0.4.11** (2026-05-18) — **Skills bundle refresh / research workflow sync**. Binary runtime behaviour unchanged from v0.4.10; the embedded skill set catches up to current `main`. **10 new skills** bundled: `/citation-audit` (fourth-layer bibliography audit) + `/experiment-queue` (SSH multi-seed job queue with OOM-retry + stale-screen cleanup) + `/kill-argument` (two-thread adversarial review for theory papers) + `/resubmit-pipeline` (W5: text-only port to a new venue under hard constraints) + `/paper-talk` (end-to-end conference talk pipeline) + `/slides-polish` (per-page Codex layout review) + `/overleaf-sync` (two-way Overleaf Git-bridge via Keychain) + `/gemini-search` + `/openalex` (broader literature sources) + `/qzcli` (Qizhi platform GPU jobs). **46 existing SKILL.md refreshed** — most notably canonical resolver chain rollout (closes real user incident where research-wiki was empty for a week from hardcoded `tools/research_wiki.py`), submission assurance gate + external verifier (paper-writing Phase 6 now functions), and proof-checker `--restatement-check` / `--deep-fix` opt-in flags. **Helpers**: tools/ goes 9 → 18; `research_wiki.py` refreshed 315 → 767 lines with canonical `ingest_paper` API (otherwise SKILL.md would reference API the bundled helper lacks). **Sync infrastructure**: `tools/sync_main_skills.sh` automates main → bundle rsync with symlink pre-flight + codex-mirror prune + `SKILLS_SOURCE_COMMIT` pinning; 3 new CI drift tests cover all 4 resolver layer patterns. **Gemini MCP** call in `/research-lit` now passes `model: 'auto-gemini-3'`. Cross-reviewed by Codex MCP (gpt-5.5 xhigh) across 4 rounds.

> **v0.4.10** (2026-05-17) — **Stream + MCP reliability release**. **C6** (closes the `#228`-style "error decoding response body" mid-stream loop): both Anthropic `MessageStream` and the OpenAI SSE loop now whole-stream-restart on chunk decode failure / premature EOF (`ARIS_STREAM_RETRY`, default 2, clamped 0..=5, fires only when nothing has been emitted yet so output never tears). **M3** (closes `#151` / `#172` "Calling codex..." stalls): MCP stdio `request()` gains a 300s default timeout covering both send + read (override `MCP_REQUEST_TIMEOUT_SECS`, clamped 1..=1800); `response.id ↔ request.id` correlation check; `ensure_server_ready()` detects dead children via `try_wait()` and transparently respawns; all failure paths `kill().await` the child so the next call starts clean. 3 new MCP regression tests bundled. **C8/P4**: OpenAI streaming requests now send `stream_options.include_usage: true` and parse `prompt_tokens_details.cached_tokens` → `cache_read_input_tokens`; Anthropic `MessageStart.usage` (input + cache halves) is stashed and merged with `MessageDelta.usage` (output) so post-compaction cache-hit ratios show the real number. **C9** multi-provider pricing: GPT-5.5/5.4/o1/o3/o4 (cache_read = input × 0.1 per OpenAI's actual prefix-cache discount — the prior generic 50% overstated savings 5×), Gemini 2.5/2.0, DeepSeek V3/V4/R1 (explicit cache_hit vs cache_miss tiers), GLM, MiniMax, Kimi/Moonshot, MiMo, Qwen, Doubao; `has_word()` boundary matcher so `openai/o3-mini` / `provider/<model>` route correctly. **Hygiene**: nine dead-code warnings cleared, `aris setup` help text + doctor strings synced with actual behaviour, `cargo fmt` over v0.4.10-touched files. Cross-reviewed by Codex MCP (gpt-5.5 xhigh).

> **v0.4.9** (2026-05-17) — **Closes Codex v0.4.7 audit residuals (L1+L3+L4)** + skill-helper subsystem completion. **L1**: `tools` crate also switches reqwest to `native-tls`, unifying TLS across all 3 reqwest consumers (DashScope-class endpoints now work on the LlmReview reviewer path too, not just main executor). Linux CI installs OpenSSL dev headers. **L3**: ApiClient trait gains `on_session_compacted()`; OpenAI's message-index-keyed reasoning_cache is cleared on auto-compaction so post-compaction replay doesn't aim at stale indices. **L4**: split `supports_reasoning_content_replay` predicate (superset includes Kimi/Moonshot/Xiaomi-MiMo/DeepSeek-R1 — providers that emit reasoning_content but don't accept reasoning_effort) + 32K char per-turn cap + 128K char total-cache cap with oldest-eviction. Plus: 2 new skills bundled (`/figure-spec` + `/paper-illustration-image2` with `scripts/` subdirs, new resolver Layer 0b = `$ARIS_CACHE_DIR/skills/<name>/scripts/`); `research_wiki.py` promoted from skill-local to shared `tools/` (9+ callers); 5 more SKILL.md migrated to fallback chain (`exa-search`, `semantic-scholar`, `arxiv`, `idea-creator`); inventory cargo test + smoke shell script for H6 regression class.

> **v0.4.8** (2026-05-17) — **Skill helper subsystem rewrite** + **two community bug fixes**. Bundled helpers now extract to `~/.config/aris/cache/<version>/` at startup (not cwd); every Skill invocation surfaces a `helperReport` with cache dir + 4-layer resolver preamble. `/skills export` ships helpers alongside SKILL.md. New `integration-contract.md` defines 6 failure policies (A gate / B side-effect / C forensic / D1 cascade / D2 multi-source / E diagnostic). 8 shared helpers (arxiv/deepxiv/exa/S2/openalex fetchers + save_trace + verify_papers + verify_paper_audits) bundled. `/research-lit` + `/deepxiv` SKILL.md migrated to fallback chain. Fixes: (a) `gpt-5.5 + tools 400` on OpenAI (executor stripped of `reasoning_effort` for gpt-5.5/o3/o4+tools on api.openai.com), (b) Custom reviewer reset-to-gpt-5.5 every restart (`/setup` menu option 9 vs 8 bug + `LlmReview` no longer falls back to gpt-5.5 for Custom).

> **v0.4.7** (2026-05-16) — **DashScope Coding Plan 405 fixed** (#159) via `native-tls` switch — credit [@GetIT-Sunday](https://github.com/GetIT-Sunday) (#225) | **`reasoning_content` replay for all reasoning models** (OpenAI o1/o3/o4 / DeepSeek-R1 / etc.), not just Kimi — pairs with v0.4.5 `reasoning_effort='xhigh'` for coherent multi-turn reasoning — credit [@GetIT-Sunday](https://github.com/GetIT-Sunday) (#226) | Cleanup: removed 600+ lines of `rusty-claude-cli` prototype dead code (`app.rs` / `args.rs` / `runtime/sse.rs`) + unused `rustyline` dep + "Claw Code" → "ARIS-Code" rebranding in user-facing strings.

> **v0.4.6** (2026-05-14) — **🚨 Two long-standing silent bugs fixed**: (1) `PermissionMode::Prompt` was *silently allowing every tool* due to derived-`Ord` bug, now correctly routes through the prompter; (2) system prompt hard-coded `current_date = "2026-03-31"`, causing models to reject real post-March-2026 data (including users' own arXiv papers) as "future / prompt injection" — now uses real system time via new `runtime::today_iso()`. Plus **Custom OpenAI-compatible provider** (`/setup` option 11, reviewer option 9) with dynamic `/models` discovery — credit [@Anduin9527](https://github.com/Anduin9527) (#221 + #222).

> **v0.4.5** (2026-05-13) — **First-class reasoning-model support** — `reasoning_effort='xhigh'` actually on the wire for GPT-5.5 / o1 / o3 / o4 / DeepSeek-thinking | **Thinking content blocks** end-to-end (fixes #161) | **Multi-tool result grouping** fix (`tool_use_ids_without_tool_result`) | **DeepSeek V4 Pro** + **Xiaomi MiMo** + **Qwen 3.6** + **Doubao** in `/setup` (options 7-10) | **Claude Code object-style hooks** parser | Default model bumped to **Claude Opus 4.7 + GPT-5.5** | REPL input hardening: multi-line wrap no longer duplicates, Cmd+V multi-line paste no longer auto-submits, CJK chars at wrap boundary render correctly | CI workflow added | Credits: [@GO-player-hhy](https://github.com/GO-player-hhy) (#186), [@Jxy-yxJ](https://github.com/Jxy-yxJ) (#171), [@GetIT-Sunday](https://github.com/GetIT-Sunday) (#216 partial)

> **v0.4.4** (2026-04-20) — **`/setup` no longer forces Bearer mode for Anthropic + custom URL** (fixes ModelScope / Claude-Code proxies like `code.newcli.com`) | Provider-aware proxy URL hints in `/setup` (OpenRouter / DeepSeek / DashScope / ModelScope / ...) | Stale state no longer leaks across provider switches | Custom base URL preserved across `/setup` re-runs | LlmReview falls back to configured reviewer when executor guesses a wrong model | Fixes #158, #162

> **v0.4.3** (2026-04-17) — **Third-party Anthropic-compat proxy support** (Bedrock etc.) — skip beta flags that proxies reject | Propagate custom base URL to `anthropic` provider (not just `anthropic-compat`) | Credit [@screw-44](https://github.com/screw-44)

> **v0.4.2** (2026-04-17) — **Auto-compaction corruption fix** (no more empty streams after skill runs) | Compaction summary preserved on OpenAI-compat executors | Custom executor base URL now applied after mid-launch setup | Shell-provided API keys no longer erased on launch | `EXECUTOR_BASE_URL` trim + empty handling

> **v0.4.1** (2026-04-15) — Reviewer/executor retries (429, 5xx, network) | Stale interrupt flag fix | Fresh HTTP client per reviewer call | Verbose error chains
>
> **v0.4.0** (2026-04-15) — **Plan mode** (`/plan`) | Cooperative Ctrl+C interrupt | API errors no longer exit REPL | Tool output folding | 62 skills synced
>
> <details><summary>Previous versions</summary>
>
> **v0.3.9** (2026-04-11) — Proxy/custom base URL | Local models (LM Studio/Ollama) | Research Wiki | Meta-Optimize | Atomic sessions | Bash safety | Windows (experimental)
>
> **v0.3.5** (2026-04-08) — Research Wiki | Meta-Optimize self-evolution | Atomic session writes | Bash safety | Windows support
>
> **v0.3.3** (2026-04-04) — Fix all config loading crashes for Claude Code hooks compatibility
>
> **v0.3.0** (2026-04-03) — Multi-file memory index | Rich task system (TodoWrite) | `/plan` | Security hardening
>
> **v0.2.2** (2026-04-03) — `/plan` step-by-step planning | `/tasks` persistent tracking
>
> **v0.2.1** (2026-04-03) — Persistent Memory | Kimi K2.5 multi-turn fix | CJK cursor fix
>
> **v0.2.0** (2026-04-02) — Open source | Kimi + MiniMax + GLM | Smart LlmReview routing | CI/CD
>
> **v0.1.0** (2026-04-02) — Initial release | Multi-executor & reviewer | 42 bundled skills
>
> </details>
>
> [Full Changelog →](CHANGELOG.md)


---

## ✨ What is ARIS-Code?

**ARIS-Code** (*Auto Research in Sleep*) is a terminal-based AI research assistant built for academic researchers. Its core philosophy:

- 🤖 **Executor**: The primary LLM — writes code, surveys literature, drafts papers, plans experiments
- 🔍 **Reviewer**: An independent LLM that adversarially critiques the Executor's output via the `LlmReview` tool
- 🔄 **Iterate**: Executor writes → Reviewer critiques → Executor revises → loop until quality converges

With **42 bundled research skills**, ARIS covers the full pipeline from idea discovery to paper submission.

---

## 🚀 Installation

**macOS (Apple Silicon)**
```bash
curl -fsSL https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/releases/latest/download/aris-code-darwin-arm64.tar.gz | tar xz
sudo mv aris /usr/local/bin/aris
```

**macOS (Intel)**
```bash
curl -fsSL https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/releases/latest/download/aris-code-darwin-x64.tar.gz | tar xz
sudo mv aris /usr/local/bin/aris
```

**Linux (x64)**
```bash
curl -fsSL https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/releases/latest/download/aris-code-linux-x64.tar.gz | tar xz
sudo mv aris /usr/local/bin/aris
```

**Windows (x64)**
Download [`aris-code-windows-x64.zip`](https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/releases/latest/download/aris-code-windows-x64.zip), extract, and run `aris.exe` in PowerShell or Windows Terminal.

> Run `aris` to start. First launch triggers the interactive setup wizard.

---

## ⚙️ First-Run Setup

The first time you run `aris`, an interactive setup wizard launches automatically:

```
🌙 ARIS-Code Setup Wizard

[1/3] Choose Executor provider (primary LLM)
  > Anthropic Claude
    OpenAI GPT
    Google Gemini
    Zhipu GLM
    MiniMax
Enter API Key: sk-...

[2/3] Choose Reviewer provider (adversarial LLM)
  > OpenAI GPT
    Google Gemini
    Zhipu GLM
    MiniMax
Enter API Key: sk-...

[3/3] Choose language preference
    中文 (CN)
  > English (EN)

✅ Config saved to ~/.config/aris/config.json
```

After setup you drop straight into the REPL. Run `/setup` at any time to reconfigure without restarting.

---

## 🤖 Supported Providers

| Provider | As Executor | As Reviewer | Key Models |
|----------|:-----------:|:-----------:|-----------|
| 🟣 Anthropic Claude | ✅ | — | claude-opus, claude-sonnet, claude-haiku |
| 🟢 OpenAI | ✅ | ✅ | gpt-5.4, gpt-5.4-mini, gpt-5.4-nano |
| 🔵 Google Gemini | ✅ | ✅ | gemini-2.5-pro, gemini-2.5-flash |
| 🔶 Zhipu GLM | ✅ | ✅ | GLM-5, GLM-5-Turbo |
| 🔷 MiniMax | ✅ | ✅ | MiniMax-M2.7, MiniMax-M2.7-highspeed |

> **Design note**: Anthropic Claude is Executor-only; all other providers can serve as both Executor and Reviewer. The classic pairing is **Claude Executor + GPT/GLM Reviewer** for true adversarial multi-agent research.

---

## 🎯 Key Features

### 1. 🔄 Adversarial Multi-Agent Architecture

```
User input
    ↓
[Executor LLM]  ──── calls ────→  LlmReview Tool
  write / code                         ↓
  research / analyze             [Reviewer LLM]
    ↑                             independent critique
    └──────── review feedback ───┘
              iterate until quality target met
```

**LlmReview in action**:

```
❯ Please review this paper for me
# ARIS reads the paper, calls LlmReview to get GPT-5.4/GLM-5/MiniMax's
# independent assessment — multi-round adversarial dialogue ensues

❯ Use LlmReview to say hello to the reviewer
# Direct LlmReview tool invocation
```

### 2. 📚 42 Bundled Research Skills

Use `/skills` to list all available skills:

```
/research-lit        — Literature search & survey
/idea-discovery      — Full idea discovery pipeline
/research-review     — GPT xhigh deep review
/paper-write         — LaTeX paper drafting
/paper-compile       — Paper compilation & error fixing
/auto-review-loop    — Autonomous multi-round review loop
/experiment-plan     — Experiment roadmap generation
/run-experiment      — Remote GPU deployment
/peer-review         — Conference reviewer simulation
/rebuttal            — Submission rebuttal generation
...  (42 total)
```

**Three-tier skill priority** (higher overrides lower):
```
~/.config/aris/skills/   [user custom — highest priority]
~/.claude/skills/        [Claude Code compatible]
bundled skills           [42 out-of-the-box skills]
```

### 3. 🖥️ REPL Commands

| Command | Description |
|---------|-------------|
| `/help` | List all commands |
| `/model` | Switch Executor model |
| `/reviewer` | Switch Reviewer model |
| `/permissions` | Toggle permission mode (allow / deny / ask) |
| `/setup` | Reconfigure without restarting |
| `/skills` | List / show / export skills |
| `/status` | Show current configuration |
| `/cost` | Token usage & cost summary |
| `/compact` | Compress conversation history |
| `/clear` | Clear the screen |
| `/version` | Version info |
| `/research-review` | Invoke research review skill directly |
| `/paper-write` | Invoke paper writing skill directly |
| `...` | All 42 skill slash commands |

### 4. 🌐 Language Preference

Your chosen language (CN/EN) is injected into the system prompt so ARIS always responds in your preferred language — no per-message configuration needed.

### 5. 🛡️ Anti-Hallucination Design

The system prompt explicitly informs the model of its exact identity (ARIS-Code), preventing role confusion in multi-agent scenarios where the Executor and Reviewer are different models from different providers.

---

## 📖 Usage Examples

### Literature Survey
```
❯ /research-lit find the latest work on diffusion models for protein design
```

### Autonomous Review Loop
```
❯ /auto-review-loop
# ARIS reads the paper in the current directory and runs:
# draft → review → revise → review → ... until quality converges
```

### Switch Executor Model
```
❯ /model
  Current Executor: claude-sonnet-4-5
  Switch to:
  > claude-opus-4
    gpt-5.4
    gemini-2.5-pro
```

### Switch Reviewer
```
❯ /reviewer
  Current Reviewer: gpt-5.4
  Switch to:
  > glm-5
    gemini-2.5-pro
    minimax-m2.7
```

### Direct Adversarial Review
```
❯ Review my method section — be brutal
# Executor reads the section, calls LlmReview,
# receives an independent adversarial critique, and iterates
```

---

## 📁 Configuration

```
~/.config/aris/
├── config.json        # Main config (provider, API keys, language)
└── skills/            # Custom user skills (override bundled skills)
```

**Example config.json**:
```json
{
  "executor": {
    "provider": "anthropic",
    "model": "claude-sonnet-4-5",
    "api_key": "sk-ant-..."
  },
  "reviewer": {
    "provider": "openai",
    "model": "gpt-5.4",
    "api_key": "sk-..."
  },
  "language": "EN"
}
```

---

## 🔌 MCP servers

> ✅ **Live since v0.4.17**: stdio MCP servers configured in
> `settings.json` are spawned at startup, their tools are advertised to
> the model as `mcp__<server>__<tool>`, and calls dispatch end-to-end —
> on both Anthropic and OpenAI-family executors.

```jsonc
// <config_home>/settings.json  (config_home = $CLAUDE_CONFIG_HOME or ~/.claude)
{
  "mcpServers": {
    "codex": {
      "type": "stdio",
      "command": "codex",
      "args": ["mcp-server"],
      "trust": true,              // optional: skip per-call approval
      "requestTimeoutSecs": 240   // optional: per-server timeout
    }
  }
}
```

The easiest way to set this up is `aris setup` → reviewer option 10
(Codex MCP), which writes the entry for you. Notes:

- MCP servers are **external processes the sandbox does not cover** —
  untrusted MCP tools therefore prompt for approval on every call (even
  under `danger-full-access`) until you set `trust: true` or choose
  "always for this server" in-session.
- A server that fails to start is skipped with a warning; the rest keep
  working. `aris doctor` shows per-server status (spawned / initialized /
  tool count / failures / trust).
- Transport is newline-delimited JSON-RPC per the MCP spec; legacy
  `Content-Length:`-framed servers are still accepted on the read side.
- Adding a new server requires an `aris` restart to spawn + discover
  (ARIS prints a notice when this applies). Subagents do not receive
  MCP tools in this release.

---

## 🗺️ Roadmap

- [x] Phase 0: Rust fork foundation (based on claw-code)
- [x] Phase 1: Multi-provider support (Anthropic / OpenAI / Gemini / GLM / MiniMax)
- [x] Phase 1: LlmReview adversarial critique tool
- [x] Phase 1: 42 bundled research skills
- [x] Phase 1: Language preference & anti-hallucination system prompt
- [ ] Phase 2: Skills system polish (three-tier priority UI)
- [ ] Phase 2: Web UI dashboard
- [ ] Phase 3: Linux / Windows support
- [ ] Phase 3: Local model integration (Ollama)

---

## 🙏 Credits & Acknowledgements

**ARIS-Code is built on the excellent foundation of [claw-code](https://github.com/ultraworkers/claw-code).**

claw-code is an open-source Rust reimplementation of Claude Code. It provided the REPL framework, tool-calling infrastructure, and cross-platform compilation that made ARIS-Code possible. Huge thanks to the ultraworkers team for their outstanding work!

- 🔗 claw-code: https://github.com/ultraworkers/claw-code
- 🔗 ARIS-Code: https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep

---

## 📄 License

MIT License © 2025 ARIS-Code Contributors

---

<div align="center">
  <sub>🌙 Let AI do research while you sleep · Built with ❤️ and Rust</sub>
</div>

