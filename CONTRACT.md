# CONTRACT.md — 魔法代理（尊者魔法代理）项目宪法

> 代码合并顺序：改代码 → 更新本文件。本文件与其他文档冲突时，本文件优先。

## 冻结的正确状态（2026-09-03 起）

- 架构：Tauri 2 + mihomo 内核。组合根 = `src-tauri/src/lib.rs::run()`；内核管理 = `mihomo.rs::MihomoManager`（深模块，接口只增不删）。
- 分流设计（README 钉死）：系统代理 + 进程级分流；TUN 仅按规则拉「该走代理」的流量（auto-route: false, strict-route: true）；两条死锁端口 7893(无条件 PROXY)/7892(无条件 DIRECT)。
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

## 回滚

- 代码：`git checkout a4106c4 -- <file>` 或整体 revert v0.1.1 提交。
- 应用：旧版备份于 `/Applications/魔法代理.app.bak-0.1.0`（替换前留）。
