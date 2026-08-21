#!/usr/bin/env python3
"""魔法代理 MCP Server —— 让 AI 直接控制代理 App。

工具：
  status        查看代理状态
  start_proxy   启动代理
  stop_proxy    停止代理
  list_apps     列出扫描到的 App 与分流设置
  set_app_mode  设置某 App 的代理模式（proxy/direct）
  list_nodes    列出代理节点
  switch_node   切换当前节点
  check_network 测试国内外网站连通性
"""
import json
import os
import subprocess
import sys
import urllib.request
import urllib.error

CONFIG_PATH = os.path.expanduser('~/Library/Application Support/magic-agent/config.json')
RUNTIME_DIR = os.path.expanduser('~/Library/Application Support/magic-agent/runtime')
MIHOMO_BIN = '/Users/zhangxuetao/Desktop/魔法代理/src-tauri/resources/bin/mihomo'
API = 'http://127.0.0.1:19091'


def api_get(path):
    try:
        r = urllib.request.urlopen(API + path, timeout=5)
        return json.loads(r.read().decode())
    except Exception as e:
        return {'error': str(e)}


def read_config():
    try:
        with open(CONFIG_PATH) as f:
            return json.load(f)
    except Exception as e:
        return {'error': str(e)}


def write_config(cfg):
    with open(CONFIG_PATH, 'w') as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)


def mihomo_running():
    p = subprocess.run(['/usr/bin/pgrep', '-f', 'resources/bin/mihomo'],
                       capture_output=True, text=True)
    return bool(p.stdout.strip())


def stop_mihomo():
    p = subprocess.run(['/usr/bin/pgrep', '-f', 'resources/bin/mihomo'],
                       capture_output=True, text=True)
    for pid in p.stdout.strip().split():
        script = f'do shell script "/bin/kill {pid}" with administrator privileges'
        subprocess.run(['/usr/bin/osascript', '-e', script],
                       capture_output=True, text=True, timeout=120)


def start_mihomo():
    conf = RUNTIME_DIR + '/mihomo.yaml'
    log = RUNTIME_DIR + '/mihomo.log'
    err = RUNTIME_DIR + '/mihomo.err.log'
    shell = f"'{MIHOMO_BIN}' -f '{conf}' -d '{RUNTIME_DIR}' > '{log}' 2> '{err}' & echo $!"
    escaped = shell.replace('\\', '\\\\').replace('"', '\\"')
    script = f'do shell script "{escaped}" with administrator privileges'
    p = subprocess.run(['/usr/bin/osascript', '-e', script],
                       capture_output=True, text=True, timeout=300)
    return p.stdout.strip()


def generate_config(cfg):
    """根据 AppConfig 生成 mihomo.yaml"""
    nodes = cfg.get('nodes', [])
    selected = cfg.get('selectedNode') or (nodes[0]['name'] if nodes else None)
    lines = []
    lines.append('mixed-port: 7891')
    lines.append('mode: rule')
    lines.append('tun:')
    lines.append('  enable: true')
    lines.append('  stack: system')
    lines.append('  auto-route: true')
    lines.append('  auto-detect-interface: true')
    lines.append('  dns-hijack:')
    lines.append('    - any:53')
    lines.append('log-level: info')
    lines.append('allow-lan: false')
    lines.append('ipv6: false')
    lines.append('find-process-mode: always')
    lines.append('external-controller: 127.0.0.1:19091')
    lines.append('geo-auto-update: false')
    lines.append('geodata-mode: false')
    lines.append('geodata-loader: memconservative')
    lines.append('')
    lines.append('dns:')
    lines.append('  enable: true')
    lines.append('  listen: 127.0.0.1:1054')
    lines.append('  enhanced-mode: redir-host')
    lines.append('  nameserver:')
    lines.append('    - 223.5.5.5')
    lines.append('    - 119.29.29.29')
    lines.append('  fallback:')
    lines.append('    - tls://8.8.8.8')
    lines.append('    - tls://1.1.1.1')
    lines.append('  fallback-filter:')
    lines.append('    geoip: true')
    lines.append('    geoip-code: CN')
    lines.append('')
    lines.append('proxies:')
    for n in nodes:
        lines.append(f'  - name: "{n["name"]}"')
        lines.append(f'    type: vless')
        lines.append(f'    server: "{n["server"]}"')
        lines.append(f'    port: {n["port"]}')
        lines.append(f'    uuid: "{n["uuid"]}"')
        lines.append(f'    network: {n.get("network", "tcp")}')
        lines.append(f'    tls: {n.get("tls", True)}')
        lines.append(f'    udp: {n.get("udp", True)}')
        lines.append(f'    flow: "{n.get("flow", "")}"')
        lines.append(f'    client-fingerprint: "{n.get("fingerprint", "chrome")}"')
        if n.get('sni'):
            lines.append(f'    servername: "{n["sni"]}"')
        if n.get('publicKey'):
            lines.append('    reality-opts:')
            lines.append(f'      public-key: "{n["publicKey"]}"')
            lines.append(f'      short-id: "{n.get("shortId", "")}"')
    lines.append('')
    lines.append('proxy-groups:')
    lines.append('  - name: PROXY')
    lines.append('    type: select')
    lines.append('    proxies:')
    ordered = sorted(nodes, key=lambda x: 0 if x['name'] == selected else 1)
    for n in ordered:
        lines.append(f'      - "{n["name"]}"')
    for n in nodes:
        lines.append(f'  - name: "NODE-{n["name"]}"')
        lines.append('    type: select')
        lines.append('    proxies:')
        lines.append(f'      - "{n["name"]}"')
    lines.append('')
    lines.append('rules:')
    for n in nodes:
        server = n['server']
        if server.replace('.', '').isdigit():
            lines.append(f'  - IP-CIDR,{server}/32,DIRECT,no-resolve')
        else:
            lines.append(f'  - DOMAIN-SUFFIX,{server},DIRECT')
    # 域名分流规则：同一个软件下载混合源时按域名决定代理/直连
    for dr in cfg.get('domainRules', []):
        target = 'PROXY' if dr.get('target') == 'proxy' else 'DIRECT'
        lines.append(f'  - DOMAIN-SUFFIX,{dr.get("domain", "")},{target}')
    lines.append('  - GEOIP,LAN,DIRECT,no-resolve')
    lines.append('  - MATCH,DIRECT')
    return '\n'.join(lines) + '\n'


def regenerate_config():
    cfg = read_config()
    if 'error' in cfg:
        return cfg
    os.makedirs(RUNTIME_DIR, exist_ok=True)
    conf = generate_config(cfg)
    with open(RUNTIME_DIR + '/mihomo.yaml', 'w') as f:
        f.write(conf)
    return {'ok': True}


def check_network():
    results = {}
    for name, url in [('baidu', 'https://www.baidu.com'), ('google', 'https://www.google.com')]:
        try:
            req = urllib.request.Request(url, method='HEAD')
            urllib.request.urlopen(req, timeout=8)
            results[name] = 'OK'
        except Exception as e:
            results[name] = f'FAIL ({e})'
    return results


TOOLS = [
    {'name': 'status', 'description': '查看魔法代理当前状态（mihomo、端口、节点、系统代理）'},
    {'name': 'start_proxy', 'description': '启动代理（TUN 模式，需管理员授权）'},
    {'name': 'stop_proxy', 'description': '停止代理'},
    {'name': 'list_nodes', 'description': '列出代理节点'},
    {'name': 'switch_node', 'description': '切换当前节点'},
    {'name': 'list_apps', 'description': '列出软件分流配置'},
    {'name': 'set_app_mode', 'description': '设置某 App 的代理模式（proxy/direct）'},
    {'name': 'check_network', 'description': '测试国内外网站连通性'},
    {'name': 'list_domain_rules', 'description': '列出域名分流规则（哪些域名走代理/直连）'},
    {'name': 'add_domain_rule', 'description': '添加或更新域名分流规则，如 {"domain":"github.com","target":"proxy"}'},
    {'name': 'remove_domain_rule', 'description': '删除域名分流规则，如 {"domain":"github.com"}'},
]


def call_tool(name, args):
    if name == 'status':
        running = mihomo_running()
        cfg = read_config()
        selected = cfg.get('selectedNode', '') if 'error' not in cfg else '?'
        return {'running': running, 'selectedNode': selected,
                'systemProxy': cfg.get('systemProxy', False) if 'error' not in cfg else '?',
                'nodes': len(cfg.get('nodes', [])) if 'error' not in cfg else 0}
    elif name == 'start_proxy':
        if mihomo_running():
            return {'ok': True, 'message': '代理已在运行'}
        regenerate_config()
        pid = start_mihomo()
        return {'ok': True, 'pid': pid, 'message': '代理已启动（需要管理员授权）'}
    elif name == 'stop_proxy':
        stop_mihomo()
        return {'ok': True, 'message': '代理已停止'}
    elif name == 'list_nodes':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        selected = cfg.get('selectedNode')
        return [{'name': n['name'], 'server': n['server'], 'port': n['port'],
                 'current': n['name'] == selected} for n in cfg.get('nodes', [])]
    elif name == 'switch_node':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        node_name = args.get('name', '')
        if not any(n['name'] == node_name for n in cfg.get('nodes', [])):
            return {'error': f'节点不存在: {node_name}'}
        cfg['selectedNode'] = node_name
        write_config(cfg)
        if mihomo_running():
            stop_mihomo()
            regenerate_config()
            start_mihomo()
        return {'ok': True, 'message': f'已切换到节点 {node_name}'}
    elif name == 'list_apps':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        return [{'id': a['id'], 'mode': a.get('mode'), 'confirmed': a.get('confirmed')}
                for a in cfg.get('apps', [])]
    elif name == 'set_app_mode':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        app_id = args.get('id', '')
        mode = args.get('mode', 'direct')
        if mode not in ('proxy', 'direct'):
            return {'error': 'mode must be proxy or direct'}
        found = False
        for a in cfg.get('apps', []):
            if a['id'] == app_id:
                a['mode'] = mode
                a['confirmed'] = True
                found = True
        if not found:
            cfg['apps'].append({'id': app_id, 'mode': mode, 'confirmed': True, 'node': None})
        write_config(cfg)
        if mihomo_running():
            stop_mihomo()
            regenerate_config()
            start_mihomo()
        return {'ok': True, 'message': f'{app_id} -> {mode}'}
    elif name == 'check_network':
        return check_network()
    elif name == 'list_domain_rules':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        return cfg.get('domainRules', [])
    elif name == 'add_domain_rule':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        domain = args.get('domain', '').strip()
        target = args.get('target', 'proxy')
        if not domain:
            return {'error': 'domain is required'}
        if target not in ('proxy', 'direct'):
            return {'error': 'target must be proxy or direct'}
        rules = cfg.get('domainRules', [])
        for r in rules:
            if r['domain'] == domain:
                r['target'] = target
                break
        else:
            rules.append({'domain': domain, 'target': target})
        cfg['domainRules'] = rules
        write_config(cfg)
        if mihomo_running():
            stop_mihomo()
            regenerate_config()
            start_mihomo()
        return {'ok': True, 'message': f'域名规则已保存: {domain} -> {target}'}
    elif name == 'remove_domain_rule':
        cfg = read_config()
        if 'error' in cfg:
            return cfg
        domain = args.get('domain', '').strip()
        rules = cfg.get('domainRules', [])
        cfg['domainRules'] = [r for r in rules if r['domain'] != domain]
        write_config(cfg)
        if mihomo_running():
            stop_mihomo()
            regenerate_config()
            start_mihomo()
        return {'ok': True, 'message': f'域名规则已删除: {domain}'}
    return {'error': f'unknown tool {name}'}


def main():
    # MCP stdio JSON-RPC 循环
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        method = msg.get('method', '')
        msg_id = msg.get('id')
        if method == 'initialize':
            resp = {'jsonrpc': '2.0', 'id': msg_id, 'result': {
                'protocolVersion': '2024-11-05',
                'capabilities': {'tools': {}},
                'serverInfo': {'name': 'magic-agent', 'version': '0.1.0'}}}
            print(json.dumps(resp, ensure_ascii=False), flush=True)
        elif method == 'tools/list':
            resp = {'jsonrpc': '2.0', 'id': msg_id, 'result': {'tools': TOOLS}}
            print(json.dumps(resp, ensure_ascii=False), flush=True)
        elif method == 'tools/call':
            params = msg.get('params', {})
            tool_name = params.get('name', '')
            tool_args = params.get('arguments', {})
            try:
                result = call_tool(tool_name, tool_args)
            except Exception as e:
                result = {'error': str(e)}
            resp = {'jsonrpc': '2.0', 'id': msg_id,
                    'result': {'content': [{'type': 'text', 'text': json.dumps(result, ensure_ascii=False)}]}}
            print(json.dumps(resp, ensure_ascii=False), flush=True)
        elif method == 'notifications/initialized':
            pass
        else:
            resp = {'jsonrpc': '2.0', 'id': msg_id,
                    'error': {'code': -32601, 'message': 'method not found'}}
            print(json.dumps(resp, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
