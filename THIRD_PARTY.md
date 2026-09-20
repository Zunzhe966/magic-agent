# 第三方组件与许可声明（Third-Party Notices）

本软件（尊者魔法代理 / Magic Agent）**不是从零自研所有部件**。它站在大量优秀开源项目之上，
在此明确列出所依赖的第三方组件、各自用途与许可证，以示尊重与合规。

> 说明：本文件由项目维护者人工维护，用于满足各依赖许可证的「保留版权声明」义务。
> 若发现遗漏，欢迎提 Issue 补充。

---

## 一、代理内核

| 组件 | 用途 | 许可证 | 项目地址 |
|---|---|---|---|
| **mihomo**（原 Clash.Meta） | 代理转发内核。本软件**不修改其源码**，仅以外部独立进程方式调用，通过其 RESTful API 控制 | **GPL-3.0** | https://github.com/MetaCubeX/mihomo |

> ⚠️ **重要合规说明**：mihomo 采用 GPL-3.0。本软件通过**独立进程 + 进程间 API 调用**方式使用它，
> 不进行源码链接（static/dynamic linking），因此本软件自身代码不因此被 GPL「传染」。
> 但**分发时所附带的 mihomo 二进制文件本身仍受 GPL-3.0 约束**，必须随附其许可证全文与源码获取方式。

---

## 二、GeoIP / GeoSite 数据

| 组件 | 用途 | 许可证 | 项目地址 |
|---|---|---|---|
| **Loyalsoldier/geoip** 等 GeoIP 数据 | 国内 IP 段识别（`GEOIP,CN` 分流） | 视上游而定（多为 GPL-3.0 / CC-BY-SA） | https://github.com/Loyalsoldier/geoip |
| **v2fly/domain-list-community** | 域名清单（`GEOSITE,cn` 分流） | **GPL-3.0** | https://github.com/v2fly/domain-list-community |
| **MetaCubeX/meta-rules-dat** | `geoip.dat` / `geosite.dat` / `ASN.mmdb` 编译产物 | GPL-3.0 | https://github.com/MetaCubeX/meta-rules-dat |

> 仓库内 `src-tauri/resources/geo/` 下的 `geoip.dat`、`geosite.dat`、`geoip.metadb`、`ASN.mmdb`
> 均为上述项目的**编译产物**，随本软件分发时同样受其原许可证约束。

---

## 三、桌面应用框架（Rust）

| 组件 | 用途 | 许可证 |
|---|---|---|
| **Tauri 2** | 桌面应用壳（Rust 后端 + WebView 前端） | MIT / Apache-2.0 |
| tauri-plugin-updater | 应用内自动更新 | MIT / Apache-2.0 |
| tauri-plugin-dialog | 原生文件对话框 | MIT / Apache-2.0 |
| tauri-plugin-process | 进程控制（重启等） | MIT / Apache-2.0 |
| **serde / serde_json** | 序列化 | MIT / Apache-2.0 |
| **dirs** | 跨平台目录路径 | MIT / Apache-2.0 |

## 四、前端（Vue / Vite）

| 组件 | 用途 | 许可证 |
|---|---|---|
| **Vue 3** | 前端框架 | MIT |
| **Vite** | 构建工具 | MIT |
| @vitejs/plugin-vue | Vue 的 Vite 插件 | MIT |
| **@tauri-apps/api** | 前端调用 Rust 命令的桥 | MIT / Apache-2.0 |
| @tauri-apps/cli | 打包命令行 | MIT / Apache-2.0 |
| **@xterm/xterm + @xterm/addon-fit** | 内置 SSH 终端（终端模拟器） | MIT |

## 五、AI 控制入口（MCP）

| 组件 | 用途 | 许可证 |
|---|---|---|
| **Model Context Protocol (MCP)** 规范 | AI 客户端与本软件交互的协议 | MIT（Anthropic 发布） |
| Python 标准库 | `server.py` 仅用标准库实现，无第三方 Python 依赖 | PSF License |

---

## 六、本软件自身

| 项目 | 许可证 |
|---|---|
| 尊者魔法代理（Magic Agent）自研代码 | 见根目录 `LICENSE` |

---

## 七、合规检查清单

- [ ] 分发二进制时，随包附带 mihomo 的 GPL-3.0 许可证全文
- [ ] 分发二进制时，随包附带 geo 数据来源说明
- [ ] 仓库根目录提供 `LICENSE`
- [ ] 仓库根目录提供本文件 `THIRD_PARTY.md`
- [ ] README 中说明第三方依赖情况
