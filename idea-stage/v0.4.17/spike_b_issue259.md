# SPIKE-B — Triage issue #259（MiniMax executor + DeepSeek reviewer 却发 OpenAI 请求）

> 只读调查，对照 HEAD=81e5652（v0.4.16）。结论：**配置误读（CONFIG MISUSE），不是 routing bug**。
> 维护者 wanshuiyin 已在 2026-05-29 给出正确诊断，用户 2026-05-29 确认升级 + `aris setup` 后「能使用了」。
> 处置建议：**不归入 v0.4.17 任何 phase，作为文档/可用性问题关单**（顺手可做一个 0.5 人日的「友好降级」防呆增强，列在最后，非必须）。

---

## 1. 用户的配置流（issue 原文）

用户手写了 `~/.aris/config.yaml`，内容是**嵌套 YAML**：

```yaml
executor:
   provider: minimax
   api_key: "sk-xxx"
   model: "MiniMax-M2.7"
reviewer:
  provider: deepseek
  api_key: "sk-xxx"
  model: "deepseek-chat"
```

然后跑 `aris auto-review-loop "..."`，观察到「发 OpenAI 请求」。

---

## 2. 证据链：ARIS 根本没读这个文件

### 2.1 配置只读 `~/.config/aris/config.json`，且是 **JSON**、**扁平字段**

- `crates/aris-cli/src/config.rs:12-13` — `CONFIG_DIR = ".config/aris"`、`CONFIG_FILE = "config.json"`。
- `crates/aris-cli/src/config.rs:58-61` — `config_path()` = `home/.config/aris/config.json`。**不是** `~/.aris/`。
- `crates/aris-cli/src/config.rs:68-71` — `load()` 用 `serde_json::from_str` 解析；**没有任何 YAML 解析器**。
- `crates/aris-cli/src/config.rs:29-55` — `ArisConfig` 是**扁平结构**：`executor_provider` / `executor_api_key` / ...，**不是** `executor.provider` 这种嵌套。
- `crates/aris-cli/src/config.rs:63-67` — 路径不存在时 `load()` 直接 `return Self::default()`（**全字段 `None`**）。

### 2.2 全仓库没有任何代码读 YAML 或 `~/.aris/`

```
grep -rni --include='*.rs' 'config\.yaml|serde_yaml|from_yaml|\.aris/config|"\.aris"' crates/
→ 0 命中（非测试）
```
唯一的 config 读取入口就是 `ArisConfig::load()`（`config.rs:63`）。

**结论**：用户的 `~/.aris/config.yaml`（错路径 + 错格式 + 错结构，三重不匹配）**从未被读到**，`load()` 返回 default，所有 `executor_*` / `reviewer_*` 字段为 `None`，`apply_to_env()` 不设置任何 env var。

### 2.3 为什么会看到 "OpenAI 请求"

config 没生效 ⟹ 进程实际用的是**默认值 + shell 环境变量**（`apply_to_env` 用 `ApplyMode::IfMissing`，shell env 永远赢，`config.rs:87-88`）。两条独立路径都会落到 `api.openai.com`：

**(a) Reviewer 侧（auto-review-loop 主要走这条，最可能就是用户看到的）**
- `ARIS_REVIEWER_PROVIDER` / `ARIS_REVIEWER_MODEL` 都没被设置（config 没读到）。
- `run_llm_review`（`crates/tools/src/lib.rs:3442-3444`）：`ARIS_REVIEWER_MODEL` 缺省 ⟹ `configured_model = "gpt-5.5"`。
- reviewer_provider 为 None ⟹ 跳过 custom（`lib.rs:3453`）、跳过 deepseek/anthropic-compat（`lib.rs:3490-3492`）。
- 落到 OpenAI-compat 路径（`lib.rs:3515-3516`）：`route_openai_compat_model("gpt-5.5")` ⟹ `("OPENAI_API_KEY", "https://api.openai.com/v1/chat/completions", "openai")`（`lib.rs:3389-3392`）。
- **即：reviewer 默认就是 gpt-5.5 @ api.openai.com**，与用户写的 DeepSeek 配置完全无关（因为配置没读到）。

**(b) Executor 侧（只有当 shell 里残留 `EXECUTOR_PROVIDER=openai` 才会触发）**
- `crates/aris-cli/src/main.rs:3956` — 只有 `resolve_openai_executor_config().is_some()` 才走 OpenAI executor。
- `crates/aris-cli/src/openai_executor.rs:357-361` — 该函数**要求 `EXECUTOR_PROVIDER=="openai"`，否则返回 None**。
- 若返回 None ⟹ 回退 Anthropic executor（需 `ANTHROPIC_API_KEY`，用户也没配 ⟹ 会报缺 key，而不是发 OpenAI）。
- 若用户 shell 里恰好 export 了 `EXECUTOR_PROVIDER=openai` + `OPENAI_API_KEY`（很多人装过别的 OpenAI 工具），则 `openai_executor.rs:363-374`：api_key 回退 `OPENAI_API_KEY`、base_url 缺省 `DEFAULT_OPENAI_BASE_URL = https://api.openai.com/v1`（`openai_executor.rs:18, 374`）⟹ executor 也打到 OpenAI。

无论 (a) 还是 (b)，**根因都是 config.yaml 没被读到**，而非任何 routing 逻辑写错。

---

## 3. 反证：如果用户用 `aris setup` 正确配置，绝不会打到 OpenAI

### 3.1 MiniMax executor（菜单项 5）

- `crates/aris-cli/src/config.rs:364-370` — 序列化为 `executor_provider="openai"` + `executor_base_url="https://api.minimax.chat/v1"` + model `MiniMax-M2.7`。
- 运行时 `config.rs:194-198` 设 `EXECUTOR_BASE_URL=https://api.minimax.chat/v1`；`openai_executor.rs:370-374` 读到非空 base_url ⟹ 请求打到 **MiniMax**，不会回退默认 OpenAI。

### 3.2 DeepSeek reviewer（菜单项 7）

- `crates/aris-cli/src/config.rs:237-240` — 序列化为 `reviewer_provider="deepseek"`，key 存进 `ARIS_REVIEWER_AUTH_TOKEN`。
- 运行时 `lib.rs:3490-3510` — `ARIS_REVIEWER_PROVIDER=="deepseek"` ⟹ 走 anthropic-compat 分支，default_base = `https://api.deepseek.com/anthropic`（`lib.rs:3503-3504`），endpoint = `.../anthropic/v1/messages`（`lib.rs:3509`）⟹ 请求打到 **DeepSeek**，**完全不碰 OpenAI**。

所以正确配置下，executor→MiniMax、reviewer→DeepSeek，两边都对。**routing 没有 bug。**

---

## 4. 判定

**(b) 配置误读（CONFIG MISUSE）。** 三重不匹配：
| 维度 | 用户写的 | ARIS 实际读的 |
|---|---|---|
| 路径 | `~/.aris/config.yaml` | `~/.config/aris/config.json`（`config.rs:12-13,58-61`） |
| 格式 | YAML | JSON（`config.rs:68-70`，无 YAML 解析器） |
| 结构 | 嵌套 `executor.provider` | 扁平 `executor_provider`（`config.rs:29-55`） |

文件从未被读 ⟹ 退回默认/shell-env ⟹ reviewer 默认 gpt-5.5@OpenAI（和/或 shell 残留的 `EXECUTOR_PROVIDER=openai`）⟹ 用户看到 OpenAI 请求。

**不是 v0.4.17 的 work item。** 维护者诊断正确，用户已确认 `aris setup` 后可用（见 §6）。建议直接关单或转为「文档可用性」标签。

---

## 5. 回复草稿（中文，给用户 — 实际上维护者已回过且用户已确认，此处仅作存档/可复用模板）

> 这个现象的根因是**配置文件没被读到**，不是 ARIS 路由错了 🙏
>
> ARIS 只读 **`~/.config/aris/config.json`**（注意：是 `.config/aris/` 不是 `.aris/`，是 **JSON 不是 YAML**，字段是**扁平的** `executor_provider` 而不是嵌套的 `executor.provider`）。你手写的 `~/.aris/config.yaml` 路径、格式、结构三处都不匹配，所以被整体忽略，ARIS 退回了默认配置 + 你 shell 里已有的环境变量 —— 默认 reviewer 是 `gpt-5.5`（打 `api.openai.com`），如果你 shell 里之前还 export 过 `EXECUTOR_PROVIDER=openai` / `OPENAI_API_KEY`，executor 也会一起打到 OpenAI。这就是「明明配了 MiniMax/DeepSeek 却在发 OpenAI 请求」的来源。
>
> **正确做法**：直接跑 `aris setup`，按提示 executor 选 `5. MiniMax`、reviewer 选 `7. DeepSeek`，它会把正确格式写进 `~/.config/aris/config.json`。配好后可以 `aris doctor` 自检一下，Executor 那行应显示 `OpenAI-compat (https://api.minimax.chat/v1)`，Reviewer API 那行能看到你的 key 被识别。
>
> 另外如果升级后 MiniMax 跑出现 `stream ended prematurely without [DONE] sentinel` 类流式报错，那是 [#249]，已在 v0.4.15 修复，升级到最新版即可。

---

## 6. 同作者后续 / 相关 issue

- **#259 本帖**：维护者 2026-05-29 回复正确（JSON 路径 + 扁平字段 + `aris setup`，并附 #249 流式修复）。用户 2026-05-29 回复「下载了最新版本，能使用了」——**问题已解决**，只是 issue 仍 OPEN。
- 用户随后在同帖**转题**：问 `/run-experiment` 能否「自动处理报错直到解决」，并报告在 WSL2 装 `mamba-ssm` 时程序被 terminate 崩溃。**这是另一个独立问题（环境/长任务 OOM 被 WSL kill），与 #259 标题无关**，不应混进 #259，建议引导另开 issue。
- 同作者其他 issue：**#274**（历史命令功能，已 CLOSED，已纳入 v0.4.16 roadmap Track A）、**#272**（survey 失败/续跑，已 CLOSED）。均与本 triage 无关。

---

## 7. 可选防呆增强（非必须，若 maintainer 想降低复发率，独立小项 ~0.5 人日，不属 v0.4.17 核心）

根因是「错配置被静默忽略」。可加**最小防呆**（不改 routing，零回归风险）：

1. **启动期 misconfig 提示**：若检测到 `~/.aris/config.yaml`（或 `~/.config/aris/config.yaml`）存在但 `config.json` 不存在 ⟹ 打一行 warning「检测到 `.aris/config.yaml`，ARIS 只读 `~/.config/aris/config.json`（JSON/扁平字段），该文件已被忽略，请运行 `aris setup`」。纯 stderr 提示，不读不解析 YAML。
2. **doctor 增强**：`run_doctor`（`main.rs:5132+`）已打印 Executor / Reviewer 状态——可额外提示「若你以为配了 X 但这里显示 Anthropic/OpenAI 默认，说明 config 没生效」。

这两条都**只加诊断输出、不碰 `apply_to_env` / routing**，可由 characterization 兜底，但**与 v0.4.17 主题（MCP 接入 + hooks 保真）无关，建议挂 backlog 或随手 PR，不进 phase gate**。
