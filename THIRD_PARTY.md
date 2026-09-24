# 第三方组件与许可声明（Third-Party Notices）

本软件（尊者网络管理 / Network Manager）**不是从零自研所有部件**。它站在大量优秀开源项目之上，
在此明确列出所依赖的第三方组件、各自用途与许可证，以示尊重与合规。

> 说明：本文件由项目维护者人工维护，用于满足各依赖许可证的「保留版权声明」义务。
> 若发现遗漏，欢迎提 Issue 补充。

---

## 一、代理内核

| 组件 | 用途 | 许可证 | 项目地址 |
|---|---|---|---|
| **mihomo**（原 Clash.Meta，本项目实测版本 `v1.19.29`） | 代理转发内核。本软件**不修改其源码**，仅以外部独立进程方式调用，通过其 RESTful API 控制 | **GPL-3.0** + ⚠️ 附加命名条款 | https://github.com/MetaCubeX/mihomo |

> ⚠️ **重要合规说明**：mihomo 采用 GPL-3.0。本软件通过**独立进程 + 进程间 API 调用**方式使用它，
> 不进行源码链接（static/dynamic linking），因此本软件自身代码不因此被 GPL「传染」。
> 但**分发时所附带的 mihomo 二进制文件本身仍受 GPL-3.0 约束**，必须随附其许可证全文与源码获取方式。

> ⚠️ **附加命名条款（依据 GPL-3.0 第 7 节 Additional Terms）**：mihomo 的 README 明确要求——
> **非 MetaCubeX 官方关联的下游项目，其名称中不得包含 "mihomo" 字样。**
> 本项目名称「尊者网络管理 / Network Manager」不含该词，**已满足此要求**。


---

## 二、GeoIP / GeoSite 数据

仓库内 `src-tauri/resources/geo/` 下随软件分发的数据文件：

| 文件 | 内容 | 上游项目 | 许可证 |
|---|---|---|---|
| `geoip.dat` / `geoip.metadb` | IP 段 → 国家映射（`GEOIP,CN` 分流） | MetaCubeX/meta-rules-dat（基于 MaxMind GeoLite2 等上游数据编译） | **GPL-3.0**（编译产物）；⚠️ 内含 **MaxMind GeoLite2** 数据，另受 MaxMind 数据库许可约束 |
| `geosite.dat` | 域名清单（`GEOSITE,cn` 分流） | MetaCubeX/meta-rules-dat ← v2fly/domain-list-community | **GPL-3.0**（编译产物）；上游域名清单为 **MIT** |
| `ASN.mmdb` | ASN 数据 | 基于 MaxMind GeoLite2 ASN 数据库 | ⚠️ 受 **MaxMind GeoLite2 EULA** 约束（非开源许可） |

> ⚠️ **数据类依赖的许可证并不统一**，需分别注意：
> - **MetaCubeX/meta-rules-dat** = **GPL-3.0**（本项目所用 `.dat` 文件的直接来源）
> - **v2fly/domain-list-community** = **MIT**（域名清单上游）
> - **Loyalsoldier/geoip** = **CC-BY-SA-4.0**（*本项目未使用该来源*，列出仅供对照）
> - **MaxMind GeoLite2** = 专有数据库许可（**非开源**），要求署名并遵守其 EULA

> 上述数据文件均为第三方**编译产物**，随本软件分发时受其原许可证约束。

---

## 三、桌面应用框架（Rust）

| 组件 | 用途 | 许可证 |
|---|---|---|
| **Tauri 2** | 桌面应用壳（Rust 后端 + WebView 前端） | **Apache-2.0 OR MIT**（双许可，二选一） |
| tauri-plugin-updater | 应用内自动更新 | **Apache-2.0 OR MIT** |
| tauri-plugin-dialog | 原生文件对话框 | **Apache-2.0 OR MIT** |
| tauri-plugin-process | 进程控制（重启等） | **Apache-2.0 OR MIT** |
| **serde / serde_json** | 序列化 | **MIT OR Apache-2.0**（双许可） |
| **dirs** | 跨平台目录路径 | **MIT OR Apache-2.0**（双许可） |

> 注：Tauri 系在 `Cargo.toml` / `package.json` 中标注为 `Apache-2.0 OR MIT`（可选择其一）。
> GitHub 仓库页因平台限制只显示其中一个（显示 `Apache-2.0`），**不代表单许可**。
> 本项目选择 **MIT** 分支以保持全项目许可一致。

## 四、前端（Vue / Vite）

| 组件 | 用途 | 许可证 |
|---|---|---|
| **Vue 3** | 前端框架 | MIT |
| **Vite** | 构建工具 | MIT |
| @vitejs/plugin-vue | Vue 的 Vite 插件 | MIT |
| **@tauri-apps/api** | 前端调用 Rust 命令的桥 | **Apache-2.0 OR MIT** |
| @tauri-apps/cli | 打包命令行 | **Apache-2.0 OR MIT** |
| **@xterm/xterm + @xterm/addon-fit** | 内置 SSH 终端（终端模拟器） | MIT |

## 五、协议方案与 AI 控制入口

| 组件 | 用途 | 许可证 |
|---|---|---|
| **XTLS/REALITY** | VLESS + Reality 协议方案（本项目采用其协议设计） | **MPL-2.0**（另含 `LICENSE-Go` 为 BSD-3-Clause） |
| **Model Context Protocol (MCP) 规范** | AI 客户端与本软件交互的协议 | ⚠️ **过渡态**：代码与规范为 **Apache-2.0**；文档（不含规范）为 **CC-BY-4.0**；未获重授权同意的历史贡献仍为 **MIT** |
| Python 标准库 | `server.py` 仅用标准库实现，无第三方 Python 依赖 | PSF License |

> 注：MCP 的**各语言 SDK**（如 `@modelcontextprotocol/sdk`）通常为 MIT，与「规范仓库」不是同一回事。
> 本项目仅**实现** MCP 协议（protocol implementation），未复制其规范文档或代码，因此不受其许可约束。

---

## 六、本软件自身

| 项目 | 许可证 |
|---|---|
| 尊者网络管理（Network Manager）自研代码 | 见根目录 `LICENSE` |

---

## 七、合规检查清单

### 已满足 ✅

- [x] 仓库根目录提供 `LICENSE`（MIT）
- [x] 仓库根目录提供本文件 `THIRD_PARTY.md`
- [x] README 中说明第三方依赖与致谢
- [x] `Cargo.toml` / `package.json` 声明 `license` 字段
- [x] 项目名称未使用 "mihomo" 字样（满足其 GPL-3.0 §7 附加命名条款）
- [x] 未修改 mihomo 源码，仅以独立进程调用 → 自身代码不被 GPL 传染
- [x] Tauri 系双许可选择 MIT 分支，与主许可一致

### 分发二进制时须补充 ✅

以下均随包分发，位于 App 内 `Resources/licenses/` 目录
（源文件见仓库 `src-tauri/resources/licenses/`）：

- [x] 随包附带 **mihomo 的 GPL-3.0 许可证全文**，并提供其源码获取地址
      → `licenses/GPL-3.0.txt` + `licenses/THIRD-PARTY-NOTICES.txt`（含源码地址）
- [x] 随包附带上表所列 **geo 数据来源说明**及各自许可证
      → `licenses/THIRD-PARTY-NOTICES.txt` 第 2 节
- [x] 若包含 MaxMind GeoLite2 数据，须遵守其 EULA（含署名要求）
      → `licenses/THIRD-PARTY-NOTICES.txt` 第 3 节（含 MaxMind 要求的英文署名原文）
- [x] 附带本 `THIRD_PARTY.md` 的等价声明
      → `licenses/THIRD-PARTY-NOTICES.txt` 为随包版本，内容覆盖本文件要点

> 技术实现：上述文件通过在 `tauri.conf.json` 的 `bundle.resources` 中登记，
> 由 Tauri 打包时自动复制进 `.app/Contents/Resources/licenses/`。

### 无需处理 🟢

- MCP：本项目仅实现协议，未复制其代码/文档
- REALITY：本项目采用其**协议设计**（协议本身不受版权保护），未复制其源代码
- Vue / Vite / xterm 等 MIT 组件：保留其版权声明即可，已在本文档中声明

---

## 八、许可证全文获取

| 组件 | 许可证全文位置 |
|---|---|
| 本项目 | 根目录 `LICENSE` |
| mihomo | https://github.com/MetaCubeX/mihomo/blob/Alpha/LICENSE |
| MetaCubeX/meta-rules-dat | https://github.com/MetaCubeX/meta-rules-dat |
| Tauri | https://github.com/tauri-apps/tauri/blob/dev/LICENSE_MIT |
| Vue | https://github.com/vuejs/core/blob/main/LICENSE |
| Vite | https://github.com/vitejs/vite/blob/main/LICENSE |
| xterm.js | https://github.com/xtermjs/xterm.js/blob/master/LICENSE |
| XTLS/REALITY | https://github.com/XTLS/REALITY/blob/main/LICENSE |
| MaxMind GeoLite2 | https://www.maxmind.com/en/geolite2/eula |

