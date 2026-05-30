# v0.4.16 plan — REPL UX + Provider abstraction (zero-regression)

**Design source**: ultracode workflow `v0416-zero-regression-design` (2026-05-30, 5-agent read-only baseline + design). Full output: `design_raw.json` (96 characterization cases + per-section design).
**Organizing principle**: ZERO REGRESSION — 之前能用的 provider 配置和 REPL 操作绝对不能因重构出错。

## 核心 scope 决策 (用户 2026-05-30 认可)

**P7 降级为"分类器 + 1 个 helper 合并",不 re-route。** 调查发现 re-route pricing/reviewer 是回归藏身处:
- pricing 是顺序敏感 3-策略匹配 (contains / has_word / provider_match)，统一会 12.5x 错价 (HIGH risk)
- reviewer 跟 pricing 对 `my-kimi-clone` 有已知不一致，统一会改答案 (HIGH risk)
- config.rs anthropic+自定义 URL = #158/#162 雷区 (HIGH risk)

### v0.4.16 Track B 做什么 / 不做什么

| ✅ 做 | ❌ DEFER (危险代码逐字不动 = 不可能回归) |
|---|---|
| ProviderFamily enum 当**纯分类器** (3 variant: AnthropicNative / AnthropicCompat / OpenAiCompat)，不 re-route | config.rs `apply_to_env_inner` env-writing |
| 合并 3 个**逐字相同**的 word-boundary helper (`word_match` @ openai_executor:99 / `has_word` @ usage:283 / `reviewer_word_match` @ tools/lib:3654) → 1 个 shared | `resolve_openai_executor_config` (EXECUTOR_PROVIDER=="openai" exact) |
| **P8 subagent dispatch 修复** (真正的 provider 价值) | pricing `pricing_for_model` 链 |
| | reviewer `route_openai_compat_model` 路由 |
| | `provider_match` (usage:234, 语义跟 word_match 不同，**不能**碰) |
| | setup menu echo (base_url.contains 子串，纯 UX) |

## P8 — 三类分治 + 揪出真 bug

`build_agent_runtime` (tools/lib.rs:1885) 当前 provider-blind (无脑 AnthropicRuntimeClient)。修法: `if resolve_openai_executor_config().is_some() { OpenAI } else { 不变的 Anthropic }`。

| 类 | executor | 当前 | P8 后 | 安全性 |
|---|---|---|---|---|
| **A** | anthropic 原生 + OAuth | ✅ works (common case) | EXECUTOR_PROVIDER≠"openai" → 结构上进不了新分支，走原 else (逐字 = 今天 line 1894) | **behavior-preserving (结构保证)** |
| **B** | anthropic-compat (DeepSeek opt7 / MiniMax-compat) | ✅ 静默 works | 同样走原 else 分支 (它们也不设 EXECUTOR_PROVIDER=openai) | **behavior-preserving + regression-test + disclose** |
| **C** | OpenAI-family (openai/glm/minimax/kimi/gemini/doubao/qwen/mimo/custom) | ❌ **MissingApiKey error,或静默用 Anthropic client = 成本/账号泄漏** | 正确路由到 OpenAI executor | **纯 bug-fix** (当前 broken，无 legit 行为可破) |

**关键约束**: P8 dispatch predicate 必须**恰好是** `resolve_openai_executor_config` (EXECUTOR_PROVIDER=="openai" exact)，绝不能用更宽的 "is non-Anthropic" 测试 (否则 B 类 anthropic-compat 被误分进 OpenAI 分支而破)。复用 CLI 的 `ExecutorClient` (main.rs:4019)，**不能造第三个 client**。

## Track A — REPL 纯加法

1 个新持久化模块 + 1 个 Ctrl+R arm。**没有任何现有 arm / state field / redraw 路径被改**，全是插入。

### 持久化 (`~/.config/aris/history`)
- **格式**: 纯文本，每行一条，oldest-first (保持内存 Vec 顺序 → Up/Down 索引逐字不变)
- **LOAD**: 新方法 `LineEditor::load_history_from(path)`，run_repl 在 `LineEditor::new` 后 (main.rs:1127-1130) 调一次，启动前。读不到/损坏 → 静默空开始
- **SAVE**: hook 到**现有** `push_history` 调用 (main.rs:1170，已经只在非空非-slash 提交后跑)，加一个 disk append。append-per-entry (不是 save-on-exit，因为 Ctrl+C/D 退出会跳过 flush)
- **安全**: ① perms 0600 ② **disk-only secret-skip** (含 sk-/AUTH_TOKEN/api[_-]?key=/≥32 char token run；**只跳 disk，内存仍 push** → session 内 Up/Down 逐字不变) ③ `ARIS_NO_HISTORY` env kill-switch (load+save 都 no-op)

### Ctrl+R 反向搜索 (子模式)
- 插入 1 个 `(Char('r'), CONTROL)` arm (input.rs:287 Ctrl+W 之后)。今天 Ctrl+R 被 `_ => continue` 吞掉，加 arm 不改任何现有 arm 可达性
- 自包含子 loop (`reverse_search()`)，char-based (CJK 安全)，`(reverse-i-search)`q': match` bash 风格提示
- 键: printable→加 query 重扫 newest→oldest / Backspace→缩 query / Ctrl+R→下一个更旧 match / Enter→Accept 进 buf (不是直接 Submit，用户再按 Enter 提交) / Esc/Ctrl+C/Ctrl+G→Cancel 还原
- **绝不**在子 loop 里 toggle raw mode / bracketed paste (跑在 read_line 的 lifecycle 内)
- 非-TTY: Ctrl+R 不可达 (read_line_fallback 不进 raw loop)

### must_not_change → 不变量 (10 项逐条锁，见 design_raw.json)
bracketed paste / CJK 宽度 / 补全 dropdown / dropdown-优先-history gating / 每个现有 KeyCode arm / history 状态机 (saved_buf/idx) / 非-TTY fallback / select_menu / run_repl 控制流。**现有 input.rs unit tests (normalize_paste/layout/dropdown/push_history_blank) 必须保持绿。**

## 零回归机制 (回答"能保证绝对安全么")

**诚实**: 没有软件改动能数学 100% 证明，但残余风险压到接近零:
- **Track B**: characterization-test-first (96 case 锁当前行为，前后都绿) + scope 纪律 (危险代码逐字不动) + word_match 合并是可证明真值不变 (3 函数已逐字相同)。**Anthropic 原生路径零代码触及。**
- **Track A.1 (P8)**: A/B 类结构上进不了新分支 (EXECUTOR_PROVIDER≠"openai") + regression-test 锁；C 类当前 broken 无行为可破
- **Track A.2 (REPL)**: 全是插入；secret-skip + ARIS_NO_HISTORY 只作用 disk I/O，内存历史不变 → session 内行为逐字相同

## Risk register (高危项 + 缓解，全文见 design_raw.json)

| 严重 | 风险 | 缓解 |
|---|---|---|
| HIGH | pricing re-route → 重排顺序敏感链 → 12.5x 错价 | **DEFER**，pricing 逐字不动 |
| HIGH | reviewer re-route → 改 my-kimi-clone 答案 | **DEFER**，route_openai_compat_model 逐字不动 |
| HIGH | config.rs 合并 anthropic+custom-URL → x-api-key 翻成 Bearer (#158/#162 复发) | **不 delegate** config.rs env-writing；exec_anthropic_custom_url_keeps_xapikey 测试锁 (最高优先 guard) |
| HIGH | P8 用宽 predicate → anthropic-compat subagent 被误路由破 | dispatch predicate **恰好** resolve_openai_executor_config；subagent_anthropic_compat_works 锁 |
| MEDIUM | word_match 合并误改 boundary set (-_/:) | 逐字提取 (已相同)；reasoning_o3/price_o3/reviewer_word_match 测试前后跑 |
| MEDIUM | Ctrl+R 子 loop 留下错 cursor_row → 视觉 corrupt | 子 loop 清自己的 prompt 行 + cursor 归 0 列；**人工 TTY smoke test** (最难单测的面) |
| MEDIUM | 持久化把 secret 写进 disk | disk-only secret-skip + 0600 + ARIS_NO_HISTORY (best-effort，disclose 可能漏新格式) |

## 实现阶段

- **Phase 0 (共享前置，两 track 都 block 在此)**: 把 96 个 characterization 测试写成 Rust `#[test]` 对**当前代码**跑绿 = **回滚锚点**。分布: config.rs apply_to_env + openai_executor predicates → aris-cli；pricing → runtime/usage.rs；reviewer + subagent A/B/C → tools/lib.rs；REPL gaps → input.rs (多数已有)。`cargo test` 全绿才许动生产代码。**commit = rollback anchor。**
- **Phase 1**: Track A (REPL: input.rs + 新 history 模块 + main.rs 1127-1170) ∥ Track B (provider: word_match 合并 + ProviderFamily 分类器)。零文件重叠 (main.rs 只碰不相交两行段)。
- **Phase 2**: P8 subagent dispatch (tools/lib.rs:1885，复用 CLI ExecutorClient)。
- **Phase 3**: 整合 → 全 matrix 100% 绿 → codex 审 (设计 + 矩阵完整性 + 最终 diff) → **等用户本地测过才 push** (branch first)。

## 不引新依赖
不用 rustyline/reedline (v0.4.7 刚删 rustyline)，REPL 在现有 crossterm 层加；Ctrl+R / 持久化零新 crate。
