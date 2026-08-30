# OpenRouter 免费模型实测报告

- **日期**：2026-08-22 11:50（北京时间）
- **目的**：验证谷歌模型 429 的真实原因；摸清当前哪些免费模型可用
- **方法**：用 WorkBuddy 实际使用的 OpenRouter 密钥，从本机直发请求（流量经魔法代理 TUN），对免费清单中 19 个文本类模型逐个发送短 prompt（max_tokens=32）

## 一、分流链路验证 ✅

mihomo 日志实锤，每一次 openrouter.ai 请求都命中显式规则：

```
[TCP] sandbox-cli --> openrouter.ai:443 match DomainSuffix(openrouter.ai) using NODE-示例节点[示例节点]
```

**结论：请求全部成功出国、到达 OpenRouter 并拿到正式业务响应。分流链路无任何问题。**

## 二、密钥状态

- key 有效，用量 $0，无限额（免费户）

## 三、实测结果：14 可用 / 19 总数

### ❌ 429 上游限速（limit_source=upstream_provider_shared_pool）
| 模型 | 说明 |
|---|---|
| google/gemma-4-26b-a4b-it:free | 复现用户遇到的错误 |
| google/gemma-4-31b-it:free | 同上 |
| z-ai/glm-5.2:free | 智谱免费池同样限速 |

429 含义：请求已到 OpenRouter，但 OpenRouter 上游的 **Google AI Studio 共享免费池**被全网用户挤爆。不是我们的网络问题、不是分流问题、也不是 key 被封。

### ✅ 当前真实可用的免费模型（14 个）
| 模型 | 备注 |
|---|---|
| nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free | 输出干净直接回 OK，推荐日常用 |
| nvidia/nemotron-nano-12b-v2-vl:free | 干净利落 |
| poolside/laguna-s-2.1:free | 干净利落 |
| dots-studio/dots-3-note-preview:free | 干净利落 |
| nvidia/nemotron-3-super-120b-a12b:free | 带思考过程输出 |
| nvidia/nemotron-3-ultra-550b-a55b:free | 大杯，带思考 |
| nvidia/nemotron-3-nano-30b-a3b:free | 带思考 |
| nvidia/nemotron-3.5-lightning:free | 带思考 |
| stealth/ox-alpha | 可用 |
| cohere/north-mini-code:free | content 为空（reasoning 字段返回），需适配 |
| liquid/lfm-2.5-2.6b:free | 同上 |
| nvidia/nemotron-3.5-content-safety:free | 内容安全分类器，非对话用途 |
| nvidia/nemotron-nano-9b-v2:free | content 空 |
| poolside/laguna-xs-2.1:free | content 空 |

### ⛔ 403 仅限智能体框架调用
thinkingmachines/inkling(-small):free —— 只允许 agentic harness 调用，裸 API 不行。

## 四、给 WorkBuddy 的建议（remedy）

1. **短期**：把默认免费模型从 google/gemma 切到 `nvidia/nemotron-nano-12b-v2-vl:free` 或 `poolside/laguna-s-2.1:free`（实测秒回、输出干净）
2. **谷歌模型**：想稳定用就 BYOK——在 OpenRouter 绑自己的 Google AI Studio key（is_byok=true 后走自己的配额）
3. **重试策略**：谷歌 :free 模型的限速是共享池性质，隔几小时重试有机会成功

## 五、本次代码变更

- `ui/src/components/ServersView.vue`：域名分流编辑区新增 reason 备注输入框 + 目标下拉可直接选具体节点名
- `src-tauri/src/mihomo.rs`：测试代码 DomainRule 初始化补 reason 字段（cargo test 4/4 通过）
- config.json：三条存量规则已回填 reason（github.com / huggingface.co / openrouter.ai→示例节点）
