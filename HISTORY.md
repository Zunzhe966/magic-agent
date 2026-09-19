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
| `pending` | fix | MCP 工具入参健壮化，防客户端传异常参数时崩溃 |

**背景**：用户要求"确认 MCP 接口有没有阻塞问题"后逐行过筛。**结论：MCP 没有 UI 卡顿风险**（它是独立 Python 进程，与 Tauri 主线程无关）。但发现一批**入参健壮性 bug**：

1. **`"arguments": null` 必崩**：`params.get('arguments', {})` 在 key 存在但值为 `null` 时返回 `None`（不是 `{}`），下游所有 `args.get(...)` 触发 `AttributeError`。→ 在 `tools/call` 入口统一兜底：非 dict 一律当 `{}`。
2. **非字符串入参崩**：`args.get('domain', '').strip()` 若客户端传数字（`{"domain":123}`）→ `int` 没有 `.strip()` → 崩。涉及 `domain`/`url`/`name`/`id`/`mode`/`reason`/`vendor`。→ 新增 `_str_arg()` 辅助函数统一转换。
3. **`int()` 非法值崩**：`list_connections` 的 `limit`、`probe_route` 的 `timeout`/`read_bytes`、`ssh_exec` 的 `timeout_secs` 直接 `int()`，传字符串/null 会崩。→ 全部 try/except 兜底 + 夹到合理范围（limit 1~500、timeout 1~120）。

**为什么之前没暴露**：外层 `tools/call` 有 `try/except` 兜底，所以只会返回 error 不会崩服务——但智能体每次都拿到一个"未知错误"，体验差且难排查。

**想法变化**：MCP 是"给不可控客户端调的接口"，**入参必须当作脏数据**——不能假设客户端会传正确类型。这条适用于所有工具入口。

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
