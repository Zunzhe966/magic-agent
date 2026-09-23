# 版本历史与变更记录（HISTORY）

> **这份文档记录：每个版本做了什么、对应哪个 git 提交号，以及开发中的「想法变化」和「增/删项」。**
> 项目只有一条主线（`main` 分支），**历史全部保存在 git 里，不另设 backup 目录**。
> 架构红线见 `CONTRACT.md`；产品设计见 `docs/设计.md`；使用说明见 `README.md`。
> 完整原始记录用 `git log`（本文档是它的「人类可读摘要」）。

---

## 版本总览

| 版本 | 日期 | git tag | 节点提交 | 一句话 |
|------|------|---------|----------|--------|
| v0.0.1 | 2026-08-20 | `v0.0.1` | `2425d2a` | 首个可运行版本：进程级智能分流 + 云服务器控制台 |
| v0.0.2 | 2026-08-21 | `v0.0.2` | `18823a9` | 订阅拉取、冲突检测、Keychain、节点管理 |
| v0.0.3 | 2026-08-21 | `v0.0.3` | `3a734cd` | TUN 模式 + 特权 mihomo，进程分流可靠化 |
| v0.0.4 | 2026-08-21 | `v0.0.4` | `161f593` | MCP 服务：AI 可控制魔法代理 |
| v0.0.5 | 2026-08-21 | `v0.0.5` | `c411cad` | 域名分流规则 |
| v0.1.0 | 2026-08-22 | `v0.1.0` | `ddd165e` | 加密 DNS（DoH/DoT）+ 按软件选节点 |
| v0.2.0 | 2026-08-31 | `v0.2.0` | `154083a` | 云服务器管理与双路探测 |
| v0.1.1 | 2026-09-03 | `v0.1.1` | `2e7aa82` | 修复 mihomo 孤儿进程劫持全机流量事故 |
| v0.2.2 | 2026-09-19 | `v0.2.2` | `45ac703` | 启动即清理第三方代理 + 卡顿修复 + 规范发版流程 |
| v0.2.3 | 2026-09-19 | `v0.2.3` | `f9a0a50` | 全部慢命令 async 化，根治点仪表盘彩色转圈 |
| v0.2.4 | 2026-09-19 | `v0.2.4` | `56e02ee` | get_status 也 async 化，根治 5 秒轮询卡顿 |
| v0.2.5~0.2.10 | 2026-09-20~22 | — | 多提交 | 开源合规（MIT+第三方许可随包）、图标更新、双通道更新器、MCP 参数健壮化 |
| v0.2.11 | 2026-09-23 | —（未打 tag） | 工作区 | **信任版（P0）**：P0-1 系统代理逐服务对账（消灭 any_success 假账 + parse_port 潜伏 bug）、P0-4 崩溃自愈（启动自检死端口残留自动恢复直连 + UI 提示）；前序工作区含 listener 钉回环、root 日志收权、文档全面重写（见下） |

> 版本号以 `tauri.conf.json` 为准。v0.2.1 只改过内部版本号、未单独发版。

---

## 完整提交记录（按时间）

### 2026-08-20 · v0.0.1 起点

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `2425d2a` | feat | 首个可运行版本：Tauri 2 + mihomo，软件级智能分流 + 云服务器 SSH 控制台 |

**想法变化**：从"做个代理工具"确定为"**按软件分流 + 服务器管理合一**"的产品方向——不做又一个 FlClash。

### 2026-08-21 · v0.0.2 ~ v0.0.5（密集开发日）

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `63e866f` | cleanup | **删除废弃的 Electron 版本**，只保留 Tauri 实现 |
| `18823a9` | feat/fix | 订阅拉取、冲突检测、Keychain 存凭据、节点管理；修 ssh `-p` 重复、独立停止 |
| `f2cbd39` | test | 新增 VLESS URI 解析单测 |
| `e4ef350` | docs | 加 README（开发/构建/自测说明） |
| `06f8cd7` | feat | SSH 快捷命令 + 连接错误处理 |
| `2d23cfa` | feat | 保存的 SSH 服务器管理 |
| `641b0b8` | fix | 修 Vue scope bug、ssh expect 临时文件、冲突自检测、Safari WebKit 规则 |
| `36b8028` | fix | 修仪表盘代理开关、ssh session id 一致性 |
| `cd6d5cd` | fix | 保留 app 规则流里的 confirmed/node 字段 |
| `1e40b9c` | fix | networksetup 代理错误显式抛出（不再静默失败） |
| `a0fb943` | perf | SSH 终端输入缓冲，降低 IPC 频率 |
| `077d7e1` | chore | 简化 applyApps 的 confirmed 标记 |
| `09b637a` | ux | 选保存的服务器后自动切到 SSH 控制台 |
| `3a734cd` | feat | **TUN 模式 + 特权 mihomo**，进程分流可靠化 |
| `fcd597a` | test | 用 `mihomo -t` 真校验生成的 TUN 配置 |
| `04105a7` | fix | 特权 mihomo 启动去掉 nohup（无 TTY 会失败） |
| `d18f5bc` | fix | 显式声明 main bin，确保打包 magic-agent 而非 magic_probe |
| `c0177a3` | fix | start_proxy 已在跑时立即返回（避免静默重启） |
| `161f593` | feat | **MCP 服务**：AI 可控制魔法代理 |
| `e8377e2` | fix | 按端口检测运行中的 mihomo，UI 按钮状态才正确 |
| `5bf8d2c` | fix | 轻量轮询（不扫全量 App），AppsView 显示每 App 连接状态 |
| `a3a291d` | fix | 去掉 auto 模式选项，缓存 App 列表，仅在 Apps 页可见时刷新 |
| `c9d5d33` | feat | AppsView 运行中的 App 排前面 |
| `c411cad` | feat | **域名分流规则**（解决同源下载混杂），新增 MCP 工具 |
| `f6356f6` | fix | MCP generate_config 漏了 app 规则（丢 PROCESS-PATH-REGEX），改用 PATCH 热重载 |
| `ce0fc09` | chore | 忽略 pycache |
| `e422797` | fix | switch_node 走 API 不重启；check_network 经代理测 google |
| `6c818b3` | feat | 新增 fetch_subscription、test_node_delay 两个 MCP 工具 |
| `8f9ef93` | feat | ServersView 加节点延迟测试按钮 |
| `71c48fb` | docs | MCP 部署指南 |
| `d3c71db` | fix | 热重载改用 SIGHUP（mihomo PATCH 会丢多条 PROCESS-PATH-REGEX 规则） |

**想法变化**：
- **放弃 Electron**（`63e866f`）——体积/内存不适合 24h 常驻。
- 分流从"只靠系统代理"演进到 **TUN 模式**（`3a734cd`），因为纯系统代理管不住 Telegram 等直连软件。
- 意识到"全量 App 扫描太慢"（`5bf8d2c` `a3a291d`）——**卡顿问题从这里就埋下了**。
- 学会用 `mihomo -t` 真校验配置，不再靠"看起来对"。

### 2026-08-22 · v0.1.0

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `ddd165e` | feat | **加密 DNS（DoH/DoT）**、**按软件单独选节点**、新增美国德州住宅节点 |

**想法变化**：隐私加固升级（DNS 全加密）；分流从"按软件开关"细化为"按软件选具体节点"。

### 2026-08-31 · v0.2.0

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `154083a` | feat | **云服务器管理与双路探测** + 全量脱敏与文档对齐 |
| `bde4fc7` | chore | 换专属图标（竹子+能量球），应用更名「尊者魔法代理」 |
| `a4106c4` | docs | 新增完整说明书 README 与产品优势对比文档 |

**想法变化**：从"能连服务器"升级为"**双路探测**（直连/代理对照，判断节点是否真的生效）"；产品正式命名。

### 2026-09-03 · v0.1.1（事故修复）

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `2e7aa82` | fix | **修复 App 退出后 mihomo 内核变孤儿继续劫持全机流量**（2026-09-03 事故） |

**想法变化（重要教训）**：App 生命周期**必须**完整覆盖内核生命周期，否则 root 权限的孤儿内核会接管全机流量、导致大面积断网/502。此后所有"进程清理"逻辑都以此为准。

### 2026-09-19 · v0.2.2（当前版本）

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `45ac703` | feat/fix | **启动即清理第三方代理** + **卡顿修复** + 建立**规范发版流程**；并把此前多轮积压的修复一并入库 |

**想法变化（重要）**：
1. **第三方代理处理逻辑反转**：从"检测到冲突就拒绝启动"改为"**启动时自动清理第三方，让本程序成为系统唯一代理**"——用户不该被迫手动去关别的软件。
2. **卡顿根因确认**：`scan_apps` 的 `lsof` 全机扫描（1~3 秒）+ 启动时同步等待 → 加 2.5s 硬超时 + 改为后台异步。
3. **发版流程正规化**：确立铁律——**必须走 `scripts/release.sh`（签名 + updater-feed），禁止手工 cp 覆盖**。此前一直手工 cp，导致更新机制形同虚设。
4. 承认"改完不提交 = 源码与 App 脱节"，首次把积压的修复全部提交入库。

### 2026-09-20 · 文档整理

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `123597c` | docs | 新增 HISTORY.md，为 9 个版本节点打 tag |
| (本次) | docs | 文档精简：3 份产品文档合并为 `docs/设计.md`；3 份模型文档合并为 `docs/免费模型.md`；删除过期计划；HISTORY 升级为含提交号的完整变更记录 |

**想法变化**：文档从"散落多份、有重复"收敛为"**少数权威文档**"——README（用法）、CONTRACT（红线）、HISTORY（历史）、docs/设计（设计）、docs/免费模型（模型数据）。

### 2026-09-19 · v0.2.3 根治仪表盘卡顿

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `f9a0a50` | fix/perf | 全部慢命令 async 化，点仪表盘不再彩色转圈 |

**根因**：Tauri 的 `#[tauri::command]` 默认**同步命令跑在主线程**。点「云服务器仪表盘」触发 `server_metrics`，它内部做 SSH 连接 + 执行命令（最长 20s 超时），阻塞了主线程 → macOS 出彩色转圈（应用无响应）。同类命令还有 `lsof` 全机扫描、`ps` 扫描、`networksetup` 改系统代理等。

**修复**：把 13 个慢命令全部改为 `async fn` + `tauri::async_runtime::spawn_blocking`，阻塞 IO 挪到线程池，主线程只负责界面：
`server_metrics`、`ssh_exec`、`ssh_connect`、`fetch_subscription`、`start_proxy`、`stop_proxy`、`set_system_proxy`、`save_config`、`scan_apps`、`proxy_api`、`check_conflicts`、`kill_foreign_proxies`、`list_foreign_proxies`。

**配套改动**：
- `SshManager` 改 `#[derive(Clone)]` + 内部字段全 `Arc<Mutex<..>>`，支持 `ssh_connect` 跨线程共享同一会话
- `effective_app_rules_with` 复用 `apps_cache`，保存配置/启动代理时不再全盘扫描 App
- 前端首屏只 `await refresh()`，`scan_apps` 改后台异步，不再阻塞首屏

**想法变化**：性能问题不能靠"少点几下"绕过——**凡是可能阻塞主线程的 IO，一律 async + spawn_blocking**。这条已作为架构红线写进 `CONTRACT.md`。

### 2026-09-19 · v0.2.4 清掉最后一个主线程阻塞源

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `56e02ee` | fix/perf | `get_status` 也 async 化；`MihomoManager` 改可 Clone |

**根因**：`get_status` 是**同步命令**（跑主线程），被前端**每 5 秒轮询一次**。它内部做两件慢事：
1. `mihomo.status()` → 对混合端口 + 控制 API 各做一次 `TcpStream::connect`（端口被防火墙 DROP 时会阻塞到超时）
2. `system_proxy::status()` → fork 子进程跑 `scutil --proxy`

这解释了用户"点服务器时转一下彩色圈"里除了 SSH 探针之外的**第二个卡顿源**——界面 5 秒一次的周期性微顿。

**修复**：
- `get_status` 改 `async fn` + `spawn_blocking`（返回 `Result<AppStatus, String>`），探测全挪到线程池。
- `MihomoManager` 改 `#[derive(Clone)]` + `pid` 字段改 `Arc<Mutex<Option<u32>>>`：clone 出的是**同一份共享 PID 状态**，实例可 move 进线程池而主线程仍能读到 pid。

**想法变化**：排查卡顿要**顺着"主线程会碰到的所有命令"逐一过筛**，不能只盯用户点的那一个——5 秒轮询的 `get_status` 是隐藏的周期性卡顿源。

### 2026-09-19 · MCP 接口参数健壮化（mcp/server.py）

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `7b12f54` | fix | MCP 工具入参健壮化，防客户端传异常参数时崩溃 |

**背景**：用户要求"确认 MCP 接口有没有阻塞问题"后逐行过筛。**结论：MCP 没有 UI 卡顿风险**（它是独立 Python 进程，与 Tauri 主线程无关）。但发现一批**入参健壮性 bug**：

1. **`"arguments": null` 必崩**：`params.get('arguments', {})` 在 key 存在但值为 `null` 时返回 `None`（不是 `{}`），下游所有 `args.get(...)` 触发 `AttributeError`。→ 在 `tools/call` 入口统一兜底：非 dict 一律当 `{}`。
2. **非字符串入参崩**：`args.get('domain', '').strip()` 若客户端传数字（`{"domain":123}`）→ `int` 没有 `.strip()` → 崩。涉及 `domain`/`url`/`name`/`id`/`mode`/`reason`/`vendor`。→ 新增 `_str_arg()` 辅助函数统一转换。
3. **`int()` 非法值崩**：`list_connections` 的 `limit`、`probe_route` 的 `timeout`/`read_bytes`、`ssh_exec` 的 `timeout_secs` 直接 `int()`，传字符串/null 会崩。→ 全部 try/except 兜底 + 夹到合理范围（limit 1~500、timeout 1~120）。

**为什么之前没暴露**：外层 `tools/call` 有 `try/except` 兜底，所以只会返回 error 不会崩服务——但智能体每次都拿到一个"未知错误"，体验差且难排查。

**想法变化**：MCP 是"给不可控客户端调的接口"，**入参必须当作脏数据**——不能假设客户端会传正确类型。这条适用于所有工具入口。

---

### 2026-09-19 · 完整回归测试（发现并修复 2 个真实 bug）

| 提交 | 类型 | 做了什么 |
|------|------|----------|
| `a7f457a` | fix | MCP status/systemProxy 字段报假状态 + start/stop_proxy 系统代理联动缺失 |

**背景**：用户要求"跑一次完整回归测试"，覆盖 启动→开代理→连通性→仪表盘→改配置→停代理→稳定性 全链路。

**发现并修复的 2 个真 bug（都在 MCP 侧，与 Rust 侧行为不一致）**：

1. **`status.systemProxy` 报假状态**：读的是 `config.json` 的 `systemProxy` 字段（"上次意图"），不是真实系统代理状态。App 重启后 config 仍是 true 但实际已关 → 误导调用方。
   → 新增 `system_proxy_enabled()`（读 `scutil --proxy`，与 Rust 侧 `system_proxy.rs::status` 一致），`status` 改用真实状态。
2. **`start_proxy`/`stop_proxy` 不联动系统代理**：
   - `start_proxy` 检测到"内核已在运行"就直接返回，**不管系统代理是否开着** → 用户点了启动，代理却不生效。
   - `stop_proxy` 只停内核，**不关系统代理** → 停了代理后系统代理仍指向已关闭的 7891，导致断网。
   → 新增 `set_system_proxy()`（MCP 侧原本完全没有此能力），start 时补开、stop 时关闭，与 Rust 同侧逻辑对齐。

**附带修正**：`mihomo_running()` 改为"进程存在 **且** 控制 API 可响应"（原仅查进程，会把孤儿内核误判为在运行）。

**回归结果（8 项全过）**：
| 项 | 结果 |
|----|------|
| T1 冷启动 | ✅ App PID 44755 健康，无第三方代理残留 |
| T2 启动代理 | ✅ 端口/系统代理/节点全就绪 |
| T3 连通性 | ✅ YouTube 200(2.8s)/Google 200/百度 200(0.7s)/GitHub 200 |
| T4 仪表盘 | ✅ SSH 期间 App 响应采样 38 次零卡死（**核心回归点通过**） |
| T5 改配置 | ✅ 域名规则 32→33→32，热重载正常无残留 |
| T6 停代理 | ✅ 端口关闭/系统代理关闭/内核退出，可反复启停 |
| T7 稳定性 | ✅ 连续启停+节点切换无泄漏，status 三字段与实际完全一致 |
| doctor | ✅ 配置/鉴权/failover/规则顺序/节点健康全绿 |

**验证**：`cargo check` 零错误、`cargo test --release` **29 测试全绿**、`py_compile` 通过。

**想法变化**：**MCP 与 Rust 是同一个软件的"两个入口"，行为必须一致**——同一个功能（如停代理关系统代理）两边的副作用必须对齐，否则一个入口正常、另一个入口留坑。这条应加入 CONTRACT 红线。

---

### 2026-09-23 · 安全修复 + P0 信任版 + 定位改版「网络管理」（v0.2.11 已构建）

| 类型 | 做了什么 |
|------|----------|
| fix(安全) | **listeners 对局域网裸奔**：mihomo `listeners` 段不受 `allow-lan:false` 约束、`bind-address` 字段实测无效，7892/7893 一直绑 0.0.0.0 → 同网段可匿名白嫖节点出口。双引擎（Rust build_conf + MCP generate_config）每条 listener 显式加 `listen: 127.0.0.1`，Rust 单测断言锁死，check_parity 验证一致。 |
| fix(隐私) | **root 日志世界可读**：提权启动 umask 020 使 mihomo.log 落成 0644（全机连接记录含进程名→域名），osascript 路径与 mihomo-ctl.sh 均改 `umask 077` + 存量日志/轮转 .old `chmod 600`。 |
| fix(对账) | **P0-1 系统代理逐服务对账**：`set_system_proxy` 的 `any_success`（任一服务成功即整体成功）替换为逐服务 `-get*state` 读回，返回 `{enabled, services[], allOk, mismatched[]}`；Rust 13 个调用点全部接入 `log_proxy_set_result` 日志，UI 部分失败黄色点名，MCP 同契约（`set_system_proxy` 返回对账、`status` 加 `verify` 可选参数）。附带揪出 `parse_port` 潜伏 bug：scutil 的 `HTTPPort : 7891` 带前导空格，旧解析恒失败、端口字段永远为 0。测试：Rust 4 项 + Python mock 4 项。 |
| fix(自愈) | **P0-4 崩溃自愈**：App 被强杀/断电后"系统代理指向本程序死端口（7891/7892/7893）"会在下次启动被 `startup_self_check` 检测→自动关闭恢复直连→toast 一次性告知（AppStatus.selfHealNotice 读取即清空）；指向第三方端口的系统代理绝不误关。 |
| docs | **全面重写**：定位从"魔法代理客户端"升级为"网络管理员·一键归序"；设计文档新增废弃旧思想对照表与验收实验章节；新增 `docs/重构计划.md`（P0 修信任 / P1 归序 MVP / P2 收敛更名）。 |
| 更正 | ① "SSH 隧道/端口转发"系 2026-09-23 排查过程中子代理误报——经 git 全量核实，历史文档从无此声称，本项目也从未实现过该功能（全源码无 `-L`/tunnel），现于各文档明示 SSH 能力边界；② MCP 工具数 README 长期写 23，实际 TOOLS 数组为 **27**（code-wiki/04 早已指出，本次统一修正所有引用处）；③ "启动清理第三方 = 接管秩序"的旧立场废弃，接管必须配审计+账本+回滚。 |
| 根因认知 | "问题是不是因为 mihomo 用的别人的"——**不是**。内核行为完全符合其文档；缺陷全部出在我们对配置语义的理解（缺 listen 字段）与工程习惯（any_success 报假账、umask 想当然）。用成熟内核恰恰是正确决策，需要的是把它的配置契约当回事。 |
| 构建 | **v0.2.11 已发版**（`release.sh 0.2.11`，未 --publish）：产物 `尊者魔法代理.app`/`.dmg`/`.app.tar.gz` + Ed25519 签名 + 双通道 feed（本地 7878 已更新、GitHub 通道待 `--publish`）。发版中揪出 release 链断裂：tauri-cli 2.11.4 起 beforeBuildCommand 工作目录改为项目根，旧配置 `cd .. && pnpm --dir ui build` 会跑到磁盘根导致构建失败，已改为目录探测兼容写法。|

**下一步**：安装 v0.2.11 走验收场景 D 人工回归（kill -9 自愈）→ P1-1 审计引擎（auditor.rs）。

### 2026-09-23（同日续）· MCP 27 工具全量实机测试 + v0.2.11 热修复（工作区）

| 类型 | 做了什么 |
|------|----------|
| 测试 | 以真实 MCP 客户端身份（stdio JSON-RPC）**跑完全部 27 个工具**：15 只读全过；写操作（规则/应用模式/节点切换/系统代理/启停/SSH 只读命令）逐项验证副作用真实发生；负例（SSRF×3、未知服务器、脏参数）行为符合预期；测试全程留基线快照，结束后 diff 确认配置零差异、现场完全恢复。 |
| fix(热修) | **揪出 v0.2.11 自埋的雷**：`-getwebproxystate` 等读命令真机【不存在】（rc=5，读写命令集不对称），上一轮对账的读回恒失败、恒误报 mismatched；纯 mock 测试全绿但真实链路全错。修复：两侧改用真机实测的 `-getwebproxy` 三件套（`Enabled: Yes/Server/Port` 行式），达标口径升级为"开关 + 端口精确匹配"（"开着指向 7890"这类残留同样点名）；Python 测试 fixture 全部换成真机输出，并加源码回归锁（verify 函数体出现 `-get*state` 即失败）。验证：真机 `status{verify:true}` 从误报 allOk:false 修正为如实 allOk:true。Rust 48 测试 + Python 14 测试全绿。**此修复未入 v0.2.11 包，发版前需 rebuild 覆盖。** |
| 登记 | CONTRACT 新增 5 条实测缺陷（Enabled 字段语义不可靠 / MCP stop 被看门狗复活 / 域名无校验 / probe_route 无参数校验 / SSH 缺凭钥慢超时），修法均已写明。 |

**教训固化**：mock 测试只能证明"代码按我以为的格式解析"，证明不了"我以为的格式是真的"。凡对接系统命令的解析，fixture 必须先取自真机输出。

---

### 2026-09-23（换账号后新会话接手）· 进度核查交接记录

> 新会话无上一轮对话记忆，依用户"开工前先核查、收工后必落账"的要求，本节记录本会话对现场的一次性核实结论，作为下个会话的交接基线。

**核实手段**：`git status` / `git log` / 文件 mtime / 已装 App 二进制字符串取证 / `cargo test` / `pytest` / `check_parity.py`。

**结论（已验证）**：
- **代码基线全绿**：Rust `cargo test` 48 项全过；`tests/test_regressions.py` 14 项全过；`check_parity.py` 退出码 0（双引擎一致）。
- **v0.2.11 热修复已入已装包**：`/Applications/尊者魔法代理.app`（14:14 安装）二进制取证 `getwebproxy`/`getsecurewebproxy` 各命中、`getwebproxystate`/`getsecurewebproxystate`/`getsocksfirewallproxystate` 全 0，且 `mismatched`/`allOk` 命中——即已装包用的是热修后的 `-get*proxy` 三件套读法，非旧的 `-get*state`。上一节"此修复未入 v0.2.11 包，发版前需 rebuild 覆盖"的告警，经时间线核对（源文件 mtime 13:47/13:50 均早于 14:14 安装、且无源文件晚于该安装）**已解除**：14:14 的安装发生在热修之后，包是新的。bundle 目录 14:19 另有一次构建产物。
- **唯一明确缺口 = 未提交**：工作区 `git status` 有 **82 个已改文件 + 多份新增未跟踪文件**（`docs/code-wiki/`、`docs/重构计划.md`、`docs/免费模型清单.md`、`tests/`、`ui/src/updater.js`、图标母版等），覆盖 v0.2.11 信任版 + MCP 27 工具实机测试 + 热修复 + 文档重写全部成果，**尚未 `git commit`**。按 `CONTRACT.md` 发版铁律，闭环最后一步"git commit"未做，即 HISTORY 自己点过的老毛病"改完不提交 = 源码与 App 脱节"。
- **已安装版本 = 0.2.11**，与 `tauri.conf.json` 一致；App 当前未运行（`pgrep` 无进程）。

**下一步候选（待用户定向，未擅自开工）**：
1. 把工作区 backlog 落一次 `git commit`（补齐发版闭环的最后一步）。
2. 开 P1-1 审计引擎 `auditor.rs`（`docs/重构计划.md` 明列的"下一步"，P1 MVP 首个新模块）。
3. 修 `CONTRACT.md`「已知待修缺陷」里登记的 5 项（Enabled 语义 / MCP stop 被看门狗复活 / add_domain_rule 无校验 / probe_route 无校验 / SSH 缺凭据慢超时）。

**本会话未改动任何代码/配置，仅新增本节交接记录。**

---

### 2026-09-23（同会话续）· 积压入库 + P1-1 审计引擎落地

| 类型 | 做了什么 |
|------|----------|
| chore | **v0.2.11 信任版全量入库**（`git add -A && commit`，103 文件 / +5451 行）：P0 四项 + 热修复 + MCP 实机测试 + 文档重写全部落库，补齐发版闭环缺的"git commit"一步。提交号见 `git log` 顶部。 |
| feat(P1-1) | **审计引擎 `src-tauri/src/auditor.rs`（新模块，652 行）**：`audit()` 只读采集 8 个维度——第三方代理进程（与 lib.rs::find_foreign_proxies 同源，杜绝两套口径）、LISTEN 扫描（lsof 2.5s 硬超时）、本程序端口被占、崩溃残留死端口（P0-4 同口径、只报告不自愈）、默认路由、utun 清单、DNS resolver #1、PAC、环境变量/shell profile 代理提示。三态 `Triage`（unknown 与 false 严格区分）+ `degraded` 字段诚实暴露采集缺口。Rust 15 项单测（真机 fixture、超时路径、坏行跳过）。 |
| feat | **接线**：`lib.rs` 新增 `audit_network` 命令（async + spawn_blocking，已注册；`MihomoManager::find_running_pid` 提为 pub(crate)）；**MCP 第 28 个工具 `audit_network`**（Python 侧同口径实现，含"命令行含 clash 但可执行路径不是 → 绝不误报"铁律回归）；**Dashboard「网络体检」卡**（手动触发、混乱源列表、降级点名）。 |
| fix(口径) | **回归测试抓出的判定错误**：内核是否在跑不能用"own 端口有 LISTEN"判定——第三方占用 7891 时内核根本没跑，会把真·死端口残留漏报。两侧同步改为按内核进程判定（pgrep runtime 路径），各加回归锁。 |
| docs | 重构计划 P1-1 标 ✅（含实现落账细节）；文档 MCP 工具数 27→28（6 处）；CONTRACT 增"体检 kernel_up 口径"红线。 |
| 验证 | Rust 63 测试全绿（48→63）、Python 18 全绿（14→18）、check_parity 退出码 0、前端 vite build 通过、**真机实测**：MCP audit_network 实跑返回与手工取证一致（route en0/网关 172.18.100.1、DNS 223.5.5.5、PAC no、无混乱源、零降级）。 |

**未做（有意为之）**：计划里"并入 doctor 的展开项"——doctor 是代理自检、audit 是整机审计，两个独立工具更清晰，doctor 描述已引导互用。

**下一步**：P1-2 接管账本 `ledger.rs`（每次 start_proxy 前快照原值，stop/还原时逆序回放）。

---

### 2026-09-23（同会话续 2）· P1-2 接管账本落地

| 类型 | 做了什么 |
|------|----------|
| feat(P1-2) | **接管账本 `src-tauri/src/ledger.rs`（新模块，~380 行）**：接管"先记账后动手"的凭证。`begin_takeover`（动手前逐服务快照**完整原值** enabled/server/port，是还原依据而非达标判定）→ `record_killed_procs`（被杀第三方进程入账，**不可逆项如实 reversible=false**）→ `settle_open`（正常结束结账）。二次接管幂等——before 永远是【最初】原值不被接管后状态覆盖。损坏→保全 .corrupt 证据（已有不覆盖，时间戳另存）后从空继续；原子写 tmp→chmod 600→rename（config.rs 同序）。Rust 8 项单测含**跨引擎 schema 契约锁**。 |
| feat | **`system_proxy.rs` 新增快照/回放原语**：`snapshot_system_proxy()`（读 -getwebproxy 三件套完整值）与 `restore_service_snapshot()`（按快照回写单服务，快照不完整的通道宁可不还原、如实报告）——P1-4 一键还原直接消费此接口。 |
| feat | **接管入口与结账接线**：`takeover_cleanup`=显式接管包装（begin→cleanup→入账），仅 start_proxy（用户点启动）与 kill_foreign_proxies（UI 一键清理）走；**App 启动 800ms 自动清理不记账**（CONTRACT：启动清理 ≠ 接管）。节点缺失前置检查挪到接管之前，mihomo.start 失败即结账——不留幽灵账本。stop_proxy / RunEvent::Exit 结账（**只结账不回滚**，回滚属 P1-4）。启动时检测到未结账本 → selfHealNotice 一次性提示（含接管原因/在册条目/不可逆项数）。 |
| feat | **MCP 侧同账本**（`mcp/server.py` ledger 段）：与 Rust 共写同一 `ledger.json`（schema 逐键 camelCase 一致，两侧各有契约锁）；start_proxy 未运行路径先记账（失败结账）、stop_proxy 结账；`audit_network` 新增 `openLedger` 字段并计入 summary（Rust 体检同步加，双引擎同口径）。Python +3 回归（完整周期/损坏保全/schema 契约锁含 0600 权限断言）。 |
| 过程修正 | ① cleanup 一度被设计成"任何清理都记账"，落码时改为显式接管入口才记——启动自动清理不是接管，记了会造成"永远有未结账本"的假象；② 体检报告的 build_summary 增 open_ledger 参数后忘同步测试调用，编译挡下已修；③ 契约测试误断言未结账本含 settledTs（两侧均 skip_serializing_if none，语义一致），测试改先 settle 再断言。 |
| 验证 | Rust 71 全绿（63→71）、Python 21 全绿（18→21）、parity 0、前端构建过。**真机端到端**：begin 快照真实原值（Wi-Fi http enabled=false/127.0.0.1:7891）→ 不可逆入账 → settle 幂等 → 0600 权限 → 体检报告含 openLedger → 测试账本删除零残留。 |

**下一步**：P1-3 归序流程编排（start_proxy 接 audit：快速/确认双模式）→ P1-4 一键还原 `restore_network` + 看门狗漂移巡检。

---

### 2026-09-23（交接会话）· P1-3 归序流程编排落地

> 交接说明：上一会话额度耗尽时 P1-3 代码已全部写完但停在未提交状态（8 文件 +422 行）。
> 本会话接手：复核全部改动 → 四道自检重跑全绿 → 落账提交。

| 类型 | 做了什么 |
|------|----------|
| feat(P1-3) | **回滚原语 `ledger.rs::rollback_session`**：把当前 open 会话的 system_proxy 条目按 before 逐服务回放原值后结账。process 项不可逆绝不进回放（如实承认"回不来"）；before 解析失败的条目跳过并点名（凭 unknown 瞎写比重置失败更危险）；回放有失败也结账——接管已结束，失败项由调用方如实上报。可注入回放函数版本供单测 mock，绝不真动本机系统代理。`system_proxy.rs` 的 `ProxyChannelRaw`/`ServiceSnapshot` 补 Deserialize（回放需从账本 JSON 反序列化）。Rust +4 单测（回放+结账/跳过 process/容忍损坏快照/无账如实 false）。 |
| feat | **`start_proxy` 编排升级（Rust + MCP 同语义）**：系统代理设置后逐服务对账不达标 = 半套秩序 → 宁可不启：停内核 + 按账本回放系统代理原值 + 结账 + 报错。错误信息按账本实况分层如实声明——有账可回/无账可回、回放失败项逐条点名、账本含不可逆 process 项才提示"第三方进程不会自动复活"（绝不空喊吓用户，也绝不隐瞒）。MCP 侧 `ledger_rollback` + `start_proxy` 失败回滚同步实现（CONTRACT 双入口一致性红线）。Python +4 回归（回滚回放原值/写失败点名/无账 false/端到端 ok:False+rolledBack）。 |
| feat | **确认模式（UI 侧编排入口）**：config 新增 `confirmTakeover`（默认 false=快速模式沿用现行为）。开启后点「启动代理」先调 `takeover_plan`（新 Tauri 命令：只读归序前体检，复用 auditor::audit，传真实内核 PID 防 kernel_up 误报），有混乱源弹原生对话框逐项列示"将关闭谁/将清理什么"+体检结论，用户确认才接管；体检失败不拦启动但 toast 点名"没体检成"。设置页新增「接管方式」面板开关（经 saveConfig 通道持久化）。 |
| 验证 | 接手复跑：Rust 75 全绿（71→75）、Python 25 全绿（21→25）、parity 0、vite build 过、dialog 插件依赖与 capabilities 权限核对齐。真机状态核查：本机无 ledger.json（从未发生真实接管），无异常残留。 |

**未做（有意为之）**：确认模式的列示放在 UI（start_proxy 内部不阻塞等确认），快速模式清理逻辑保持原样（防误杀铁律在 CONTRACT）；route/dns 层级混乱源目前仅列示不处置（P1-4 之后视需求扩展）。

**下一步**：P1-4 一键还原 `restore_network`（消费 rollback_session/restore_service_snapshot）+ 看门狗漂移巡检（每 5 分钟抽验账本关键项现值）。

---

## 各版本「增 / 删」总表

| 版本 | 新增 | 删除 |
|------|------|------|
| v0.0.1 | Tauri 工程骨架、mihomo 管理、进程分流、SSH 控制台 | — |
| v0.0.2 | 订阅拉取、Keychain、节点管理、冲突检测 | 废弃的 Electron 版本 |
| v0.0.3 | TUN 模式、特权 mihomo、`mihomo -t` 校验 | nohup 启动方式 |
| v0.0.4 | MCP 服务（AI 控制） | — |
| v0.0.5 | 域名分流规则、PATCH/SIGHUP 热重载 | auto 模式选项 |
| v0.1.0 | 加密 DNS、按软件选节点、住宅节点 | — |
| v0.2.0 | 云服务器管理、双路探测、新图标/更名 | — |
| v0.1.1 | 退出清理钩子、提权兜底 | 孤儿内核隐患 |
| v0.2.2 | 第三方代理清理、卡顿修复、release.sh 流程、看门狗 | 老的"拒绝启动"逻辑、手工 cp 流程 |
| v0.2.3 | 13 个慢命令 async 化、SshManager 可 Clone、apps_cache 复用 | 同步命令阻塞主线程的写法 |
| v0.2.4 | get_status async 化、MihomoManager 可 Clone | 5 秒轮询里残留的同步阻塞 |
| 文档整理 | docs/设计.md、docs/免费模型.md | 3 份旧产品文档、2 份旧模型文档、过期计划 |
| 2026-09-23 改版 | listener 回环钉死、日志 0600、重构计划.md、定位「网络管理」 | "allow-lan 管得住 listeners"错觉、"SSH 隧道"排查误报（已澄清，项目从无此功能）、"清理=接管"旧立场、README"23 工具"错数 |

---

## 怎么看历史

```bash
git log --oneline            # 全部提交（一行一条）
git log --oneline --graph    # 带分支图
git tag -l -n1               # 所有版本标记
git show v0.2.2              # 某版本改了什么
git diff v0.2.0 v0.2.2       # 两个版本间的差异
git checkout v0.1.0          # 回到某个历史版本（只读查看）
```

## 备份说明

**git 本身就是备份。** 所有历史版本都在 `.git/` 里，可随时 `git checkout <tag>` 查看。
**不另建 `backup/` 目录**放 .app 包——那是重复且占空间（v0.1.0 旧包已清）。
发版产物（`.app.tar.gz`、dmg）由 `release.sh` 按需生成，不入库（已 gitignore）。
