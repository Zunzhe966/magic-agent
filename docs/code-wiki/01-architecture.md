# 01 · 整体架构

## 1. 技术栈总览

| 层 | 选型 | 版本（Cargo.toml / package.json） | 存在的理由 |
|---|---|---|---|
| 桌面壳 | **Tauri 2** | `tauri = "2"` | 体积 2~10MB、空闲内存 30~50MB，适合 24h 常驻（Electron 是硬伤） |
| 后端语言 | **Rust**（edition 2021） | `magic-agent` v0.2.10 | 单一语言管进程/网络/文件，无 GC 停顿 |
| 前端 | **Vue 3 + Vite 5** | `vue ^3.4.21`, `vite ^5.2.11` | 页面少（7 个视图），单文件组件足够，无需路由/Pinia |
| 终端组件 | **@xterm/xterm** | `^5.5.0` + `@xterm/addon-fit ^0.10.0` | 只负责渲染文字，SSH 会话逻辑自研 |
| 代理内核 | **mihomo**（外部独立二进制） | 随包分发于 `src-tauri/resources/bin/mihomo` | 只做"转发到 VLESS 节点"，用成熟实现 |
| 代理协议 | **VLESS + Reality + XTLS Vision** | — | 现行隐蔽性最高组合，抗主动探测 |
| SSH | **系统 `ssh` + `expect`** | `/usr/bin/ssh`, `/usr/bin/expect` | 交互 PTY + 非交互执行，避免引入 russh 复杂度 |
| 凭据存储 | **macOS Keychain** | via `/usr/bin/security` | 密码/私钥不落明文 |
| AI 集成 | **MCP（stdio / 本地 HTTP）** | 纯标准库 Python 3 | AI 客户端标准化控制本软件 |

> 注意：`docs/设计.md` 中列出的 `sysinfo` / `russh` / `keyring` 三个 crate 是**早期选型设想**，
> 实际 `Cargo.toml` 未引入 —— 当前实现是**直接调用系统命令**（`ps`/`lsof`/`plutil`/`ssh`/`expect`/`security`）。
> 这是本仓库"零重依赖"的实际取舍。

---

## 2. 进程模型

```
                     ┌───────────────────────────────────────────────┐
                     │            Tauri 桌面 App（普通用户权限）      │
                     │  ┌─────────────────────────────────────────┐  │
                     │  │  WebView（Vue3 UI，ui/dist 由 Vite 构建）│  │
                     │  └───────────────┬─────────────────────────┘  │
                     │           Tauri IPC（invoke / event）          │
                     │  ┌───────────────▼─────────────────────────┐  │
                     │  │  Rust 主线程：AppState + 命令注册表      │  │
                     │  │  · 看门狗线程（30s 轮询，自动拉起内核）  │  │
                     │  │  · 启动清理线程（800ms 后清第三方代理）  │  │
                     │  │  · spawn_blocking 线程池（慢 IO）        │  │
                     │  └───────┬──────────────────┬──────────────┘  │
                     └──────────┼──────────────────┼─────────────────┘
                                │ osascript 提权    │ sudo -n（免密）
                                ▼                  ▼
                    ┌───────────────────┐  ┌──────────────────────────┐
                    │  mihomo 内核(root) │  │ mihomo-ctl.sh（特权控制器）│
                    │  · mixed-port 7891 │  │ /usr/local/lib/magic-agent│
                    │  · 7893 → PROXY    │  │ start/stop/reload/status │
                    │  · 7892 → DIRECT   │  └──────────┬───────────────┘
                    │  · API 127.0.0.1:19091│            │
                    │  · TUN auto-route:false│           │
                    └───────────────────┘  ◄─────────────┘
                                ▲
                                │ 同一份 config.json + mihomo.yaml
                     ┌──────────┴──────────┐
                     │  MCP Server（Python）│  ← AI 客户端（stdio / HTTP 19092）
                     └─────────────────────┘
```

**关键点**：

- **mihomo 以 root 运行**（TUN 的硬约束），因此无法作为普通子进程管理 —— 代码只记录 PID，靠 `pgrep -f "magic-agent/runtime/bin/mihomo"` 重新发现。
- **内核对 App 无依赖**：内核二进制在启动时被复制到 `~/Library/Application Support/magic-agent/runtime/bin/mihomo` 常驻，App 升级替换 `.app` 不会触碰正在运行的内核。
- **MCP Server 与 App 平级**：两者共享同一份 `config.json` 与 `runtime/mihomo.yaml`，可独立启停内核。

---

## 3. 三个入口

| 入口 | 代码位置 | 通信方式 | 适用场景 |
|---|---|---|---|
| **GUI** | `ui/src/**` + `src-tauri/src/lib.rs` | Tauri IPC（`invoke`） | 人操作：点按钮、看连接、敲终端 |
| **MCP** | `mcp/server.py` | JSON-RPC over stdio / HTTP 19092 | AI 客户端（Claude / Codex / WorkBuddy）自主控制 |
| **CLI** | `src-tauri/src/bin/*.rs` | 直接调用 `magic_agent_lib` | 自检 / 配置对账（`magic_probe`、`dump_conf`） |

**一致性铁律**（`CONTRACT.md` 规定）：GUI（Rust 命令）与 MCP 工具实现同一功能时，**副作用必须一致**。
历史事故：MCP 侧 `stop_proxy` 不做系统代理联动 → 走 MCP 停代理后系统代理仍指向已关闭端口 → 用户断网。
因此 `scripts/check_parity.py` 会对两侧生成的 mihomo 规则逐行 diff。

---

## 4. 端口分配（全局统一）

| 端口 | 归属 | 行为 | 代码常量 |
|---|---|---|---|
| **7891** | mihomo `mixed-port` | 统一本地入口，进入后按**规则**裁决；绑 `127.0.0.1` | `MihomoManager::port`（默认 7891） |
| **7892** | mihomo `listeners: direct-only` | **无条件 DIRECT**，绝不碰节点；显式 `listen: 127.0.0.1` | `mihomo::DIRECT_PORT` |
| **7893** | mihomo `listeners: proxy-only` | **无条件 PROXY**，连国内域名也强制走节点；显式 `listen: 127.0.0.1` | `mihomo::PROXY_PORT` |
| **19091** | mihomo `external-controller` | 控制 API（带 `secret` Bearer 鉴权），只监听回环 | `mihomo::API_PORT` |
| **19092** | MCP Server HTTP 桥接 | 本地 HTTP（Origin 白名单 + Bearer 令牌） | `mcp/server.py` 默认值 |
| **7878** | 本地更新源 | `latest.json` 静态服务（仅 `local` 通道） | `updater::ENDPOINT_LOCAL` |
| **1054** | mihomo DNS listener | `127.0.0.1:1054` | `build_conf()` |
| **5173** | Vite dev server | 开发期前端热更新 | `ui/vite.config.js` |

> **7892 / 7893 是本项目最核心的产品设计**：mihomo 不替用户判断国内外，而是给两条物理上分开的路，
> 由用户或 AI 实测后自己拍板。代码上用 `listeners[].proxy` **加** `IN-PORT` 规则**双保险**（见 `build_rules`）。
>
> ⚠️ **监听红线（2026-09-23）**：listeners 必须逐条显式 `listen: 127.0.0.1`——实测 `allow-lan: false`
> 管不到 listeners 段、`bind-address` 字段对 listener 无效，缺 `listen:` 时内核绑 0.0.0.0 暴露局域网。
> Rust 单测已锁死该断言（CONTRACT.md 禁区）。

---

## 5. 配置与数据流

### 5.1 文件布局

```
~/Library/Application Support/magic-agent/
├── config.json            # 主配置（0600）：nodes / apps / domainRules / servers / apiSecret / updateChannel
├── config.json.corrupt    # 解析失败时的备份（只在不存在时写入，二次事故不覆盖好备份）
└── runtime/
    ├── mihomo.yaml        # 生成的完整内核配置（0600，含 UUID / reality 公钥 / secret）
    ├── mihomo             # 内核常驻副本（0755，App 升级不影响运行中的内核）
    ├── mihomo.log         # stdout（超 10MB 轮转为 .log.old）
    ├── mihomo.err.log     # stderr
    ├── geoip.dat / geosite.dat / ASN.mmdb / geoip.metadb   # 从 resources/geo 复制
    └── http-bridge.token  # MCP HTTP 桥接令牌（0600）
```

> 注：`mihomo.rs` 中 `runtime_dir` 用 `dirs::config_dir()`，在 macOS 上解析为 `~/Library/Application Support`；
> `config.rs::config_path()` 同样基于 `dirs::config_dir()`。测试文件中出现过 `~/Library/Application Support/magic-agent/...` 的绝对路径。

### 5.2 配置读取的健壮性设计（`config.rs::load`）

这是本项目最值得注意的数据安全设计，三层防护：

```
读 config.json
   │
   ├─ 文件不存在 → 生成默认配置 + 新 apiSecret，落盘返回
   │
   ├─ 解析成功 → 补齐空 apiSecret 后返回
   │
   └─ 解析失败 → ① backup_once（只备份，绝不覆盖已有 .corrupt）
                ② recover_from_backup 尝试自愈
                ③ 自愈失败 → 仅内存兜底 Default，**绝不动磁盘上的用户文件**
```

**为什么**：早期版本解析失败会把空配置写回，导致"数据凭空消失"。`parse_config_str` 先 parse 成
`serde_json::Value`（重复键取最后一个）再递归 `normalize_keys_to_camel`，避免旧版 snake_case 键
与新版 camelCase 键共存时被字符串替换制造重复键 → serde 直接 Err → 整份配置判损。

### 5.3 原子写（`config.rs::save`）

```
写 config.json.tmp → chmod 0600 → rename 到 config.json
```

> `chmod` **必须在 `rename` 之前** —— 若顺序反了，rename 完成到 chmod 之间会有一个
> 其他本机用户可读取 secret 的短窗口（tmp 默认 0644）。

---

## 6. 分流模型（三层漏斗）

mihomo 规则**首条命中生效**，顺序即优先级。`mihomo.rs::build_rules` 按以下顺序生成：

```
第 -1 层  IN-PORT,7893,PROXY            ← 两条「路」死锁（双保险，优先级最高）
          IN-PORT,7892,DIRECT
第  0 层  IP-CIDR,<保命域名>/32,DIRECT   ← PROTECTED_DIRECT_DOMAINS，永远走原生路由
第  1 层  IP-CIDR,<节点server>/32,DIRECT ← 防卷：节点服务器自身流量绝不进隧道（否则死循环）
          DOMAIN-SUFFIX,<节点server>,DIRECT
第  2 层  DOMAIN-SUFFIX,<用户域名规则>,<target>   ← 显式域名规则（direct/proxy/节点名）
          GEOSITE,cn,DIRECT                        ← 国内域名清单直连
          GEOIP,CN,DIRECT                          ← 国内 IP 直连
第  3 层  PROCESS-PATH-REGEX,^<App路径>,<target>   ← 按软件分流（核心差异点）
第  4 层  GEOIP,LAN,DIRECT,no-resolve
          MATCH,DIRECT                             ← 兜底
```

**为什么第 1 层必须在最前**：节点服务器流量若被自己代理 → 代理套代理 → 死循环/超时。
**为什么要 `PROCESS-PATH-REGEX` 而不是 `PROCESS-NAME`**：前者匹配可执行文件完整路径，能区分
`Chrome` 主程序与其 Helper 子进程，也支持"裸二进制白名单"（非 `.app` 的命令行工具）。

### 6.1 软件分流的生效条件

`lib.rs::settings_to_app_rules` 是**纯函数**（GUI 与 `dump_conf` bin 共用）：

- 只处理 `confirmed == true` 的条目（未确认不进规则表，由 `MATCH,DIRECT` 兜底）。
- `mode == "proxy"` 且 `node` 是**现存节点** → 目标为 `NODE-<sanitize_node_name(节点名)>`；
  节点已被删除则**降级为 `PROXY`**（否则引用不存在的组，mihomo 拒绝整个配置）。
- 其他情况 → `DIRECT`。

### 6.2 组名净化一致性（有回归测试钉死）

节点名会同时出现在两处：`proxy-groups` 的 `name` 和规则的 `target`。
必须使用**同一个** `sanitize_node_name` 结果，否则组名与引用对不上 → 规则**静默失效**。

- `sanitize_node_name`：**保留中文与空格**（对 mihomo 组名合法），只剔除 `,` `\n` `\r` 及控制字符。
- `sanitize_rule_field`（用于域名/IP）：只保留 ASCII 字母数字与 `. - _ : * / # [ ]`。
- `yaml_quote`：把任意字符串安全嵌入 YAML 双引号串，防订阅注入（如改 `allow-lan`、去 secret）。
- `regex_escape_path`：转义 `PROCESS-PATH-REGEX` 的路径中的正则元字符。

---

## 7. 安全设计清单

| 面向 | 措施 | 代码位置 |
|---|---|---|
| 控制 API | `secret` Bearer 鉴权 + 只监听 `127.0.0.1` | `mihomo.rs::build_conf`、`lib.rs::proxy_api` |
| 前端隔离 | `get_config` 把 `apiSecret` 置 `None`；前端永不直连 19091，统一走 `proxy_api` 命令 | `lib.rs::get_config`、`proxy_api_blocking` |
| CSP | `default-src 'self'`，`connect-src 'self' ipc:` | `tauri.conf.json → app.security.csp` |
| SSRF | 订阅 URL 只允许 http/https，拒绝回环/内网/链路本地/组播/CGNAT | `config.rs::validate_public_host`、`is_private_or_reserved` |
| HTTP 注入 | `proxy_api` 拒绝不含 `/` 开头、含 `://` 路径段、含 `\r\n` 的 path | `lib.rs::proxy_api_blocking` |
| 配置注入 | `yaml_quote` / `sanitize_rule_field` / `sanitize_node_name` | `mihomo.rs` |
| 文件权限 | `mihomo.yaml` 0600、`config.json` 0600、临时私钥 0600、**运行日志 0600（root 启动路径 umask 077 + chmod，2026-09-23）** | `mihomo.rs::write_conf`、`config.rs::save`、`ssh.rs::write_temp_key`、`mihomo.rs::start`、`scripts/mihomo-ctl.sh` |
| 监听范围 | mixed-port 与两条 listener 全部钉死 `127.0.0.1`（listener 用 `listen:` 字段；`allow-lan`/`bind-address` 均管不到） | `mihomo.rs::build_conf`、`mcp/server.py::generate_config` |
| 凭据 | SSH 密码/私钥只进 Keychain；`security add-generic-password -w`（值走 stdin 不进 argv） | `keychain.rs`、`ssh.rs` |
| 子进程匹配 | 只匹配**可执行文件路径**，绝不匹配整个命令行（否则 `grep clash` 会被误杀） | `lib.rs::find_foreign_proxies` |
| 误杀防护 | 绝不匹配宽泛的 `proxy` / `mihomo`；跳过自身进程与 runtime 目录内核 | 同上 |

---

## 8. 生命周期与自愈机制

### 8.1 退出收尾（`lib.rs::run` 的 `RunEvent::Exit`）

```
RunEvent::Exit（关窗 / Cmd+Q / app.exit() / 系统注销，所有路径必经）
  ├─ should_run.store(false)   ← 先通知看门狗别自动拉起
  ├─ mihomo.stop()             ← 停内核（含提权 kill 兜底）
  ├─ ssh.disconnect()          ← 断 SSH，否则 ssh/expect 子进程变孤儿
  └─ set_system_proxy(false)   ← 关系统代理，避免死代理端口导致全机断网
```

> 这是 2026-09-03 事故的根因修复：此前 App 退出无清理钩子，root mihomo 变孤儿进程
> 继续用 TUN 接管全机流量，劫持其他应用（表现为 `ECONNRESET` / 502）。
> **`RunEvent::Exit` 钩子是禁区，不得删除。**

### 8.2 看门狗线程（每 30s）

```
should_run == false → 跳过（用户本来就希望它停）
mihomo.status().running → 跳过（健康）
否则 → ctl("start") 零弹窗重启
        ├─ 成功且 wait_api() → 更新 PID，continue
        └─ 失败 → 关闭系统代理（避免"内核死了但系统代理还指着死端口"→ 全机断网）
```

### 8.3 内核状态判定（`MihomoManager::status`）

```
内存 PID 存活？
  ├─ 否 → pgrep -f "magic-agent/runtime/bin/mihomo" 重新发现（App 重启后 PID 丢失）
  │        └─ 不能只看端口：第三方程序占用 7891+19091 会造成假"运行中"
  └─ 是 → 再确认控制 API 19091 可连接（connect_timeout 500ms）
           └─ 否则"孤儿进程/半启动"会被误报为可用
```

> `connect_timeout` 必须带超时：此处持有 `pid` 锁，无超时的 connect 在异常网络栈下
> 长时间阻塞会把所有并发 `status` 调用串行卡死。

### 8.4 启动即清理第三方代理

```
App 启动 800ms 后（后台线程）+ 每次 start_proxy 前
  → find_foreign_proxies()   扫描 ps -axo pid=,args=
  → SIGTERM 全部 → 等 1.5s → 仍存活者 SIGKILL
  → set_system_proxy(false)  把系统代理恢复为"未设置"
```

匹配名单（`FOREIGN_PROXY_APPS` + `FOREIGN_PROXY_CORES`）：FlClash、Clash Verge、ClashX、
Clash for Windows、Clash Nyanpasu、Clash.Meta、V2Ray/Xray、ShadowsocksX、Surge、Quantumult、
Stash、Loon、sing-box、Trojan、NaiveProxy、Hysteria 等，以及它们的**内核进程名**。

---

## 9. 性能约束（卡顿事故的经验沉淀）

Tauri 的 `#[tauri::command]` **默认同步执行在主线程**，任何阻塞 IO 都会冻住整个 UI（macOS 彩圈）。
以下命令已全部 `async` + `spawn_blocking`（**改回同步 = 重演卡顿**）：

| 命令 | 阻塞原因 | 最长耗时 |
|---|---|---|
| `get_status` | `TcpStream::connect` 探端口 + `scutil` 子进程 | 被**每 5 秒轮询**，尤其敏感 |
| `scan_apps` | 全盘扫描 App + `lsof` | 秒级 |
| `start_proxy` | 停旧进程 + cleanup + 等 API | 数十秒 |
| `server_metrics` | SSH 远程执行 | 20s |
| `ssh_exec` / `ssh_connect` | SSH 建连 + 认证 | 10~20s |
| `proxy_api` | TCP 直连 mihomo API（read timeout 15s） | 高频调用 |
| `fetch_subscription` | `curl` 拉订阅 | 15s |
| `set_system_proxy` | `networksetup` | 秒级 |
| `save_config` | 热更新要调 mihomo API | 秒级 |
| `check_conflicts` / `kill_foreign_proxies` / `list_foreign_proxies` | `ps` 扫描 + SIGTERM 等待 | 1.5s |

**配套优化**：

- `apps_cache`：`scan_apps` 结果缓存进 `AppState`，`get_status` 只读 `len()`（不重扫），
  `start_proxy`/`save_config` 复用（`effective_app_rules_with`）。
- `lsof` 硬超时 2500ms：超时即 kill 子进程并放弃联网标记，不阻塞扫描。
- async 命令**必须返回 `Result`**（Tauri 硬性要求）。
- 前端 `onMounted` 中 `scan_apps` 后台异步**不 await**，首屏只拉轻量状态 + 配置。

---

## 10. 目录结构对照

```
魔法代理/
├── ui/                      Vue3 前端
│   ├── src/
│   │   ├── App.vue          根组件 + 总控（视图切换、全局状态、5s 轮询）
│   │   ├── main.js          应用引导
│   │   ├── toast.js         轻量全局提示（函数式 API）
│   │   ├── updater.js       更新模块单例（reactive state）
│   │   ├── styles/main.css  全局样式与 CSS 变量
│   │   └── components/      7 个业务视图 + ServerDashboard
│   ├── dist/                构建产物（Tauri frontendDist）
│   ├── index.html
│   ├── vite.config.js
│   └── package.json
├── src-tauri/               Rust 后端
│   ├── src/
│   │   ├── main.rs          二进制入口（4 行）
│   │   ├── lib.rs           组合根：命令注册、冲突检测、代理启停、SSH 命令、退出钩子
│   │   ├── mihomo.rs        内核管理：配置生成、进程生命周期、特权控制器
│   │   ├── config.rs        配置持久化、键名兼容、订阅解析、SSRF 校验
│   │   ├── apps.rs          macOS App/进程扫描与分类
│   │   ├── ssh.rs           SSH 交互会话 + 非交互执行（expect 喂密码）
│   │   ├── system_proxy.rs  系统代理读写（networksetup / scutil）
│   │   ├── keychain.rs      macOS Keychain 封装
│   │   ├── updater.rs       双通道更新（local / github）
│   │   └── bin/
│   │       ├── magic_probe.rs  启停探针（起 8 秒停）
│   │       └── dump_conf.rs    stdin 配置 → stdout 规则（给 check_parity 用）
│   ├── resources/
│   │   ├── bin/mihomo       内核二进制（随包分发）
│   │   ├── geo/             geoip/geosite/ASN 数据
│   │   └── licenses/        GPL-3.0 / MIT / THIRD-PARTY-NOTICES
│   ├── icons/               图标（母版 source-icon.png）
│   ├── capabilities/default.json
│   ├── Cargo.toml
│   └── tauri.conf.json
├── mcp/
│   ├── server.py            MCP Server（工具定义 + 协议层）
│   └── README.md
├── scripts/                 工具链（发版/图标/特权控制器/守护/校验）
├── tests/test_regressions.py  Python 侧回归测试（规则对账、工具 schema）
├── docs/                    产品文档 + 本 Code Wiki
├── README.md / CONTRACT.md / HISTORY.md / THIRD_PARTY.md / OPEN_SOURCE_INVENTORY.md
└── graphify-out/            代码图谱工具的产物（非源码）
```
