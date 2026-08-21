# Magic Agent (魔法代理)

macOS 桌面软件：**按软件分流** + **云服务器 SSH 控制台**。

- 技术栈：Tauri 2（Rust 后端 + Vue3 前端）+ mihomo 代理内核
- 核心理念：不用 TUN 全接管，用「系统代理 + PROCESS-PATH-REGEX 进程规则」实现按软件精确分流
- SSH 密码/私钥通过 macOS Keychain 加密存储，不落明文

## 功能

- **按软件分流**：扫描本机 App（/Applications、/System/Applications），每个软件可设 代理 / 直连 / 智能
- **云服务器**：VLESS 节点管理（手动添加 + 订阅拉取）、SSH 终端（xterm.js）、Keychain 凭据存储
- **代理引擎**：mihomo 管理（启停、VLESS+Reality 配置生成、国内直连兜底）
- **冲突检测**：启动前检测 FlClash / mihomo / 端口占用
- **系统代理**：通过 networksetup 设置 macOS HTTP/SOCKS 代理

## 开发

```bash
# 前端依赖
cd ui
pnpm install

# 运行（需先启动前端 dev server）
pnpm dev          # 终端 1：Vite dev server
cargo tauri dev   # 终端 2：Tauri 应用（在 src-tauri 目录）
```

## 构建 .app

```bash
cd ui
pnpm build                     # 构建前端到 ui/dist
cd ../src-tauri
cargo tauri build              # 打包 macOS .app
```

产物在 `src-tauri/target/release/bundle/macos/`。

## 自检

```bash
cd src-tauri
cargo test                     # 运行 Rust 单元测试
cargo run --bin magic_probe    # 启动代理 -> 8 秒后停止（验证 mihomo 可用）
```

## 目录结构

```
ui/             Vue3 前端（Vite + xterm.js）
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

## 注意

- 启动代理前请关闭 FlClash / 其它占用 7891 端口的代理，否则会提示冲突。
- 系统代理只对「遵守系统代理」的软件生效；Telegram、部分游戏等可能自行直连，属于已知边界。
