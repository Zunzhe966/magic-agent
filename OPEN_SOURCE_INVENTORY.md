# 开源资产清单（开源 / 私密 边界说明）

> 本文回答三个问题：**这个仓库里到底有多少东西？哪些是开源的？哪些是私密的、没有开源的？**
>
> 盘点基准：`git ls-files`（真正被 Git 跟踪、会随仓库公开的文件），
> 时间：2026-09-20，版本：v0.2.5

---

## 一、结论速览

| 类别 | 数量 | 是否公开 |
|---|---|---|
| **被 Git 跟踪的文件** | **111 个** | ✅ 全部公开在 GitHub |
| **主动排除、仅在本地存在的资产** | **4 类** | ❌ 不开源（含私密信息） |
| Git 提交历史 | 52 个提交 | ✅ 公开（已脱敏） |
| GitHub Release 资产 | 3 个（v0.2.5） | ✅ 公开 |

**一句话**：仓库里只有「程序代码 + 图标 + 第三方运行库」，
**所有跟服务器、账号、密码、个人节点有关的东西都在仓库之外**。

---

## 二、公开在 GitHub 的东西（108 个文件）

### 2.1 按目录分布

| 目录 | 文件数 | 内容 |
|---|---|---|
| `src-tauri/` | 77 | Rust 后端 + 图标资源 + 第三方运行库 |
| `ui/` | 17 | Vue3 前端 |
| `scripts/` | 6 | 发版与运维脚本 |
| 根目录 | 6 | README / LICENSE / 合规声明等 |
| `docs/` | 3 | 设计文档、免费模型清单 |
| `mcp/` | 2 | MCP 接口说明 |

### 2.2 按文件类型

| 类型 | 数量 | 说明 |
|---|---|---|
| `.png` | 48 | 应用图标（Android / iOS / Windows / macOS 全套） |
| `.rs` | 11 | Rust 源码 |
| `.vue` | 9 | 前端页面组件 |
| `.json` | 8 | 配置与能力声明 |
| `.md` | 5 | 文档 |
| `.sh` | 3 | 脚本 |
| `.py` | 3 | 辅助脚本 |
| `.js` | 3 | 前端脚本 |
| `.yaml` | 2 | 配置 |
| `.dat` / `.mmdb` / `.metadb` | 4 | GeoIP / GeoSite 数据 |
| `mihomo`（无扩展名） | 1 | 代理内核二进制 |
| 其他（toml/plist/xml/icns/ico） | 5 | 配置与图标 |

### 2.3 具体模块清单

**Rust 后端（`src-tauri/src/`，11 个 .rs）**

| 文件 | 作用 |
|---|---|
| `lib.rs` | 主入口：注册所有命令、启动 mihomo、系统代理 |
| `updater.rs` | **（本次新增）** 双通道更新：运行时切换更新端点 |
| `config.rs` | 配置读写（`config.json`），含更新通道字段 |
| `mihomo.rs` | mihomo 进程管理与 REST API 调用 |
| `system_proxy.rs` | macOS 系统代理设置 |
| `ssh.rs` | 云服务器 SSH 控制台 |
| `keychain.rs` | 系统钥匙串读写 |
| `apps.rs` | 按应用分流规则 |
| `main.rs` | 二进制入口 |
| `bin/dump_conf.rs` | 调试工具：导出配置 |
| `bin/magic_probe.rs` | 调试工具：连通性探测 |

**Vue3 前端（`ui/src/`，9 个 .vue）**

| 文件 | 作用 |
|---|---|
| `App.vue` | 主界面 + 启动静默更新检查 |
| `components/SettingsView.vue` | **（本次改动）** 设置页 + 更新通道切换 UI |
| `components/Dashboard.vue` | 总览面板 |
| `components/ServersView.vue` | 服务器列表 |
| `components/ServerDashboard.vue` | 服务器监控面板 |
| `components/SshView.vue` | SSH 控制台 |
| `components/AppsView.vue` | 应用分流 |
| `components/DomainRulesView.vue` | 域名规则 |
| `components/ConnectionsView.vue` | 连接列表 |

**第三方运行库（`src-tauri/resources/`，不入代码，但是二进制）**

| 文件 | 大小 | 上游 | 许可证 |
|---|---|---|---|
| `bin/mihomo` | 42 MB | MetaCubeX/mihomo | **GPL-3.0**（含附加命名条款） |
| `geo/geoip.dat` | 20 MB | MetaCubeX/meta-rules-dat | GPL-3.0 |
| `geo/geoip.metadb` | 10 MB | 同上 | GPL-3.0 |
| `geo/ASN.mmdb` | 10 MB | MaxMind GeoLite2 | **专有许可（非开源）** |
| `geo/geosite.dat` | 4.2 MB | 同上 | GPL-3.0 |

> ⚠️ 注意：`ASN.mmdb` 来自 MaxMind GeoLite2，是**专有数据库**，
> 不属于开源许可。详见 `THIRD_PARTY.md`。

---

## 三、不开源 / 私密的东西（本地存在，永不推送）

这些文件**要么在 `.gitignore` 里，要么存在于仓库目录之外**，
GitHub 上完全看不到：

| # | 资产 | 位置 | 为什么必须私密 |
|---|---|---|---|
| 1 | **更新签名私钥** | `~/.tauri/magic-agent.key` | 泄漏后任何人都能伪造「官方更新包」，直接变成供应链攻击。这是全项目最敏感的文件。 |
| 2 | **发版 feed 目录** | `scripts/updater-feed/`<br>`scripts/updater-feed-publish/` | 含 43 MB 构建产物，且 `latest.json` 是发布物，不该进源码仓库（已在 `.gitignore`） |
| 3 | **AI 助手工作目录** | `.workbuddy/` | 开发过程记录，可能含临时上下文（已在 `.gitignore`） |
| 4 | **真实服务器与凭据** | `~/Library/Application Support/magic-agent/config.json` | **最关键**：真实节点 IP、UUID、Reality 公钥、shortId、SSH 密码全在这里。这个文件在用户目录，根本不在仓库里。 |

### 3.1 第 4 项展开：用户配置不在仓库里

App 运行时的配置写在 macOS 用户目录：

```
~/Library/Application Support/magic-agent/config.json
```

里面包含（举例字段名，**不列真实值**）：

- `nodes[]` —— 真实节点：`server`、`uuid`、`publicKey`、`shortId`、`sni`
- `servers[]` —— 云服务器列表
- `sshPassword` / `sshPrivateKey` —— SSH 凭据

这个路径**不在任何 Git 仓库内**，因此从设计上就不会被推送。
仓库里也**没有任何硬编码的节点默认值**。

> 历史教训：早期 Electron 遗留文件里曾硬编码过真实 shortId，
> 已用 `git filter-repo` 重写全部 52 个提交的历史并重新建库清除。
> 现在全历史扫描（`git rev-list --all` + `git grep`）**无任何真实凭据命中**。

---

## 四、开源许可证总览

本项目自身：**MIT**（见 `LICENSE`）

引入的第三方组件（详见 `THIRD_PARTY.md`）：

| 组件 | 许可证 | 是否传染 |
|---|---|---|
| mihomo | GPL-3.0 + 命名条款 | ⚠️ 以独立进程调用，不传染本项目代码 |
| MetaCubeX/meta-rules-dat | GPL-3.0 | 数据文件，非链接 |
| MaxMind GeoLite2 | 专有 EULA | 仅限非商业使用 |
| Tauri 全家桶 | Apache-2.0 OR MIT | 否 |
| Vue / Vite / xterm | MIT | 否 |
| serde / dirs | MIT OR Apache-2.0 | 否 |
| Loyalsoldier/geoip | CC-BY-SA-4.0 | 需署名 |
| v2fly/domain-list-community | MIT | 否 |
| REALITY | MPL-2.0 | 否 |

**关键合规动作**：mihomo 是 GPL-3.0，本项目通过**独立进程 + REST API** 调用它
（不是静态链接），因此本项目自身可以保持 MIT。

---

## 五、GitHub 上的发布资产（v0.2.5）

这些是公开的下载物，位于 Release `v0.2.5`：

| 资产 | 大小 | 用途 |
|---|---|---|
| `latest.json` | 704 B | 更新清单，App 检查更新时读取 |
| `magic-agent_0.2.5_aarch64.app.tar.gz` | 43.7 MB | 更新包本体 |
| `magic-agent_0.2.5_aarch64.app.tar.gz.sig` | 420 B | Ed25519 签名 |

> 命名规范：发布资产**必须用 ASCII 名**。
> GitHub 会静默改写中文文件名（曾把「尊者魔法代理.app.tar.gz」变成「app.tar.gz」），
> 导致 `latest.json` 里的下载地址 404。已在 `release.sh` 中固化此规则。

---

## 六、仓库体积说明

| 项目 | 大小 |
|---|---|
| GitHub 报告仓库大小 | 28.7 MB（压缩后） |
| 被跟踪文件实际占用 | 约 94 MB（含 42 MB mihomo + 44 MB geo 数据） |

> 说明：仓库里有约 86 MB 的二进制资源（mihomo 42 MB + geo 数据 44 MB）。
> 这是为了让用户**克隆即可构建**（无需自行下载内核）。
> 代价是仓库偏大，属有意取舍。

---

## 七、如何自行核对

任何人都可以用以下命令验证上述结论：

```bash
# 1. 看真正被跟踪的文件（= 会公开的文件）
git ls-files

# 2. 确认没有真实凭据泄漏（全历史）
git rev-list --all | while read c; do
  git grep -lE "你的真实IP|你的真实UUID" "$c" 2>/dev/null
done

# 3. 确认私钥不在仓库里
git ls-files | grep -E "\.key$" || echo "无私钥"

# 4. 看哪些文件被主动忽略
cat .gitignore

# 5. 看第三方许可证声明
cat THIRD_PARTY.md
```

---

## 八、维护约定

新增文件时请对照：

- ❌ **绝不入库**：私钥、`.env`、真实 IP/UUID/密码、构建产物（`target/`、`dist/`）
- ⚠️ **谨慎入库**：二进制依赖（体积大，需说明来源与许可证）
- ✅ **鼓励入库**：源代码、文档、图标、许可证声明

判断标准：**「这个文件如果被陌生人看到，我会不会不舒服？」**
会 → 不入库。
