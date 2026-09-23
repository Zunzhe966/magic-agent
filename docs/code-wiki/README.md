# 尊者魔法代理（Magic Agent）· Code Wiki

> 本目录是项目的**代码级结构化文档**（Code Wiki），面向需要阅读/修改/接手本项目源码的开发者。
> 与产品文档的分工：`README.md`（怎么用）、`docs/设计.md`（为什么这么设计）、`CONTRACT.md`（架构红线）
> —— 本 Wiki 只回答一个问题：**代码是怎么组织的、每个模块在干什么、怎么跑起来。**

**文档基线**：仓库当前工作区版本 `0.2.10`（`src-tauri/tauri.conf.json` / `Cargo.toml` / `ui/package.json` 三处一致）。

> ⚠️ 本目录索引中列出的 **06-run-and-build.md、07-dependencies.md、08-key-flows.md 尚未创建**（规划中），文中指向它们的链接暂不可用。

---

## 目录

| 文档 | 内容 | 适合谁 |
|---|---|---|
| [01-architecture.md](./01-architecture.md) | 整体架构、进程与数据流、三个入口的关系、关键设计约束 | 所有人，先读这篇 |
| [02-backend-rust.md](./02-backend-rust.md) | Rust 后端逐模块说明（lib / mihomo / config / apps / ssh / system_proxy / keychain / updater） | 后端开发 |
| [03-frontend-ui.md](./03-frontend-ui.md) | Vue3 前端组件树、状态管理、Tauri command 调用矩阵 | 前端开发 |
| [04-mcp-server.md](./04-mcp-server.md) | Python MCP Server：工具清单、协议层、与 Rust 端的一致性约束 | 集成 / AI 侧开发 |
| [05-scripts-and-tooling.md](./05-scripts-and-tooling.md) | 脚本工具链：发版、图标、特权控制器、MCP 守护、一致性校验 | 构建 / 运维 |
| [06-run-and-build.md](./06-run-and-build.md) | 环境要求、开发运行、测试、构建打包、发版流程、故障排查 | 所有人，动手前必读（**待创建**） |
| [07-dependencies.md](./07-dependencies.md) | 依赖关系清单、外部命令依赖、许可证边界 | 所有人（**待创建**） |
| [08-key-flows.md](./08-key-flows.md) | 关键业务流程时序（启停代理、分流决策、SSH、更新） | 需要改业务逻辑的人（**待创建**） |

---

## 30 秒读懂这个项目

**一句话**：一个 macOS 桌面应用（Tauri 2 + Vue3），把 **mihomo 代理内核**包成"按软件精确分流"的工具，同时内置 **SSH 云服务器控制台**，并暴露 **MCP 工具**给 AI 客户端操控。

**三个入口，一套内核**：

```
┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐
│  Tauri 桌面 App  │   │  MCP Server(Py)  │   │  CLI bin 工具    │
│  (Vue3 前端 UI)  │   │  (给 AI 客户端)  │   │  magic_probe 等  │
└────────┬─────────┘   └────────┬─────────┘   └────────┬─────────┘
         │ Tauri IPC            │ 共享 config.json     │ 直接调用 lib
         ▼                      ▼                      ▼
      Rust 后端 (lib.rs / mihomo.rs / config.rs ...)   magic_agent_lib
         │
         │ 生成 mihomo.yaml + osascript/sudo 提权启动
         ▼
   mihomo 内核进程（root，端口 7891/7892/7893，控制 API 19091）
```

**核心约束（改代码前必须知道，详见 `CONTRACT.md`）**：

1. **App 生命周期必须完整覆盖内核生命周期** —— 关 App = 关代理，绝不留下 root 孤儿内核。
2. **凡可能阻塞的 IO，一律 `async fn` + `spawn_blocking`** —— 否则主线程被占，macOS 转彩圈。
3. **MCP 与 Rust 是同一软件的两个入口，同一功能行为（含副作用）必须一致**。
4. **状态查询必须报真实状态，不能报配置意图**（如系统代理读 `scutil --proxy`，不读 config）。
5. **`tun.auto-route` 永远是 `false`** —— 改回 true 会接管系统默认路由，重演劫持事故。
6. **listeners 必须逐条显式 `listen: 127.0.0.1`**（2026-09-23）—— `allow-lan:false` 管不到 listeners、`bind-address` 无效，缺省即绑 0.0.0.0 暴露局域网。
7. **root 启动路径必须 umask 077 + 日志 chmod 600**（2026-09-23）—— 日志含全机连接记录，世界可读即隐私泄露。

---

## 代码规模速览

| 部分 | 文件 | 行数（约） |
|---|---|---|
| Rust 后端 | `src-tauri/src/*.rs` | ~5 600 |
| Rust bin 工具 | `src-tauri/src/bin/*.rs` | ~37 |
| Python MCP | `mcp/server.py` | ~2 180 |
| Vue3 前端 | `ui/src/**` | ~1 680 |
| 脚本 | `scripts/*` | ~800 |
| **合计** | | **~10 300** |
