# CONTRACT.md — 网络管理（现名：尊者魔法代理）项目宪法

> 代码合并顺序：改代码 → 更新本文件。本文件与其他文档冲突时，本文件优先。
> 产品定位与重构路线：`docs/设计.md`（一键归序五步闭环）、`docs/重构计划.md`（P0/P1/P2）。

## 冻结的正确状态（2026-09-03 起；2026-09-23 增补安全红线）

- 架构：Tauri 2 + mihomo 内核。组合根 = `src-tauri/src/lib.rs::run()`；内核管理 = `mihomo.rs::MihomoManager`（深模块，接口只增不删）。
- 分流设计：系统代理 + 进程级分流；TUN 仅按规则拉「该走代理」的流量（auto-route: false, strict-route: true）；两条死锁端口 7893(无条件 PROXY)/7892(无条件 DIRECT)。
- **监听红线（2026-09-23）**：所有 `listeners` 条目必须显式 `listen: 127.0.0.1`。实测证伪两个旧假设：①`allow-lan: false` 管不到 listeners 段；②`bind-address` 字段对 listener 无效——缺 `listen:` 时内核绑 0.0.0.0，局域网可匿名白嫖节点出口。Rust 单测已锁死断言，升级 mihomo 版本后须重跑 docs/设计.md §七 验收实验。
- **日志权限红线（2026-09-23）**：root 启动路径（osascript shell_cmd 与 mihomo-ctl.sh start）必须 `umask 077` 并对存量日志/轮转 .old `chmod 600`。日志含全机连接记录，世界可读 = 同机隐私泄露。
- 回归基线：`cd src-tauri && cargo test` 全绿（含 `generated_conf_is_valid_mihomo_yaml` 用 `mihomo -t` 真校验配置）。

## 发版流程铁律（2026-09-19 起，**这是唯一正确的发布方式**）

**禁止手工 `cp` 覆盖 `/Applications` 来"发版"。** 必须走 `scripts/release.sh`：

```bash
cd "/Volumes/A区/魔法代理" && bash scripts/release.sh <版本号>
```

它做三件事，缺一不可：
1. 同步三处版本号（`tauri.conf.json` / `Cargo.toml` / `ui/package.json`）——必须始终一致。
2. 构建 + 用 `~/.tauri/magic-agent.key` 签名，产出 `尊者魔法代理.app.tar.gz` + `.sig`。
3. 生成 `scripts/updater-feed/latest.json`（updater 更新源）。

App 已内置 updater（`tauri.conf.json` → `plugins.updater.active=true`，endpoint `http://127.0.0.1:7878/latest.json`，`App.vue::checkForUpdateQuiet` 启动时静默检查）。**升级用户走自动更新，不重新下载安装包。**

本地更新源：`cd scripts/updater-feed && python3 -m http.server 7878 --bind 127.0.0.1`。

**改代码后的完整闭环**：改代码 → `cargo test` 全绿 → `release.sh <新版本>` → 部署验证 → `git commit`。三处版本号 + latest.json 版本必须指向同一版本。

## 2026-09-19 变更（v0.2.5 / 回归测试）

- **架构红线：MCP 与 Rust 是同一软件的两个入口，同一功能的行为（含副作用）必须一致。**
  - 原因：回归测试发现 MCP 侧 `start_proxy`/`stop_proxy` 不做系统代理联动，而 Rust 侧做 → 走 MCP 入口停代理后系统代理仍指向已关闭端口，用户断网。
  - 规则：任何改变系统级状态的命令（启停代理、改系统代理），两侧实现必须同步核对。
- **状态查询必须报「真实状态」，不能报「配置意图」。**
  - 反面教材：`status.systemProxy` 曾读 `config.json` 的字段（意图），App 重启后 config 仍 true 但实际已关 → 误导智能体。改为读 `scutil --proxy`（真实值）。
  - 用途：判断"代理是否在服务"用 `mihomo_running()`（进程**且** 控制 API 可响应），不要只看进程存在（孤儿内核会误判）。
- **MCP 工具入参必须当脏数据**：`arguments: null` / 数字 / 字符串混传都不能崩。用 `_str_arg()` 取字符串，`int()` 一律 try/except + 范围夹取。

## 2026-09-19 变更（v0.2.3）

- **架构红线：凡是可能阻塞的 IO，一律 `async fn` + `tauri::async_runtime::spawn_blocking`。**
  - 原因：Tauri 的 `#[tauri::command]` 默认**同步执行在主线程**，任何 SSH 连接、`lsof`/`ps` 扫描、`networksetup`、`curl` 都会冻住整个 UI → macOS 彩色转圈（应用无响应）。
  - 反面教材：点「云服务器仪表盘」触发 `server_metrics`（内部 SSH 最长 20s），界面直接卡死。
- 已 async 化的命令（改回同步 = 重演卡顿）：`get_status`、`server_metrics`、`ssh_exec`、`ssh_connect`、`fetch_subscription`、`start_proxy`、`stop_proxy`、`set_system_proxy`、`save_config`、`scan_apps`、`proxy_api`、`check_conflicts`、`kill_foreign_proxies`、`list_foreign_proxies`。
  - 特别注意 **`get_status`**：它被前端**每 5 秒轮询**，内部会 `TcpStream::connect` 探端口 + `scutil` 子进程；同步化 = 界面周期性微顿。这类「高频轮询命令」尤其不能阻塞主线程。
  - 注意：async 命令**必须返回 `Result`**（Tauri 硬性要求），如 `scan_apps -> Result<Vec<AppEntry>, String>`、`get_status -> Result<AppStatus, String>`。
- `SshManager` 已 `#[derive(Clone)]` + 内部字段全 `Arc<Mutex<..>>`：跨线程共享同一 SSH 会话。取共享实例用 `ssh.inner().clone()`（`State` Deref），不要用 `(*ssh).clone()`。
- `MihomoManager` 已 `#[derive(Clone)]` + `pid: Arc<Mutex<Option<u32>>>`：clone 出的是同一份共享 PID 状态，实例可 move 进阻塞线程池。
- `effective_app_rules_with(config, cached)` 复用 `apps_cache`，保存配置/启动代理时不再全盘扫描 App；`effective_app_rules(config)` 保留为无缓存便捷入口。

## 2026-09-19 变更（v0.2.2）

- **启动即清理第三方代理**：App 启动 800ms 后 + `start_proxy` 时，自动杀第三方代理进程（FlClash/Clash Verge/ClashX/V2Ray/Xray/Surge/sing-box 等）并关闭系统代理，让系统回到干净状态后再启动本程序代理。实现：`lib.rs::cleanup_foreign_proxies/find_foreign_proxies`，前端 `Dashboard.vue` 面板 + `kill_foreign_proxies/list_foreign_proxies` 命令。
  - **铁律**：只匹配可执行文件路径，**绝不匹配整个命令行**（否则 grep/编辑器里含 "clash" 字样会被误杀）；绝不匹配宽泛的 "proxy"/"mihomo"；跳过自身进程及 runtime 目录下自己的内核。
- **`start_proxy` 不再因端口冲突拒绝启动**，改为先清理再启动。
- **卡顿修复**：`apps.rs::scan_network_connections` 的 `lsof` 加 2.5s 硬超时；`App.vue::onMounted` 的 `refreshApps()` 改后台异步（不 await），首屏只拉轻量状态+配置。
- **MCP server**：`_active_server()` 增加从选中代理节点推导 SSH 主机（与 Rust 端 `config.rs::active_server` 对齐）；`ssh_exec` 密码分支 expect 脚本结尾加 `set rc [lindex [wait] 3]; exit $rc` 透传远端退出码。

## 2026-09-03 事故与修复（v0.1.1）

事故：App 关闭后 root mihomo 成为孤儿进程，以 TUN + 8 条切分路由接管全机流量，DIRECT 出站一天 1500+ 次 dial i/o timeout，WorkBuddy 等应用流量被劫持（ECONNRESET/502）。当日已手动处置：杀内核、清路由、MCP 自启项改名 .disabled。

三个根因与修复：

1. **App 无退出清理钩子** → `lib.rs::run()` 改为 `build()+run(callback)`，`RunEvent::Exit` 时调 `mihomo.stop()` + 关系统代理。铁律：**App 生命周期必须完整覆盖内核生命周期，关 App = 关代理**。
2. **stop() 杀不掉 root 进程时静默放弃** → 普通 kill 10 秒后仍存活（= root 内核），用 osascript 提权 kill 兜底，绝不留孤儿。
3. **auto-detect-interface 抓错接口致 DIRECT 超时** → build_conf 启动时探测默认路由接口（`/sbin/route -n get default`），钉死顶层 `interface-name`，探测失败回退 en0。
4. **MCP 守护 launchd KeepAlive 常驻** → 改为 KeepAlive=false（登录时启动、崩溃不自动复活）。

## 禁区

- 不得把 `tun.auto-route` 改回 true（会接管系统默认路由，重演劫持事故）。
- 不得删除/绕过 `PROTECTED_DIRECT_DOMAINS` 保命直连名单及其测试。
- 不得删除 `RunEvent::Exit` 收尾钩子。
- mihomo 以 root 运行是 TUN 的硬约束；一切「App 退出后仍需内核活着」的需求必须走显式的后台服务（launchd），不许靠孤儿进程。
- 组名/规则引用必须同源 `sanitize_node_name`（有回归测试钉死）。
- 图标唯一母版必须是 `src-tauri/icons/source-icon.png`，全部尺寸只能通过 `scripts/make-icons.sh` 生成；禁止手工替换单个尺寸后直接发版。
- **不得给 listeners 缺省 `listen:` 字段或用 `bind-address` 替代**（见上方监听红线；违反 = 局域网裸奔）。
- **文档不得声称本项目有 SSH 端口转发/隧道功能**（全源码无 `-L`/tunnel；2026-09-23 曾出现排查误报，已澄清并作为能力边界明示，防止未来误加此表述）。
- **归序功能（P1）落地前，UI/文档不得宣称"一键还原/可回滚"**——清理第三方 ≠ 接管，报状态要如实。

## 已知待修缺陷（修复后移入 HISTORY）

- **`-getwebproxy` 的 `Enabled:` 字段语义在真机不可靠**（2026-09-23 MCP 实机实测）：
  `-set*state off` 后立刻读回显示 `Enabled: No`，但同一台机器另一时点出现
  scutil 全局 `HTTPEnable: 0` 而服务级仍回 `Enabled: Yes` 的矛盾态；
  `-get*state` 系列读命令真机又不存在（读写命令集不对称）。
  后果：`set_system_proxy(false)` 的对账可能虚报未达标（漏报方向安全但违反
  "状态如实"红线）。修法：**达标口径改为"scutil 全局视图为准 + 服务级仅核对
  Server/Port 精确匹配"**，不再从服务级 Enabled 字段推断开关态；需真机多状态
  取证（开/关/半关）后再定判定表。
- **MCP stop_proxy 会被 App 看门狗 30 秒内复活**（同轮实测）：App 在跑且
  should_run=true 时，MCP `stop_proxy` 杀掉内核后，App 看门狗按设计自动拉起
  （PID 变化 34747→40591 证实）。单看是"预期行为"，但从 AI 入口视角
  "我停了它又活了"= 失控。这是双引擎共享状态缺仲裁的实例，P2-1 收敛时一并解决；
  过渡修法：MCP stop 同时把 should_run 落盘（config 增 userStopped 标记），App 看门狗读取。
- **MCP add_domain_rule 域名入参无校验**（同轮实测）：`localhost` 与含换行的
  `evil.com\nallow-lan: true` 都返回"已保存"原样入库。配置生成器两侧都会把
  非法域名静默丢弃（dump_conf/generate_config 双验证未发生注入），但"保存成功
  却永不生效"就是静默失效。修法：add 时即校验拒绝、明确报错。
- **MCP probe_route 无参数校验**（同轮实测）：url=`file:///etc/passwd` 被拼成
  `https://file:///etc/passwd` 照常发起探测；url 传数字 12345 被拼成
  `https://12345`。无注入风险但结果全是误导。修法：scheme 白名单 + 类型校验。
- **SSH auth:key 缺私钥时退化为 10 秒 TCP 超时**（同轮实测）：Keychain 无对应
  私钥时未快速失败提示"缺凭据"，而是等 connect 超时报网络错误，误导排查方向。

## 状态如实红线（P0-1 修复后新增）

- 系统代理设置必须走逐服务读回对账（`verify_system_proxy`），返回值如实携带
  `allOk/services/mismatched`；**禁止重新引入 any_success 类"任一成功即整体成功"逻辑**。
- `set_system_proxy` 返回值里 `enabled` 字段在写后路径取期望值（权威是 services），
  轮询路径（`status()`）取 scutil 全局视图——两者语义已在函数 doc 注释钉死，不得混用。

## 回滚

- 代码：`git checkout a4106c4 -- <file>` 或整体 revert v0.1.1 提交。
- 应用：旧版备份于 `/Applications/魔法代理.app.bak-0.1.0`（替换前留）。
