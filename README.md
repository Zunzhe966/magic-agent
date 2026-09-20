# 尊者魔法代理（Magic Agent）

> macOS 桌面软件：**按软件精确分流** + **云服务器 SSH 控制台** + **AI 客户端（MCP）控制入口**。
> 一个软件同时解决「上网代理」和「远程管理服务器」两件事。

---

## 一、这是什么

一个跑在 macOS 上的本地代理工具，核心理念与现有代理软件（Clash Verge / FlClash / Surge 等）**完全不同**：

**现有工具的痛点**：TUN 全接管 + `MATCH` 全局代理，整台电脑几乎所有流量都进隧道，国内国外混在一起，无法精确控制「哪个软件走代理、哪个软件走直连」，只能靠手工猜域名做例外。

**本软件的做法**：不用 TUN 全接管，改用「系统代理 + 进程级规则（PROCESS-PATH-REGEX）」，把本机每个联网软件都变成一张可勾选的清单——每个软件可以精确指定 **走代理 / 直连 / 自动**。

---

## 二、核心能力

### 1. 按软件分流（最大差异化）

- 扫描本机 App（`/Applications`、`/System/Applications`）和常驻后台进程，列出 Chrome、Safari、微信、QQ、Telegram 等。
- 每个软件独立设置：**走代理 / 直连 / 自动**。
- 精确到「Chrome 走云服务器、Safari 走本机、微信直连、Telegram 走节点」这种粒度。
- 支持「裸二进制白名单」：非 `.app` 的可执行文件（如自建监控 daemon）也能按进程路径精确分流。

### 2. 双路出口（决策权在你，不在软件）

软件**不替你做国内外自动分流**，而是给你两条物理上分开的「路」：

| 路 | 端口 | 行为 |
|---|---|---|
| 🚄 坐火车（直连） | `127.0.0.1:7892` | 无条件直连，绝不碰节点 |
| ✈️ 坐飞机（代理） | `127.0.0.1:7893` | 无条件走节点，连国内域名也强制走 |

你（或 AI 智能体）看清目标后自己拍板走哪条，进去后不再被二次判断。配套 `probe_route` 工具可以实测两条路到同一目标的延迟和吞吐，用数据说话。

### 3. 云服务器控制台

- 添加/保存多台服务器（主机、端口、用户名、密钥/密码、备注）。
- 内置 SSH 终端（xterm.js），像 FinalShell 一样直接在软件里敲命令。
- 一键探针：远程采集 CPU / 内存 / 磁盘 / 带宽 / 负载 / 在线时长。
- 连接状态实时显示、快捷命令。

### 4. AI 客户端控制入口（MCP）

`mcp/server.py` 暴露 **23 个工具**，让 Claude / Codex / WorkBuddy 等 AI 客户端能直接：

- 控制代理（启停、切节点、拉订阅、测延迟、看实时连接）
- 实测路由（`probe_route` 双路对比、`download_proxy` 拿双路入口）
- 远程管服务器（`ssh_exec`、`server_metrics`）
- 一键自检（`doctor`）、查免费模型台账（`list_free_models`）

> 这让「AI 需要联网/连服务器时」不再靠猜，而是通过工具拿到真实路况后自主决策。

---

## 三、技术栈与协议（最严格档）

| 层 | 选型 | 理由 |
|---|---|---|
| 桌面壳 | **Tauri 2**（Rust 后端 + Vue3 前端） | 体积 2~10MB、空闲内存 30~50MB，适合 24h 常驻的代理工具（Electron 体积/内存是硬伤） |
| 代理内核 | **mihomo**（外部独立进程调用，一行代码不进工程） | 只做「转发到 VLESS 节点」一件事，协议用别人验证过的成熟实现 |
| 协议 | **VLESS + Reality + XTLS Vision** | 见下 |
| SSH | russh / 系统 ssh | 纯 Rust，交互 PTY + 命令执行 |
| 密钥存储 | macOS Keychain | 密码/私钥不落明文 |

### 协议：为什么说是「最严格」的一档

节点配置使用 VLESS 体系里隐蔽性最高的组合：

```
type: vless
encryption: none          # VLESS 本身已内建认证，无需二次加密
security: reality         # Reality：无证书、无域名、握手即伪装成正常 TLS 流量
flow: xtls-rprx-vision    # XTLS Vision：流量特征伪装，抗主动探测
client-fingerprint: chrome # 指纹伪装成 Chrome 浏览器
```

配套的隐私加固：

- **DNS 全加密**：国内走 DoH（阿里/腾讯 DoH）、境外走 DoT（8.8.8.8 / 1.1.1.1），本机不发起任何明文 DNS 查询。
- **配置文件收紧 `0600`**：`mihomo.yaml` 内含节点 UUID、Reality 公钥、控制 API secret，权限 0600 防止本机其他用户读取。
- **控制 API 带 secret**：`external-controller` 只监听 `127.0.0.1`，且必须带鉴权 secret，防止本机网页 CSRF 攻击。
- **密钥走 Keychain**：SSH 密码/私钥加密存 macOS Keychain，不写明文 JSON。

---

## 四、开发

```bash
# 前端依赖
cd ui
pnpm install

# 运行（需先启动前端 dev server）
pnpm dev          # 终端 1：Vite dev server
cargo tauri dev   # 终端 2：Tauri 应用（在 src-tauri 目录）
```

## 五、构建 .app

```bash
cd ui
pnpm build                     # 构建前端到 ui/dist
cd ../src-tauri
cargo tauri build              # 打包 macOS .app
```

产物在 `src-tauri/target/release/bundle/macos/`。

## 六、自检

```bash
cd src-tauri
cargo test                     # Rust 单元测试
cargo run --bin magic_probe    # 启动代理 -> 8 秒后停止（验证 mihomo 可用）
```

---

## 七、目录结构

```
ui/             Vue3 前端（Vite + xterm.js）
mcp/            Python MCP server（server.py，AI 控制入口，23 个工具）
scripts/        部署/守护/自检脚本（mcp 守护、mihomo 控制、订阅解析）
docs/           需求设计、技术选型、免费模型调研
src-tauri/      Rust 后端（Tauri 2）
  src/
    lib.rs          Tauri command 注册、冲突检测、代理启停
    mihomo.rs       mihomo 配置生成与进程管理
    apps.rs         macOS App 扫描与分类
    ssh.rs          SSH 会话（系统 ssh + expect / key）
    keychain.rs     macOS Keychain 封装（密码/私钥安全存储）
    config.rs       配置持久化、VLESS 订阅解析
    system_proxy.rs 系统代理设置（networksetup）
  resources/
    bin/mihomo      代理内核
    geo/            geoip/geosite 数据
```

---

## 八、分流模型

**三层漏斗规则引擎**（mihomo 规则首条命中生效，顺序即优先级）：

1. **进程级（谁进隧道）**：防卷优先——节点服务器自身流量永远直连（`IP-CIDR,<server>/32,DIRECT,no-resolve`）
2. **域名级（去了哪）**：显式域名规则（target 支持 `direct`/`proxy`/节点名）→ 国内清单直连（`GEOSITE,cn` + `GEOIP,CN`）
3. **节点级（谁来送）**：进程规则把该软件剩余流量交给指定节点（`PROCESS-PATH-REGEX`），最后 `MATCH,DIRECT` 兜底

**统一本地端口：7891**。所有需要显式指定出站代理的外部程序都应指向此端口，流量进入 mihomo 后仍按域名级分流裁决。

---

## 九、已知边界（诚实说明）

- macOS 上「强制某软件 100% 走/绕代理」做不到绝对，系统代理只对遵守系统代理的软件生效（浏览器、大部分 App 遵守）；Telegram 默认直连、部分游戏/命令行工具需要额外手段。
- 启动时会自动清理 FlClash 等第三方代理并恢复系统代理为干净状态（v0.2.2 起），保证本程序是系统唯一代理。
- 当前仅支持 macOS。

---

## 十、安全与隐私

- 所有敏感信息（真实 IP、VLESS 凭据、用户名）在仓库中均已脱敏为占位值。
- 本地配置存 `~/Library/Application Support`，不上传任何云端。
- 详细设计见 `docs/设计.md`；架构红线见 `CONTRACT.md`；版本历史见 `HISTORY.md`。
- 开源/私密资产边界见 [`OPEN_SOURCE_INVENTORY.md`](./OPEN_SOURCE_INVENTORY.md)。

---

## 十一、开源协议

本项目自研代码以 **MIT License** 发布，详见根目录 [`LICENSE`](./LICENSE)。

**为什么选 MIT**：本项目定位是「工具型桌面软件」，希望别人能自由使用、学习、修改、二次开发，
MIT 是最简洁、限制最少、兼容性最好的选择，也便于与上下游生态（Tauri / Vue 等均为 MIT）保持一致。

### 协议兼容性说明

本项目的第三方依赖许可证情况（详见 [`THIRD_PARTY.md`](./THIRD_PARTY.md)）：

| 依赖 | 许可证 | 与本项目 MIT 的关系 |
|---|---|---|
| mihomo（代理内核） | **GPL-3.0** + 附加命名条款 | ⚠️ **以独立进程方式调用，不做源码链接**，因此不传染本项目代码；但分发其二进制时须随附 GPL-3.0 许可证。其附加条款要求下游项目名不含 "mihomo" 字样（本项目已满足） |
| geoip / geosite / ASN 数据 | **GPL-3.0**（含 MaxMind 数据条款） | 作为**数据文件**随包分发，非代码链接；MaxMind GeoLite2 部分另受其 EULA 约束 |
| Tauri 系 | **Apache-2.0 OR MIT**（双许可） | ✅ 本项目选 MIT 分支，完全兼容 |
| Vue / Vite / xterm 等 | MIT | ✅ 完全兼容 |
| XTLS/REALITY | MPL-2.0 | ✅ 仅采用协议设计，未复制源码 |
| MCP 规范 | Apache-2.0 / CC-BY-4.0 / MIT（过渡态） | ✅ 仅实现协议，未复制代码或文档 |

---

## 十二、致谢（站在巨人的肩膀上）

本软件不是从零造轮子，而是把优秀开源项目组装成解决具体问题的工具。特别感谢：

- **[mihomo](https://github.com/MetaCubeX/mihomo)**（MetaCubeX）—— 稳定强大的代理内核，本项目只写「配置生成 + 进程管理」，把转发交给久经考验的实现
- **[Tauri](https://tauri.app/)**（Tauri Apps）—— 让 Rust + WebView 的轻量桌面应用成为可能
- **[Vue.js](https://vuejs.org/)** / **[Vite](https://vitejs.dev/)** —— 前端框架与构建工具
- **[xterm.js](https://xtermjs.org/)** —— 内置 SSH 终端的终端模拟器
- **[MetaCubeX/meta-rules-dat](https://github.com/MetaCubeX/meta-rules-dat)** / **[v2fly/domain-list-community](https://github.com/v2fly/domain-list-community)** —— 国内域名与 IP 清单
- **[XTLS / REALITY](https://github.com/XTLS/REALITY)** —— VLESS + Reality 协议方案
- **[Model Context Protocol](https://modelcontextprotocol.io/)**（Anthropic）—— 让 AI 客户端能标准化地控制本软件

完整清单与许可证见 [`THIRD_PARTY.md`](./THIRD_PARTY.md)。
