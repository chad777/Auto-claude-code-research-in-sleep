# v0.4.17 实施计划 — MCP 真接入 + hooks 保真 + 长尾(P8 建议独立成 v0.4.18)

> 基于 2026-06-05 6-agent 只读调查(原始材料 `design_raw.json`,所有 file:line 已对照 HEAD=81e5652 实查)。
> 沿用 v0.4.16 零回归方法论:characterization-test-first + 危险代码逐字不动 + 每 phase codex gate。

## 0. Scope 裁定(本计划最重要的一条)

**推荐:P8 full routing 从 v0.4.17 拆出去,独立成 v0.4.18。**

理由:
1. **风险性质不同**。MCP 接线对不配 `mcpServers` 的用户**不触发任何新 MCP 分支,普通工具目录等价由 characterization 保证**(措辞与 §7 对齐,codex R2-P2);P8.2 却要重写 `OpenAIRuntimeClient::stream`(437-1053,600 行单函数,SSE 解析+UI 渲染+IO 内联),v0.4.15 刚稳定的 #249/C6/C11/stream_options 全在这段里。**把最高风险的重构和最大的新功能捆一个版,违反 v0.4.15 确立的 scope 纪律**。
2. **主题完整性**。v0.4.17 = "让用户配置的功能真正生效"(MCP + hooks);v0.4.18 = "OpenAI 系 subagent 真路由"(provider 抽象)。各自可独立验收。
3. **P8 估时 4.25-5.5 人日**(P8.1 0.75-1 + P8.2 1.5-2 + P8.3 0.5 + P8.4 0.75-1 + P8.5 1),单独已是一个完整 release 的量。
4. v0.4.16 的 fail-loud guard 还在兜底,凭证泄漏窗口已关,P8 不抢时间。

(若 maintainer 仍要合并成一版,Phase 4 之后追加 P8 的 5 步即可,phase gate 机制不变。)

## 1. Issue sweep 新信号(并入 scope)

| Issue | 信号 | 处置 |
|---|---|---|
| **#286**(2026-06-05 今天新开)| Codex MCP >3000 char 内联 prompt 走 stdio 卡死,写文件传路径可绕过。现象在 Claude Code↔Codex,但 aris 自己的 `mcp_stdio.rs` 是同款**串行 write_all→flush→read 单任务模型**(write_frame:667 / read_response:723),大 payload 同样有 pipe buffer 死锁风险 | **新增 RW9**(见 Phase 1);回复作者确认绕过方案 |
| **#259** | 用户配 MiniMax executor + DeepSeek reviewer 却发 OpenAI 请求 | Phase 0 顺手 triage(可能是 routing 真 bug,也可能是配置误读);不阻塞本版 |
| **#258** | Copilot CLI 内置 subagent 审阅请求 | P8/provider 主题,挂 v0.4.18 backlog |
| #142 | Skill tool args 必填报错 | 低优先,Phase 3 顺手验证最小修(args 可选) |

## 2. Phase 0 — characterization + 两个前置 spike(~0.5-1 人日)

锁当前行为(全部先绿再动手,commit = 回滚锚点):

1. **工具目录真值**:`mvp_tool_specs()` 的 20 个工具名/描述/schema 序列化结果;`execute_tool` 对未知名返回 "unsupported tool";`filter_tool_specs` 过滤语义。
2. **doctor 输出**:~~MCP section 当前文案~~ **显式移出 Phase 0(codex R3-P1.2 + W1 论证)**:文案是 `run_doctor()` 内联 println(main.rs:5286/5289),实读本机状态,不可单测。改在 **Phase 1 RW7/T7 内按"先抽纯 formatter `mcp_doctor_section(count)->Option<String>` → 立刻锁基线测试(含 'lands in v0.4.16' 现文案)→ 再改文案"的次序执行**,characterize-then-modify 不破锚点纪律。
3. **hooks 解析**:string-style 与 object-style 各 shape 的 flatten 结果(`optional_hook_commands` config.rs:805-864 真值表);PreToolUse/PostToolUse 对所有工具无差别触发;未知 event 静默忽略;`aris init` 写的 meta_opt hook(5 event,object-style 带 async:true)解析结果。
4. **mcp_stdio 现有 11 测试**确认全绿(已是 characterization)。
5. **SSE/stream**:本版不动 openai_executor 主路径,但 OE6/CL2 触及完成判定边缘——为 `stream_eof_action` / tool_call 累积补 fixture 锁现值。

两个 spike(只读验证,决定 Phase 1 设计):
- **SPIKE-A(RW4 最大雷)**:实查 `CliToolExecutor::execute` 调用栈是否在 tokio runtime 上下文内。证据倾向**不在**(conversation run_turn 是同步 fn,ExecutorClient 内部 `self.runtime.block_on` 在 stream 调用处已退出)→ 若证实,manager 用「sync 句柄封装独立 current_thread runtime + block_on」(同 bash.rs:103 模式)安全。若证伪,改 `tokio::task::block_in_place`。
- **SPIKE-B(#259 triage)**:复现配置 MiniMax executor + DeepSeek reviewer,看请求实际打到哪。

## 3. Phase 1 — MCP 真接入(M1/M2 + M6 + #286 防御,~2-2.5 人日)

### Runtime 侧(crates/runtime/src/mcp_stdio.rs)

| ID | 内容 | 关键点 |
|---|---|---|
| **RW1** | initialize 后补发 `notifications/initialized` | MCP spec 强制;无 id 通知,不走 request() 读循环;严格 server 不收到会拒 tools/list |
| **RW2 (M6)** | read_frame 容错:空行判定兼容 LF-only;`Content-Length` 大小写不敏感(`split_once(':')` + `eq_ignore_ascii_case`);加 `MAX_CONTENT_LENGTH`(64MiB)防 OOM | 不破现有 11 测试 |
| **RW9 (#286)** | request() 大 payload 防死锁。**codex R1-P1.4:不能只写 "select/join"——`&mut self` 下 spawn 两个任务借用不成立**。真实设计:拆 stdin/stdout ownership(`ChildStdin` / `BufReader<ChildStdout>` 本就是独立 owned 半边,struct 改持 `Option<_>`,request 时 take 两半 → write 任务持 stdin 半边、read pump 持 stdout 半边,`tokio::join!` 至 response id 匹配 → put back;失败路径仍 kill+respawn) | 回归测试:fake server 先写满 stdout 再读 stdin 的场景;timeout 仍包整个并发体(死锁→超时 kill 的兜底不丢) |
| **RW6** | discover_tools 改 per-server try:失败 server 记 warning 跳过(对齐 unsupported_servers 模式),不再一损俱损;stderr 改 piped + 后台 drain(防 v0.4.13 同款管道满阻塞) | 现有 discover 测试预期要同步改(语义变化点,changelog 披露) |

### Tools/CLI 侧

| ID | 内容 | 关键点 |
|---|---|---|
| **T1** | `ToolSpec.name: &'static str` → 支持动态名 | **codex 裁定(R1): 选 (A) 新 `RuntimeToolSpec{name:String,...}` 仅用于请求构造层**——`tools/lib.rs:55` 静态 MVP spec 一字不动,零回归;否决 (B) Cow(会触碰全部 mvp_tool_specs 消费者) |
| **T2** | `mcp_tool_specs(manager)`:ManagedMcpTool → RuntimeToolSpec(qualified_name 做 name,schema None 给空 object,最小净化保证 object schema)。**+碰撞策略(codex R1-P1.5)**:`normalize_name_for_mcp` 把非法字符归一成 `_`,`"a b"` 与 `"a_b"` 会撞名且后者静默覆盖 tool_index(mcp_stdio.rs:407-408)——discover 时检测重名,warn + 确定性后缀(`_2`),广告侧去重;加 collision test | 命名/清洗用现成 `runtime/src/mcp.rs:25-37 mcp_tool_name` |
| **T3** | MCP dispatch 分支:`tool_name.starts_with("mcp__")` → manager.call_tool → McpToolCallResult.content 展平为文本 + is_error 映射。**拦截层 = `CliToolExecutor::execute`(main.rs:4888),在调 `tools::execute_tool` 之前(codex R2-P1.3)**——不进 `tools::execute_tool` 静态 match(lib.rs:410),因为 subagent 也直调它(lib.rs:2244),放 executor 层结构性保证 T6"subagent 不给 MCP" | MCP 名落到 tools::execute_tool 仍返回 unsupported = T6 的免费测试面 |
| **T4/RW4** | CliToolExecutor 持有 manager(SPIKE-A 决定桥接方式);构造点 main.rs:3978 注入;build_runtime 14 处调用点签名同步 | **plan-mode 切档(main.rs:2435/2472/2527)重建 runtime 时 manager 必须复用**(Arc/外层持有),避免重连 server |
| **RW5** | 启动 eager discover 一次,**工具目录 = mvp + mcp 拼接必须同时进两条 provider 路径(codex R1-P1.3)**:Anthropic 侧 main.rs:4138 `filter_tool_specs()` + OpenAI 侧 openai_executor.rs:457 独立调的同一静态过滤器——只改一边会让 OpenAI-family 主会话看不到 MCP 工具 | server 多工具撞 provider tool 上限:>N(如 40)时 warn,本版不做 deferred 化(留 v0.5.0) |
| **T5** | MCP 工具权限(**codex R1-P1.1 重设计 + R2-P1.1/P1.2 细化**):现有模型下"注册成某 PermissionMode"不可行——permissions.rs:81 未注册工具默认 `DangerFullAccess`,CLI 默认 active mode 也是 `DangerFullAccess`(main.rs:562),注册 = 静默放行。**改为 MCP 专用 approval path**,与 `PermissionPolicy` 的相对位置(R2-P1.1):工具调用先走 generic 权限授权(conversation.rs:262-282)再进 executor——MCP 工具在 generic gate 按**最小 required mode 注册通过**,真正安全确认由 `CliToolExecutor::execute` 里的 MCP 专用 approval 执行(避免 read-only 下先被 deny 够不到 approval、Prompt mode 下双重确认);**测试 read-only/workspace/danger/prompt 四档行为**。信任出口:`mcpServers.<name>.trust: true` + 会话级"本 server 不再问"。**trust 字段要真落 config schema(R2-P1.2)**:`McpStdioServerConfig`(config.rs:103)现无此字段,parse(config.rs:624)只读 command/args/env/requestTimeoutSecs,未知字段静默忽略——必须加字段 + getter + round-trip 解析测试,否则 `trust: true` 看似可配实际无效 | 新增小型 McpApproval 检查点,不动 PermissionPolicy 本体 |
| **T8** | `--allowedTools` 放行动态名(**codex R1-P1.2**):main.rs:500 只从 `mvp_tool_specs()` 建合法名集,main.rs:526 unknown 直接报错——用户无法 `--allowedTools mcp__codex__codex`。改为 `mcp__` 前缀延迟校验(arg 解析期放行,dispatch 期实际过滤) | 加 char test 锁现有非-mcp 名的报错行为不变 |
| **T6** | Subagent 明确不给 MCP 工具(注释 + 测试钉死),v0.4.18 随 P8 一起考虑 | 最小验证面 |
| **T9** | fail-loud guard 文案修正(**codex R1-P1.6**):tools/lib.rs:1901 错误文案说 "lands in v0.4.17",P8 拆版后变假——改成 v0.4.18,**同步翻转断言 `msg.contains("v0.4.17")` 的 characterization test**(故意翻转,changelog 披露) | trivial 但不能漏 |
| **RW7/T7** | doctor MCP section 升级,**三步次序(codex R3-P1.2)**:① 抽纯 formatter `mcp_doctor_section(count)->Option<String>`(行为保持);② 锁基线测试(含 'lands in v0.4.16' 现文案);③ 改文案为 per-server spawn/initialize/tools 真实状态 | doctor 'Codex MCP' 读 ~/.claude.json 与 ConfigLoader 路径不一致的问题顺手统一披露(不强修) |

**Phase 1 出口**:配 codex MCP 的 settings.json,在 aris 里真实调用 `mcp__codex__codex` 跑通一次对抗审(零 API key 路径验收)+ 全部 char test 绿 + codex review GO。

## 4. Phase 2 — hooks 保真 + timeout(~1 人日)

| ID | 内容 | 裁定(codex 复核) |
|---|---|---|
| **SCHEMA-1/2** | `optional_hook_commands` 不再 flatten 成 `Vec<String>`:新 `RuntimeHookConfig{matcher:Option<String>, command:String, timeout:Option<u64>, async_flag:Option<bool>}`;string-style 自动升格(全 None);非-command type 与未知 event 继续静默接受。**+prompt summary 同步(codex R1-P2.2)**:`prompt.rs:759` 有一套独立的 hook 解析做系统提示摘要——schema 改后必须同步,否则模型看到的 hook 数和实际执行不一致 | **向后兼容矩阵 = characterization Phase 0 #3**,现有用户两种 shape 解析结果不变(只是不丢字段) |
| **SCHEMA-3** | 执行层 matcher 过滤:**用 regex(codex R1-P2.1:`runtime/Cargo.toml:11` 已有 `regex = "1"` 依赖,"不引依赖"的理由不成立)**;matcher 缺省/空 = 匹配所有(现行为);regex 编译失败 → warn + fallback 到字面精确匹配(可预测、不静默禁用用户 hook) | meta_opt hook(aris init 写的)无 matcher → 行为不变 |
| **TIMEOUT-1/2** | per-hook timeout(default 30s,字段覆盖,clamp 合理区间);超时 **Warn 不 Deny**(codex R1 裁定 D4 确认:hooks.rs:179 非 0/2 退出已是 Warn 继续,timeout 是 hook 基础设施失败,用户要阻断应让 hook 在 timeout 内 exit 2);超时后 kill 子进程(对照 v0.4.10 M3 `kill().await` 模式);同步→async 桥接方式跟随 SPIKE-A 结论 | `async:true` 本版只解析存储,执行仍同步,CHANGELOG 标 known-unsupported;event 扩展(SessionStart 等)不做 |

## 5. Phase 3 — 长尾四件(~0.5 人日)

| ID | 内容 | 备注 |
|---|---|---|
| **A5.1** | api Keychain gate:`ARIS_DISABLE_KEYCHAIN=1` 环境变量,gate 住 client.rs:394/485 两个 fallback 调用点;CI/测试设置之 → 本地 7 红测试转绿 | 选 env var 不选 #[cfg(test)](fallback 有两个生产调用点,测试走真实路径) |
| **A5.2** | CL2:Anthropic `MessageDelta.stop_reason`(types.rs:260)纳入完成判定,对称 v0.4.15 OpenAI finish_reason | 健壮性对称,无已知 blocked 用户;改动最小化 |
| **A5.3** | OE6:tool_call index 缺失时按 id fallback(保守版,不做 IndexMap 重构);OE5:usage fallback 小修;OE8:仅解析器容错(近 no-op,优先级最低,时间紧可砍) | 全部 fixture 先行 |
| **A5.4** | slash 入历史 — **需 maintainer 拍板**:v0.4.16 characterization 把"slash 不持久化"锁成了契约(input.rs:1262),改 = 故意翻转该测试 | 默认做(trivial),但等用户点头 |

## 6. Phase 4 — release(~0.5 人日)

- CHANGELOG + 双语 README banner + Cargo bump 0.4.17
- codex final integration review(diff base = 81e5652)
- 测试矩阵 CI-mode 全绿(runtime/aris-cli/tools/commands;api 在 A5.1 后应全绿)
- 回复 #286(确认写文件绕过 + aris 侧防御已做)/ #274 无需动作
- 用户手测 gate(重点:配真实 codex MCP 跑通对抗审;不配 mcpServers 的默认路径无感)→ push + tag + main README rollup + memory

## 7. 零回归机制(继承 v0.4.16)

1. characterization-test-first,Phase 0 commit = 回滚锚点;
2. **不配 mcpServers 的用户:无任何新 MCP 分支被触发;不配 hooks 的用户:执行等价**(codex R1-P2.3 修正措辞:hooks 空配置仍会经过新 parser、工具目录构造层有 RuntimeToolSpec 转换——"等价"由 characterization 测试证明,而非"走不到");
3. 危险代码不碰:openai_executor 主路径(除 OE5/6/8 边缘 + RW5 的工具目录注入点 openai_executor.rs:457 + fixture 先行)、pricing 链、reviewer 路由、config env-writing、push_history 契约;
4. 每 phase codex review gate;
5. 最大已知雷 = RW4 嵌套 runtime panic(SPIKE-A 前置排雷)+ RW6 stderr piped 不 drain 的管道满(实现时直接配 drain 任务)+ RW9 借用拆分(设计已定 take/put-back)。

## 8. 估时汇总

| Phase | 内容 | 估时 |
|---|---|---|
| 0 | characterization + 2 spike | 0.5-1 人日 |
| 1 | MCP 真接入(RW1/2/9/6 + T1-T9 + RW5/7,含权限/allowedTools/双广告/碰撞) | 3-3.5 人日 |
| 2 | hooks(SCHEMA-1/2/3 + TIMEOUT-1/2 + prompt summary 同步) | 1-1.5 人日 |
| 3 | 长尾(A5.1-A5.4) | ~0.5 人日 |
| 4 | release | ~0.5 人日 |
| **合计** | | **~6-7 人日**(codex R1-P2.4 校正:原 4.5-5.5 偏乐观,漏算权限/allowedTools/双广告/manager 生命周期/RW9 IO/碰撞/schema migration/summary 对齐) |

## 9. 裁定记录

| # | 决策 | 结论 | 出处 |
|---|---|---|---|
| D1 | P8 拆 v0.4.18? | **拆**(codex R1 同意 §0 论证;唯一隐藏耦合 = subagent-MCP,T6 显式切开) | **maintainer 确认(2026-06-05)** |
| D2 | T1 方案 | **A:RuntimeToolSpec**,静态 ToolSpec 一字不动 | codex R1 裁定 |
| D3 | slash 入历史(翻转 v0.4.16 契约)? | **做**(CHANGELOG 披露契约翻转) | **maintainer 确认(2026-06-05)** |
| D4 | hook timeout 语义 | **Warn**(对齐 hooks.rs:179 现有 Warn-继续模式) | codex R1 裁定 |
| D5 | MCP 工具默认权限 | **MCP 专用 approval path,必须 prompt + 显式 trust 出口**;不能注册成 DangerFullAccess(= 静默放行) | codex R1 裁定 |

## 10. Codex review 记录

- **R1 (2026-06-05, gpt-5.5 xhigh, read-only)**: **NO-GO** — 0 P0 / 6 P1(权限模型不成立、--allowedTools 拒动态名、双 provider 广告漏 OpenAI、RW9 借用设计不成立、qualified name 碰撞、guard 文案 v0.4.17 变假)/ 4 P2(matcher 应直接用已有 regex 依赖、prompt.rs:759 摘要不同步、§7 措辞过满、估时乐观)。**全部 6 P1 + 4 P2 已并入本版 plan(T5 重设计/T8/RW5 双路径/RW9 ownership 拆分/T2 碰撞策略/T9 文案,SCHEMA-3 regex/SCHEMA-1 摘要同步/§7 措辞/§8 估时 6-7 人日)**。D2/D4/D5 按其裁定落定。
- **R2 (同日, 同线程)**: **GO-WITH-NITS,准入 Phase 0** — 3 P1-before-Phase-1(① T5 必须明确 MCP approval 与 PermissionPolicy 的相对位置:generic gate 最小 required mode 通过 + executor 层真确认 + 四档 mode 测试;② `trust` 字段必须真落 config schema + round-trip test,当前未知字段静默忽略;③ T3 拦截层必须在 CliToolExecutor::execute 不在 tools::execute_tool,结构性保证 subagent 不带 MCP)+ 1 P2(§0 措辞与 §7 对齐)。**4 条全部已并入**(见 T3/T5/§0)。
