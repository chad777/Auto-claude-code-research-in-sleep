# SPIKE-A — RW4 tokio 嵌套 runtime 排雷

> v0.4.17 Phase 1 最大已知雷。只读调查,HEAD=81e5652 (v0.4.16 release)，所有 file:line 实查。
> 调查者: SPIKE-A subagent · 日期 2026-06-05

## 结论(一句话)

**(a) 成立 — `ToolExecutor::execute` 的调用栈不在任何 tokio runtime 上下文内。** `CliToolExecutor` 持有 `McpServerManager` 后，在同步 `execute` 里用「独立 `current_thread` runtime + `block_on`」桥接 async API 是**当前入口拓扑下安全的**(codex R3 限定措辞：此结论绑定 `fn main()` 非 tokio 的现状；若未来有人从已有 tokio runtime 内调用 `ConversationRuntime::run_turn`，`block_on` 仍会 panic)，与 `bash.rs:103` 现有先例完全同构。**无需** `block_in_place`、无需后台线程 + channel。

唯一的 panic 风险来自实现细节(在已有的 `block_on` 闭包**内部**再调 MCP)，而不是来自架构 —— 只要把 manager 的同步句柄放在 `CliToolExecutor::execute` 这一层(它在 `block_on` 闭包外、纯同步上下文)，就天然规避。**Phase 1 实现加固(codex R3-P2.2)**：在 MCP 同步句柄的 `block_on` 入口前加 `debug_assert!(tokio::runtime::Handle::try_current().is_err(), ...)` 或等效注释约束，把「不得在 tokio 上下文内调用」钉进代码而不是只留在本文档。

---

## 1. CLI 的 runtime 拓扑:没有进程级 runtime

- `crates/aris-cli/src/main.rs:168` `fn main()` —— **不是** `#[tokio::main]`。它只调 `run()`(`main.rs:179` `fn run() -> Result<...>`，纯同步)。
- 全仓库 grep `#[tokio::main]` / `Handle::current` / `.enter()` 在 Rust 源码中**零命中**(唯一匹配在 `crates/runtime/assets/skills/serverless-modal/SKILL.md` 的 Python 示例，无关)。
- `tokio` 的 `rt-multi-thread` feature 虽在 `crates/aris-cli/Cargo.toml:25` / `crates/runtime/Cargo.toml:14` 开着，但**从未在进程根安装多线程 runtime**。每个 runtime 都是某个 `block_on` 调用点局部 `Runtime::new()` / `Builder::new_current_thread()` 构建、用完即 drop。

**拓扑总图**(同步主干 → 局部 block_on 孤岛):

```
main() [sync]                                   main.rs:168  非 #[tokio::main]
└─ run() [sync]                                 main.rs:179
   └─ REPL/Prompt 循环 [sync]
      └─ Cli::run_turn() [sync]                 main.rs:1512   无 block_on 包裹
         └─ ConversationRuntime::run_turn() [sync fn]   conversation.rs:195
            ├─ self.api_client.stream(request)?         conversation.rs:237
            │  └─ ExecutorClient::stream
            │     └─ self.runtime.block_on(async {…})   main.rs:4152  ← runtime 仅在此闭包内活
            │        └─ 闭包 return Vec<AssistantEvent>  main.rs:4332  ← block_on 在此返回
            │                                              ↑ runtime 上下文到此结束
            └─ for 工具 dispatch [sync, block_on 已退出]  conversation.rs:261-282
               ├─ permission_policy.authorize()         conversation.rs:262-267
               ├─ hook_runner.run_pre_tool_use()        conversation.rs:271
               ├─ self.tool_executor.execute()  ← T3/RW4 拦截点  conversation.rs:282
               │  └─ CliToolExecutor::execute() [sync]  main.rs:4889
               │     └─ execute_tool() [sync]           tools/lib.rs:410
               │        └─ run_bash → execute_bash      tools/lib.rs:412 → bash.rs:67
               │           └─ Builder::new_current_thread().build()  bash.rs:103
               │              └─ runtime.block_on(execute_bash_async)  bash.rs:104  ← 同款桥接,已上线
               └─ hook_runner.run_post_tool_use()       conversation.rs:288
```

关键:**`block_on`(`main.rs:4152`)与工具 dispatch(`conversation.rs:282`)是顺序兄弟，不是嵌套。** `stream()` 把整段流式收成 `Vec<AssistantEvent>`(含 `ToolUse` 事件)后 runtime 即 drop，控制权回到同步的 `run_turn` 循环体，再由它发起工具 dispatch。

---

## 2. 证据链(逐条 file:line)

### 2.1 `run_turn` 是同步 fn，dispatch 在同步循环里
- `crates/runtime/src/conversation.rs:195` `pub fn run_turn(&mut self, …) -> Result<TurnSummary, RuntimeError>` —— **sync**(非 `async fn`)。
- `crates/runtime/src/conversation.rs:54-55` `pub trait ToolExecutor { fn execute(&mut self, …) -> Result<String, ToolError>; }` —— **sync trait 方法**。
- 工具 dispatch 点 `conversation.rs:261-282`:`for (…) in pending_tool_uses { … self.tool_executor.execute(&tool_name, &input) … }`。这段在 `run_turn` 的同步 `loop`(`conversation.rs:220`)体内，`stream()` 调用(`conversation.rs:237`)之后。

### 2.2 `block_on` 闭包在工具 dispatch 之前已返回(Anthropic 路径)
- `crates/aris-cli/src/main.rs:4152` `self.runtime.block_on(async {` —— `AnthropicRuntimeClient::stream`(`ApiClient::stream` impl，`main.rs:4115`)。
- 闭包体只做「流式拉事件 → 收进 `Vec<AssistantEvent>`」，`ToolUse` 在 `main.rs:4255` 被 push 进 events(只存数据，不执行工具)。
- 闭包在 `main.rs:4320 return Ok(events)` / `main.rs:4331 response_to_events(...)` 返回，`})` 收尾于 **`main.rs:4332`**。`block_on` 到此结束，runtime 释放。
- 函数返回 `Vec<AssistantEvent>`(`main.rs:4115` 签名 `-> Result<Vec<AssistantEvent>, RuntimeError>`)。**没有任何工具执行发生在 runtime 上下文内。**

### 2.3 CLI 层 `run_turn` 调用点无 block_on 包裹
- `crates/aris-cli/src/main.rs:1512` `Cli::run_turn` → `main.rs:1521` `self.runtime.run_turn(input, Some(&mut permission_prompter))` —— 直接同步调用，外层只有 `Spinner`(`main.rs:1513`)等纯同步代码。
- 其他 4 个 `run_turn` 调用点(`main.rs:1573` run_prompt_json / `main.rs:2647` / `main.rs:251` run_turn_with_output / 各 slash 派生)同样是裸同步调用。
- `main.rs` 里**仅有的两个 `block_on`**:`main.rs:701`(OAuth 一次性交换，与会话循环无关)和 `main.rs:4152`(上述 stream 内部)。两者都不在 `run_turn` 的外层栈上。

### 2.4 `CliToolExecutor::execute` 本体是纯同步
- `crates/aris-cli/src/main.rs:4888` `impl ToolExecutor for CliToolExecutor` / `main.rs:4889 fn execute(&mut self, …)`：解析 JSON → `main.rs:4901 match execute_tool(tool_name, &value)` → 渲染输出。**全程同步，无 tokio。**

### 2.5 决定性先例:Bash 工具已在同一栈深做 sync→async block_on 桥接
- 链路:`CliToolExecutor::execute`(`main.rs:4889`)→ `tools::execute_tool`(`tools/lib.rs:410`)→ `"bash" => … run_bash`(`tools/lib.rs:412`)→ `run_bash`(`tools/lib.rs:442`)→ `execute_bash`(`tools/lib.rs:443` 调用 / 定义 `crates/runtime/src/bash.rs:67`)。
- `crates/runtime/src/bash.rs:103` `let runtime = Builder::new_current_thread().enable_all().build()?;` / `bash.rs:104 runtime.block_on(execute_bash_async(...))`。
- **意义**:Bash 工具每次调用都在与 MCP manager **完全相同的调用栈深度**(`ToolExecutor::execute` → `execute_tool`)新建 `current_thread` runtime + `block_on`。若该栈已在 tokio 上下文内，Bash 会**每次必 panic** "Cannot start a runtime from within a runtime" —— 而它在 v0.4.x 全程稳定运行。**这是 (a) 成立的运行时实证。**

### 2.6 最深嵌套(subagent)也不向上泄漏 tokio 上下文
- `crates/tools/src/lib.rs:2235` `impl ToolExecutor for SubagentToolExecutor` / `lib.rs:2244 execute_tool(...)` —— subagent 直调 `execute_tool`(plan.md T3/T6 据此把 MCP 拦截放 executor 层而非 `execute_tool` 内)。
- subagent 的 ApiClient `crates/tools/src/lib.rs:2129` `self.runtime.block_on(async {…})` 同样只包流式，返回 `Vec<AssistantEvent>`。
- 即便父 `run_turn` → 父工具 dispatch → subagent → subagent 的 `run_turn` → subagent stream `block_on`，每层 `block_on` 都「进-收事件-出」，**runtime 不跨 dispatch 边界存活**。无任意层会让父或子的 `execute` 跑在 tokio 上下文里。

### 2.7 全仓库 block_on / runtime 构建审计(确认无遗漏的根 runtime)
`grep block_on|Runtime::new|new_current_thread|new_multi_thread|spawn_blocking|block_in_place|Handle::current` 命中分类:
- `crates/aris-cli/src/main.rs`:`700-701`(OAuth)、`4075/4091`(Anthropic stream runtime)、`4152`(stream block_on)。
- `crates/runtime/src/bash.rs:103-104`:Bash 桥接(先例)。
- `crates/tools/src/lib.rs:2085/2097/2129`:subagent stream runtime + block_on;`4429`(测试)。
- `crates/runtime/src/mcp_stdio.rs:1365+` 的所有 `Builder::new_current_thread()` + `block_on`:**全在 `#[cfg(test)]` 测试块内**(`mcp_stdio.rs:999 use tokio::runtime::Builder;` 在 `mod tests` 里)。生产 manager API 全是 `async fn`,自身不建 runtime。
- 结论:没有任何「进程级 / 长生命周期 / 包住 run_turn」的 runtime。所有 runtime 都是 block_on 局部孤岛。

---

## 3. 推荐:manager 同步句柄的形态(RW4 落地)

### 3.1 句柄形态 — 谁持有 runtime

`McpServerManager` 的关键 API 都是 **`&mut self`**:
- `crates/runtime/src/mcp_stdio.rs:359` `pub async fn discover_tools(&mut self)`
- `crates/runtime/src/mcp_stdio.rs:433` `pub async fn call_tool(&mut self, …)`
- `crates/runtime/src/mcp_stdio.rs:472` `pub async fn shutdown(&mut self)`

且 manager 内部 `servers: BTreeMap<String, ManagedMcpServer>`(`mcp_stdio.rs:313`)持有子进程的 `ChildStdin`/`BufReader<ChildStdout>`(非可共享的 IO 半边)。因此:

**推荐形态 = 在 tools/CLI 侧新建一个同步包装器(暂名 `McpSyncHandle`),它独占持有:**
1. 一个 **`tokio::runtime::Runtime`**(用 `Builder::new_current_thread().enable_all().build()`，与 `bash.rs:103` 同款 —— MCP stdio 只需 io + process + time，`current_thread` 足够，避免多线程开销),
2. 一个 **`McpServerManager`**(直接 owned,**不是** `Arc`)。

同步方法实现:
```rust
fn call_tool_sync(&mut self, qualified: &str, args: Option<Value>) -> Result<…> {
    self.runtime.block_on(self.manager.call_tool(qualified, args))   // 同 bash.rs:104
}
```
`block_on` 借 `&mut self.manager` 满足 `&mut self` 约束，runtime 与 manager 同 owner、生命周期一致。

### 3.2 Arc<Mutex<_>> 还是单线程独占?

**默认单线程独占(不要 Arc<Mutex>)。** 理由:
- `CliToolExecutor::execute` 是 `&mut self`(`main.rs:4889`)且整个 run_turn 是单线程同步串行(`conversation.rs:220` 的 `loop`),工具一个个跑,**不存在并发访问 manager**。直接 `CliToolExecutor` 持有 `Option<McpSyncHandle>` 即可,零锁。
- **但有一个生命周期约束触发共享需求**:plan-mode 切档(`main.rs:2435` `/plan execute`、`main.rs:2472` `/plan exit`、`main.rs:2527` `/plan <task>`)会调 `build_runtime`(`main.rs:3955`)**重建整个 `ConversationRuntime`,内含全新 `CliToolExecutor`(`main.rs:3978`)**。若 manager owned 在 `CliToolExecutor` 里,每次切档都会 drop 旧 manager(杀掉已 spawn 的 MCP server 子进程)并重连 —— 重复 spawn/initialize,慢且可能触发 server 端速率限制。

   → **解法**:把 `McpSyncHandle` 提一层,由 **`Cli` 结构体(`main.rs` 的顶层 REPL 状态)持有,跨 `build_runtime` 复用**,通过 `Rc<RefCell<McpSyncHandle>>` 注入每个新建的 `CliToolExecutor`(单线程 REPL 用 `Rc<RefCell>` 足够,**不需要 `Arc<Mutex>`** —— 全程同一线程,无 `Send` 需求)。`build_runtime` 签名增参接收这个共享句柄(plan.md RW4「14 处调用点签名同步」「manager 必须复用」即指此)。

   - 若未来某条路径真的跨线程(目前**没有**:tool dispatch 全单线程),再升级到 `Arc<Mutex<McpSyncHandle>>`。本版按 `Rc<RefCell>` 即可,与现有 REPL 单线程模型一致。

### 3.3 实现纪律(规避唯一的真 panic 风险)

(a) 成立的前提是 **MCP `block_on` 必须发生在 `CliToolExecutor::execute` 这一纯同步层(`main.rs:4889`),而不是塞进任何已存在的 `block_on` 闭包**(尤其 `main.rs:4152` stream 闭包、`bash.rs:104` 闭包内)。plan.md T3 已把拦截层定在 `CliToolExecutor::execute`(在调 `execute_tool` 之前),天然落在 block_on 之外的同步区 —— **设计已对**。实现时只需保证 `McpSyncHandle::call_tool_sync` 的 `block_on` 不被任何外层 `block_on` 包住即可(当前栈不会)。

---

## 4. Phase 2 hooks 交叉验证:同一结论

plan.md TIMEOUT-1(per-hook timeout + 超时 `kill().await`)若用 tokio 实现,需同样的 sync→async 桥接。验证:
- hook 执行点在 `conversation.rs:271`(`run_pre_tool_use`)与 `conversation.rs:288`(`run_post_tool_use`)—— 与工具 dispatch(`conversation.rs:282`)**在同一个同步 `run_turn` 循环体内,彼此相邻**。
- 现状 hooks 用 **`std::process::Command`**(`crates/runtime/src/hooks.rs:2`),`hooks.rs:174 output_with_stdin(...)` 同步阻塞 —— 已是纯同步,无 tokio。
- 结论:hook 的调用上下文与 MCP manager **完全相同(同栈、同循环、block_on 已退出)**。Phase 2 若引入 tokio 做 timeout/kill,可复用 SPIKE-A 同款「`current_thread` runtime + `block_on`」桥接,**(a) 同样成立,无嵌套 panic**。
  - (备选:hook timeout 也可纯 `std` 实现 —— `std::process::Child` + `wait_timeout` crate 或自建计时线程 —— 完全不碰 tokio。但即便选 tokio 路线,本 spike 结论保证其安全。)

---

## 5. 一页式裁定

| 问题 | 答案 | 关键证据 |
|---|---|---|
| main 是 `#[tokio::main]`? | **否**,纯 `fn main`→`fn run` | main.rs:168/179 |
| 进程级/根 runtime 存在? | **否**,只有局部 block_on 孤岛 | 全仓 grep,无 Handle::current/.enter() |
| `run_turn` / `execute` 是 sync? | **是**(sync fn / sync trait) | conversation.rs:195/54 |
| 工具 dispatch 在 block_on 内? | **否**,stream 的 block_on 在 dispatch 前已返回 `Vec` | main.rs:4152→4332,conversation.rs:237→282 |
| `execute` 跑在 tokio 上下文? | **否** | Bash 工具 bash.rs:103 同栈 block_on 长期不 panic = 实证 |
| 结论 | **(a)** 独立 `current_thread` runtime + `block_on` 安全;**无需** block_in_place / 后台线程 | bash.rs:103-104 先例 |
| 句柄形态 | `McpSyncHandle{runtime, manager}` owned;`Cli` 顶层持有,`Rc<RefCell<_>>` 注入各 `CliToolExecutor`,跨 plan-mode build_runtime 复用 | 因 `&mut self`(mcp_stdio.rs:359/433)+ 子进程不可共享 + plan-mode 重建 main.rs:2435/2472/2527 |
| 锁需求 | **不要 Arc<Mutex>**;单线程串行 dispatch 用 `Rc<RefCell>` 足够 | conversation.rs:220 单线程 loop |
| Phase 2 hooks 同结论? | **是**,hook 与工具同栈同循环,现纯 std 同步 | conversation.rs:271/288,hooks.rs:2/174 |

**给 Phase 1 的硬约束**:T4/RW4 实现时,MCP `block_on` 只能放在 `CliToolExecutor::execute`(main.rs:4889)同步层、`execute_tool` 之前(plan.md T3 已定),绝不塞进 stream(main.rs:4152)或 bash(bash.rs:104)的既有 block_on 闭包 —— 否则会人为制造嵌套 panic。架构上 (a) 安全,风险全在这一条实现纪律。
