#!/usr/bin/env python3
"""双规则引擎一致性校验：Rust build_rules vs Python generate_config。

背景：魔法代理有两份规则生成逻辑（Rust 冷启动、Python MCP 热重载），
历史上漂移过一次（域名/进程规则顺序颠倒）。此脚本用同一份样例配置
分别喂给两侧引擎，对 rules 段逐行 diff，不一致则退出码 1。

用法：python3 scripts/check_parity.py
前置：cargo build（需要 target/debug/dump_conf）
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, 'mcp'))
import server  # noqa: E402

# 样例配置：app 条目全部用 bin- 前缀（不依赖实机 /Applications 扫描，两侧可确定性对比）
SAMPLE = {
    "nodes": [
        {"name": "测试节点A", "server": "203.0.113.10", "port": 443, "uuid": "u-1",
         "network": "tcp", "tls": True, "udp": True, "flow": "xtls-rprx-vision",
         "fingerprint": "chrome", "publicKey": "pk-1", "shortId": "sid-1", "sni": ""},
        {"name": "测试节点B", "server": "proxy-b.example.com", "port": 443, "uuid": "u-2",
         "network": "tcp", "tls": True, "udp": True, "flow": "",
         "fingerprint": "chrome", "publicKey": "", "shortId": "", "sni": ""},
    ],
    "selectedNode": "测试节点A",
    "domainRules": [
        {"domain": "github.com", "target": "proxy"},
        {"domain": "bilibili.com", "target": "direct"},
        {"domain": "openai.com", "target": "测试节点B"},
        {"domain": "company.internal", "target": "direct"},
    ],
    "apps": [
        # 多节点共用同一 server，验证防卷去重；三种 target 全覆盖 + 未确认条目应被忽略
        {"id": "bin-/opt/tools/downloader", "mode": "proxy", "node": "测试节点B", "reason": "t", "confirmed": True},
        {"id": "bin-/opt/monitor/core", "mode": "proxy", "node": None, "confirmed": True},
        {"id": "bin-/opt/local/tool", "mode": "direct", "confirmed": True},
        {"id": "bin-/opt/unconfirmed/app", "mode": "proxy", "confirmed": False},
        # 注：app- 前缀条目依赖实机 /Applications 扫描（Python 找不到还会构造兜底路径，
        # Rust 则跳过），属非确定性行为，不纳入 parity 范围；分流表实际以 Rust 扫描为准。
    ],
    "systemProxy": True,
    "autoGlobal": "auto",
    "sshHost": None,
    "sshPort": 22,
    "sshUser": "root",
    "sshAuth": "password",
    "sshPassword": None,
    "sshPrivateKey": None,
}


def extract_rules(yaml_text):
    sec = yaml_text.split('\nrules:\n')[1]
    return [l.strip()[2:] for l in sec.splitlines() if l.strip().startswith('- ')]


def main():
    py_rules = extract_rules(server.generate_config(SAMPLE))

    bin_path = os.path.join(ROOT, 'src-tauri', 'target', 'debug', 'dump_conf')
    if not os.path.exists(bin_path):
        print('缺少 dump_conf 二进制，请先: cd src-tauri && cargo build')
        sys.exit(2)
    rust_out = subprocess.run([bin_path], input=json.dumps(SAMPLE),
                              capture_output=True, text=True)
    if rust_out.returncode != 0:
        print('dump_conf 失败:', rust_out.stderr[:300])
        sys.exit(2)
    rust_rules = [l for l in rust_out.stdout.splitlines() if l.strip()]

    # Python 侧的进程路径会做 regex 转义（re.escape），Rust 侧 regex_escape_path 同理；
    # 两侧转义字符集不同（Python 转义更多），进程规则行只比较结构，其余行严格相等。
    def normalize(lines):
        out = []
        for l in lines:
            if l.startswith('PROCESS-PATH-REGEX,'):
                head, _, target = l.rpartition(',')
                out.append(f'{head}|{target}')  # 只比路径+目标结构，忽略转义差异
            else:
                out.append(l)
        return out

    a, b = normalize(py_rules), normalize(rust_rules)
    if a == b:
        print(f'✅ 双引擎一致，共 {len(a)} 条规则')
        for l in a:
            print('  ', l)
        sys.exit(0)

    print(f'❌ 规则引擎漂移！Python {len(a)} 条 vs Rust {len(b)} 条')
    for i, (x, y) in enumerate(zip(a, b)):
        if x != y:
            print(f'  第{i}条:\n    Python: {x}\n    Rust:   {y}')
    if len(a) != len(b):
        longer, tag = (a, 'Python') if len(a) > len(b) else (b, 'Rust')
        for l in longer[min(len(a), len(b)):]:
            print(f'  仅 {tag}: {l}')
    sys.exit(1)


if __name__ == '__main__':
    main()
