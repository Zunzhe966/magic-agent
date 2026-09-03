# CONTRACT.md — 魔法代理（尊者魔法代理）项目宪法

> 代码合并顺序：改代码 → 更新本文件。本文件与其他文档冲突时，本文件优先。

## 冻结的正确状态（2026-09-03 起）

- 架构：Tauri 2 + mihomo 内核。组合根 = `src-tauri/src/lib.rs::run()`；内核管理 = `mihomo.rs::MihomoManager`（深模块，接口只增不删）。
- 分流设计（README 钉死）：系统代理 + 进程级分流；TUN 仅按规则拉「该走代理」的流量（auto-route: false, strict-route: true）；两条死锁端口 7893(无条件 PROXY)/7892(无条件 DIRECT)。
- 回归基线：`cd src-tauri && cargo test` 全绿（含 `generated_conf_is_valid_mihomo_yaml` 用 `mihomo -t` 真校验配置）。

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
