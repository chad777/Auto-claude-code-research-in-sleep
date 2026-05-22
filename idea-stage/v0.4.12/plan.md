# v0.4.12 实施 plan v2 (2026-05-22, post-codex round-1)

类型：bug-fix + small-feature release。比 v0.4.11 (skills sync) 范围小很多。

> **v2 修订**: 按 codex round-1 (gpt-5.5 xhigh) GO-WITH-CAUTION verdict 的 8 个 finding 重写：
> 1. #238 还要在 `config.rs:555` 加 `strictMode` 解析；strict 模式必须忽略 5 个 override 全部不只 enabled
> 2. (同上)
> 3. 批 3 顺序：先 sync 再 patch (避免 rsync --delete 覆盖)；路径校正 `crates/runtime/assets/skills/`
> 4. #232 当前默认是 `deepseek-chat` 不是 `deepseek-reasoner`；要改的是 provider 表里 `deepseek-reasoner` legacy ID + deprecation 警告
> 5. P2 `has_word("qwen")` 漏 `qwen3.6-plus` (`qwen` 后跟数字 `3`, has_word 不把数字当边界)。把数字加进边界字符 OR 用 prefix-match
> 6. P1.A 保留 `events_emitted`；新增 `has_emitted_meaningful_content` 只用于 retry eligibility, **不替代**任何 protocol event 已交给 caller 的语义
> 7. P1.B 还要同步 reviewer path 的 `reviewer_supports_reasoning_effort` (`tools/lib.rs:3632`) — 两份镜像逻辑必须一致改
> 8. P1.C 严格 matcher: 不能 `body.contains("stream_options")`；要求 `status == 400` + JSON `error.param == "stream_options"` 或 error message 同时含 `stream_options` + (`unknown` | `unrecognized` | `extra` | `additional` | `unsupported`) keyword

## 范围确认

✅ **做**：
- P0 #238 sandbox `strict_mode`
- P0 #232 deepseek-reasoner reviewer-model gate (SKILL.md fix)
- P1.A Anthropic stream retry coverage
- P1.B o-series reasoning effort `has_word`
- P1.C `stream_options.include_usage` proxy fallback
- P2 pricing substring → `has_word`
- v0.4.11 follow-up: build.rs `starts_with("skills-codex")`
- v0.4.11 follow-up: CI fetch-depth: 0 + origin/main fetch
- Skills sync 追新 (1 个新 skill `/interview-cheatsheet`)

❌ **推 v0.4.13**：
- P1.D per-server MCP timeout (medium, multi-crate)
- JSON-RPC notifications id-skip (medium, mcp_stdio.rs 重构)
- meta_opt hook init-time deploy (medium, 跨 CLI/runtime)
- #240 README 章节层级 (small-medium, 文档大改)

## 具体改动点

### 批 1 — Streaming + Pricing 小修 (估 1.5h)

| Fix | File | Line | 改法 |
|---|---|---|---|
| P1.A | `crates/api/src/client.rs` | 668-770 | **保留 `events_emitted: usize`**（codex finding #6）。新增 `has_emitted_meaningful_content: bool`，定义跟 OpenAI `nothing_emitted_yet()` 一致：text **非空** / completed tool_use / non-empty reasoning。retry guard 改用此字段。`events_emitted` 保留作其他语义 |
| P1.B (executor) | `crates/aris-cli/src/openai_executor.rs` | 37-58 | `supports_reasoning_effort` 用 word-boundary helper（共享 helper 跟 `usage.rs:has_word` 一致）。重复 line 316-317 也换成调 helper |
| **P1.B (reviewer)** ⚠️ codex finding #7 | `crates/tools/src/lib.rs:3625-3645` | 同步 `reviewer_supports_reasoning_effort` 跟 executor 改法一致，否则 LlmReview path 仍然对 `openai/o3-mini` 判断错 |
| P1.C | `crates/aris-cli/src/openai_executor.rs` | 284-287, 398 | 加 `stream_options_400_error()` helper：`status == 400` 且 (JSON `error.param == "stream_options"` OR error message 同时含 `stream_options` + (`unknown` \| `unrecognized` \| `extra` \| `additional` \| `unsupported`) keyword)。命中时 retry 一次去掉 `stream_options` 字段。加单元测试覆盖 4 种 proxy 错误格式 |
| P2 | `crates/runtime/src/usage.rs` | 197-209, 251-? | `has_word` 边界字符扩到 `[-_/:0-9]`（codex finding #5：让 `qwen3.6-plus` 里 `qwen` 后的 `3` 算边界）。或者保留 `contains` 但加 unit test 覆盖 `qwen3.6-*` / `kimi-k2.5` / `glm-4-plus` 等真实模型名 |

### 批 2 — Sandbox #238 (估 1.5h, codex finding #1 + #2 expanded)

| Fix | File | Line | 改法 |
|---|---|---|---|
| #238-1 struct | `crates/runtime/src/sandbox.rs` | 28-? | `SandboxConfig` 加 `pub strict_mode: Option<bool>` |
| **#238-2 config 解析** ⚠️ codex finding #1 | `crates/runtime/src/config.rs:555-575` | `parse_optional_sandbox_config()` 加读 `strictMode: optional_bool(sandbox, "strictMode", ...)` 字段到 SandboxConfig 构造 |
| #238-3 resolve_request | `crates/runtime/src/sandbox.rs` | 87-104 | strict 分支：**忽略所有 5 个 override**（enabled / namespace / network / filesystem / allowed_mounts）codex finding #2 |
| #238-4 tool schema desc | `crates/tools/src/lib.rs:65-81` | `dangerouslyDisableSandbox` 字段加 description: "Sandbox-bypass request from the LLM. **Honored only when** runtime config has `sandbox.strictMode != true`. Setting `true` under strictMode is ignored and emits a warning." |
| #238-5 stderr warning | `crates/runtime/src/bash.rs:172-185` | 当 strict + LLM 传 disable=true 不一致 → 打 stderr warning（一次性 per-session, OnceLock） |
| #238-6 doctor | `crates/aris-cli/src/main.rs` (Doctor) | 加一栏 "Sandbox: strict (config) / permissive (config) / default-allow"  显示当前生效配置 |
| #238-7 docs | README 双语 + CHANGELOG | sandbox 优先级文档化 (strict > config > LLM-override) |
| #238-8 unit tests | `crates/runtime/src/sandbox.rs` `mod tests` | 加 4 个 test: (a) strict + LLM 传 disable → 仍 enabled; (b) strict + LLM 传所有 5 个 override → 全 ignore; (c) non-strict (default) + LLM 传 override → 兼容旧行为; (d) config.strictMode 解析 |

### 批 3 — Skills + SKILL.md fixes (估 30min, codex finding #3 + #4)

**顺序硬约束** (codex #3): 必须**先 sync 再 patch** SKILL.md，否则 `sync_main_skills.sh` 用 `rsync --delete` 会覆盖手改:

1. **先**: `bash tools/sync_main_skills.sh` 把 main `ed638f3..HEAD` 的改动同步进来（含 `/interview-cheatsheet`）
2. **后**: patch `crates/runtime/assets/skills/auto-review-loop-llm/SKILL.md` (路径校正, codex finding #3) 

#232 的具体改动 (codex finding #4 修正：当前 SKILL.md 默认是 `deepseek-chat` 不是 `deepseek-reasoner`):

| Fix | 改法 |
|---|---|
| #232-1 provider 表 | `crates/runtime/assets/skills/auto-review-loop-llm/SKILL.md:49` 把 DeepSeek 行 `deepseek-chat, deepseek-reasoner` 改成 `deepseek-v4-flash, deepseek-v4-pro`，注脚说明 `deepseek-chat / deepseek-reasoner` 2026-07-24 deprecate |
| #232-2 capability note | 同文件加段说明 reasoning model (R1 类、reasoner 后缀) 当前不接受 `tool_choice` parameter, 用 `*-flash` / `*-chat` / `*-pro` 系列 |
| **#232-3 同步 main 分支 SKILL.md** | 同样改 main 分支 `skills/auto-review-loop-llm/SKILL.md` 推 PR，保证下次 sync 不回滚 |

**v0.4.11 sync 推到 v0.4.12 的 skills 追新** (跟 #232 同批): 只新增 `/interview-cheatsheet`, sync 脚本自动处理 — 不需要其他改动。

### 批 4 — build.rs glob + CI fetch-depth (估 30min)

| Fix | File | 改法 |
|---|---|---|
| v0.4.11.follow.A | `crates/runtime/build.rs:12-16` | `EXCLUDED_SKILL_PREFIXES.contains(&name)` 改成 `name.starts_with("skills-codex")` |
| v0.4.11.follow.B | `.github/workflows/ci.yml` | actions/checkout 加 `fetch-depth: 0` + 显式 `git fetch origin main:refs/remotes/origin/main` step，让 drift test 1 ancestor check 真生效 |

### 批 5 — Release (估 30min)

- Cargo.toml workspace.package.version: 0.4.11 → 0.4.12
- CHANGELOG.md prepend v0.4.12 section
- README.md / README_CN.md aris-code banner: 加 v0.4.12 entry
- commit + annotated tag + push
- Codex 5 轮 review per v0.4.10/v0.4.11 模式

## 测试策略 (codex finding 加强)

### 新增 targeted unit tests (codex 建议)

| Module | Test | 验证什么 |
|---|---|---|
| `sandbox.rs` mod tests | strict_mode_overrides_llm_disable | strict + LLM disable=true → still enabled |
| `sandbox.rs` mod tests | strict_mode_ignores_all_overrides | strict + 5 个 override → 全 ignore |
| `sandbox.rs` mod tests | default_mode_honors_llm_disable | non-strict + LLM disable=true → 兼容旧行为 |
| `config.rs` mod tests | parse_sandbox_strict_mode | merged settings 含 `"sandbox": {"strictMode": true}` 正确解析 |
| `usage.rs` mod tests | pricing_qwen36_plus_classification | `qwen3.6-plus` / `kimi-k2.5` / `glm-4-plus` / `deepseek-v4-flash` 等真实模型名都路由正确 tier |
| `openai_executor.rs` mod tests | has_word_o_series | `o3-mini` / `openai/o3-mini` / `provider/o4` 都 hit; `o3-tuned-custom` 边界正确 |
| `openai_executor.rs` mod tests | stream_options_400_proxy_fallback | 4 种 proxy 错误 body 都能正确 detect; 普通 400 不误判 |
| `client.rs` mod tests | anthropic_has_emitted_meaningful_content | 空 text_delta 不算 meaningful; MessageStart 不算; 非空 text_delta / completed tool / non-empty reasoning 才算 |
| `tools/lib.rs` mod tests | reviewer_supports_reasoning_effort_provider_prefix | 跟 executor 一致 |

### 整体测试

- `cargo build --release` — `Embedded 75 bundled skills, ~49 helper resources` (+1 `/interview-cheatsheet`)
- `cargo test --workspace -- --test-threads=1` (codex finding 建议跑 workspace 不只 runtime)
- `./target/release/aris doctor` — sandbox 新栏显示
- 手动 REPL: `/skills list` 看 `/interview-cheatsheet` 进来
- 手动: SandboxConfig strict + LLM disable → 看 stderr warning 一次

## 风险评估

| 风险 | 严重度 | 缓解 |
|---|---|---|
| P1.A `has_emitted_meaningful_content` 误判（某些 text_delta 是空的）| 中 | 跟 OpenAI 路径 (`nothing_emitted_yet()`) 一致：text/tool/reasoning **非空** 才算 meaningful。空 text_delta / 仅 MessageStart 不算 |
| #238 strict_mode 默认 false 还是 true | 高 | 默认 false（向后兼容旧 config）。doc 推荐用户 explicit 设 true |
| P1.C 400 detection 误命中 (其他 400 也被吃) | 中 | 严格匹配 error body 含 `stream_options` keyword |
| has_word 在 pricing 改完后某些自定义模型走错 tier | 低 | 看 unit test 现状先；不行加 model 名单测试 |
| build.rs starts_with 引入 false-exclude | 极低 | main 上没有 `skills-codex-prod/` 这种正经 skill 名字 |
| CI fetch-depth: 0 拖慢 build | 低 | aris-code 仓库 < 100MB，影响小 |

## Codex review 计划 (v3 — finding 3 处实施级修正)

| Round | 内容 |
|---|---|
| round-1 (DONE) | plan v1 → GO-WITH-CAUTION + 8 finding → plan v2 |
| round-2 (DONE) | plan v2 → GO-WITH-CAUTION + 3 finding (实施精度，非设计缺陷) → v3 |
| **round-3 (实施后)** | final diff review → 期望 GO |

### v3 修订 (round-2 的 3 个 finding)

| Round-2 Finding | 怎么修 |
|---|---|
| #1 批 3 sync 顺序卡 clean-tree | **执行顺序改成**: 批 3 sync (脚本要求 clean tree) → 批 3 patch SKILL.md → 批 1 → 批 2 → 批 4 → cargo test → 批 5 release. **把 sync 放第一步**, 这样后续代码改动不影响 sync |
| #2 #232 SKILL.md 修订不完整 | 改 3 处 (不只 provider 表 line 49)：line 36 配置默认值 `deepseek-chat` → `deepseek-v4-flash`; line 49 provider 表; line 65 MCP 示例 |
| #3 P1.A EOF after non-meaningful | retry guard 改成"`has_emitted_meaningful_content == false` 且 (parser error 或 缺 terminal 事件)" — 即使收到 MessageStart 后 EOF 也算可以 retry。补 unit test `eof_after_message_start_retries` |

### Round-2 nits 处理

- **#5 `o32` 命中 `o3`**: 不全局把数字当 boundary。改成: 只对 provider prefix (qwen/glm/kimi/minimax/doubao) 加特判 — `m.starts_with(provider) || m.contains(format!("/{}", provider))`。o-series 仍用 `has_word` 边界 `[-_/:]` 不加数字
- **#8 JSON param 前缀 match**: `error.param.starts_with("stream_options")` 不要求 exact equal (兼容 `stream_options.include_usage` 这种深层 path)
- **OnceLock per-process**: 文档 + 测试明确"per-process"而非"per-session"
- **估时调到 6-8h**: 加 main SKILL.md PR + workspace test 时间

## 执行顺序 (v3 — 调到 sync 先做)

⚠️ codex round-2 finding #1: `sync_main_skills.sh` 要求 working tree clean，所以**必须放第一步**:

1. **批 3a** (sync): 跑 `bash tools/sync_main_skills.sh` 把 `/interview-cheatsheet` + main 上 28 commits 同步进 bundle
2. **批 3b** (#232 patch + main PR): patch `crates/runtime/assets/skills/auto-review-loop-llm/SKILL.md` 3 处, 同时 main 分支也改, push PR
3. **批 1** (api/client.rs + openai_executor.rs + usage.rs + tools/lib.rs reviewer): 4 个 streaming/pricing 小修 + 镜像
4. **批 2** (sandbox.rs + config.rs + bash.rs + lib.rs schema + main.rs doctor + README): #238 strict_mode 全链路
5. **批 4** (build.rs + .github/workflows/ci.yml): v0.4.11 follow-up
6. **全量 cargo build + cargo test --workspace** 一次性验证
7. **批 5 release** + codex round-3 final diff review
