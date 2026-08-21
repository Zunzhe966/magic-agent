# 魔法代理 MCP Server

让 AI 助手（Claude Desktop / Codex / WorkBuddy 等）直接控制魔法代理。

## 是什么

MCP（Model Context Protocol）是 AI 客户端与外部工具之间的标准协议。
本 server 通过 stdio 与 AI 客户端通信，暴露 13 个工具，AI 可以：

- 查看代理状态、启动/停止代理
- 列出节点、切换节点、测试节点延迟
- 列出软件分流、设置某 App 走代理/直连
- 添加/删除域名分流规则
- 拉取 VLESS 订阅
- 测试国内外网络连通性

## 工具清单

| 工具 | 参数 | 说明 |
|---|---|---|
| status | - | 查看代理状态 |
| start_proxy | - | 启动代理（TUN，需管理员授权） |
| stop_proxy | - | 停止代理 |
| list_nodes | - | 列出节点 |
| switch_node | {name} | 切换当前节点（API 秒切，不重启） |
| test_node_delay | {name} | 测试节点延迟 ms |
| list_apps | - | 列出软件分流配置 |
| set_app_mode | {id, mode} | 设置 App 走 proxy/direct |
| list_domain_rules | - | 列出域名分流规则 |
| add_domain_rule | {domain, target} | 添加域名规则（target=proxy/direct） |
| remove_domain_rule | {domain} | 删除域名规则 |
| fetch_subscription | {url} | 从订阅 URL 拉取 VLESS 节点 |
| check_network | - | 测试百度直连 + Google 走代理 |

## 配置到 AI 客户端

### Claude Desktop

编辑 `~/Library/Application Support/Claude/claude_desktop_config.json`：

```json
{
  "mcpServers": {
    "magic-agent": {
      "command": "python3",
      "args": ["/Users/zhangxuetao/Desktop/魔法代理/mcp/server.py"]
    }
  }
}
```

### Codex / WorkBuddy

在对应 MCP 配置中注册同一命令即可：

```
command: python3
args: /Users/zhangxuetao/Desktop/魔法代理/mcp/server.py
```

## 直接测试

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | python3 mcp/server.py
```

## 注意

- server 依赖：系统 curl、base64、mihomo external-controller（127.0.0.1:19091）
- 启动/停止代理需要 macOS 管理员授权（osascript 弹框）
- 切换节点、修改分流/域名规则走 mihomo PATCH /configs 热更新，不弹框
