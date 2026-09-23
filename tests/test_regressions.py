import importlib.util
import json
import pathlib
import sys
import threading
import urllib.error
import urllib.request
from http.client import HTTPConnection

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'mcp'))

spec = importlib.util.spec_from_file_location('magic_server', ROOT / 'mcp' / 'server.py')
server = importlib.util.module_from_spec(spec)
spec.loader.exec_module(server)


def test_server_metrics_include_identity_like_rust_consumer_expects(monkeypatch):
    monkeypatch.setattr(server, '_active_server', lambda: {
        'name': 'prod-1', 'host': '203.0.113.10', 'user': 'root', 'port': 22,
        'auth': 'password',
    })
    monkeypatch.setattr(server, 'ssh_exec', lambda cmd, timeout_secs=15: (
        '---CPU---\n%Cpu(s): 5.0 us, 95.0 id\n'
        '---MEM---\nMem: 1000 500 100 0 0 400\n'
        '---DISK---\n/dev/vda1 50G 20G 30G 40% /\n'
        '---LOAD---\n0.10 0.20 0.30 1/10 1\n'
        '---UPTIME---\nup 1 day\n'
        '---NET---\neth0: 1 0 0 0 0 0 0 0 2 0 0 0 0 0 0 0\n',
        '', 0,
    ))

    result = server.server_metrics({})

    assert result['server'] == {
        'name': 'prod-1', 'host': '203.0.113.10', 'user': 'root',
    }
    assert result['cpu_usage_pct'] == 5.0
    assert result['mem_usage_pct'] == 50.0
    assert result['disk_usage_pct'] == '40'
    assert result['net_eth0_tx_bytes'] == 2.0


def test_tools_expose_input_schema_for_mcp_clients():
    tools = {tool['name']: tool for tool in server.TOOLS}

    assert tools['switch_node']['inputSchema'] == {
        'type': 'object',
        'properties': {'name': {'type': 'string'}},
        'required': ['name'],
        'additionalProperties': False,
    }
    assert tools['ssh_exec']['inputSchema']['properties']['command']['type'] == 'string'
    assert tools['ssh_exec']['inputSchema']['properties']['timeout_secs']['type'] == 'integer'
    assert tools['server_metrics']['inputSchema'] == {
        'type': 'object', 'properties': {}, 'additionalProperties': False,
    }


def test_rule_mutation_reports_hot_reload_failure(monkeypatch):
    cfg = {
        'nodes': [], 'selectedNode': None, 'apps': [], 'domainRules': [],
        'systemProxy': False,
    }
    monkeypatch.setattr(server, 'read_config', lambda: json.loads(json.dumps(cfg)))
    monkeypatch.setattr(server, 'write_config', lambda value: None)
    monkeypatch.setattr(server, 'mihomo_running', lambda: True)
    monkeypatch.setattr(server, 'hot_reload_rules', lambda value: False)

    result = server.call_tool('add_domain_rule', {'domain': 'example.com', 'target': 'proxy'})

    assert result['ok'] is False
    assert '热更新' in result['message']


def test_deleted_node_domain_rule_falls_back_to_proxy_group():
    cfg = {
        'nodes': [{
            'name': 'alive', 'server': '203.0.113.1', 'port': 443,
            'uuid': 'u-1', 'network': 'tcp', 'tls': True, 'udp': True,
            'flow': '', 'fingerprint': 'chrome', 'publicKey': '',
            'shortId': '', 'sni': '',
        }],
        'selectedNode': 'alive',
        'domainRules': [{'domain': 'old.example', 'target': 'deleted'}],
        'apps': [], 'systemProxy': False,
    }

    text = server.generate_config(cfg)

    assert 'DOMAIN-SUFFIX,old.example,PROXY' in text
    assert 'DOMAIN-SUFFIX,old.example,NODE-deleted' not in text


def test_deleted_node_app_rule_falls_back_to_proxy_group():
    cfg = {
        'nodes': [{
            'name': 'alive', 'server': '203.0.113.1', 'port': 443,
            'uuid': 'u-1', 'network': 'tcp', 'tls': True, 'udp': True,
            'flow': '', 'fingerprint': 'chrome', 'publicKey': '',
            'shortId': '', 'sni': '',
        }],
        'selectedNode': 'alive',
        'domainRules': [],
        'apps': [{'id': 'bin-/opt/demo', 'mode': 'proxy', 'node': 'deleted', 'confirmed': True}],
        'systemProxy': False,
    }

    text = server.generate_config(cfg)

    assert 'PROCESS-PATH-REGEX,^/opt/demo,PROXY' in text
    assert 'NODE-deleted' not in text


def test_http_bridge_rejects_foreign_origin_and_requires_bearer_token(monkeypatch):
    token = 'a' * 64
    monkeypatch.setattr(server, 'HTTP_BRIDGE_TOKEN', token)
    httpd = server.create_http_server(0)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    port = httpd.server_address[1]
    try:
        conn = HTTPConnection('127.0.0.1', port, timeout=5)
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/list'}).encode()
        conn.request('POST', '/extension', body=body, headers={
            'Content-Type': 'application/json',
            'Origin': 'https://evil.example',
            'Authorization': 'Bearer ' + token,
        })
        blocked = conn.getresponse()
        assert blocked.status == 403
        blocked.read()
        conn.close()

        conn = HTTPConnection('127.0.0.1', port, timeout=5)
        conn.request('POST', '/extension', body=body, headers={
            'Content-Type': 'application/json',
            'Origin': f'http://127.0.0.1:{port}',
        })
        denied = conn.getresponse()
        assert denied.status == 401
        denied.read()
        conn.close()
    finally:
        httpd.shutdown()
        httpd.server_close()


def test_http_bridge_allows_native_health_without_origin_or_token(monkeypatch):
    """Launcher 和原生客户端没有 Origin；/health 必须保持无鉴权可探测。"""
    monkeypatch.setattr(server, 'HTTP_BRIDGE_TOKEN', 'a' * 64)
    httpd = server.create_http_server(0)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    port = httpd.server_address[1]
    try:
        conn = HTTPConnection('127.0.0.1', port, timeout=5)
        conn.request('GET', '/health')
        response = conn.getresponse()
        assert response.status == 200
        assert json.loads(response.read())['ok'] is True
        conn.close()
    finally:
        httpd.shutdown()
        httpd.server_close()


def test_http_bridge_allows_native_jsonrpc_without_origin_or_token(monkeypatch):
    """非浏览器客户端（无 Origin）不能被 bearer 鉴权破坏兼容性。"""
    monkeypatch.setattr(server, 'HTTP_BRIDGE_TOKEN', 'a' * 64)
    httpd = server.create_http_server(0)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    port = httpd.server_address[1]
    try:
        conn = HTTPConnection('127.0.0.1', port, timeout=5)
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/list'}).encode()
        conn.request('POST', '/extension', body=body, headers={
            'Content-Type': 'application/json',
        })
        response = conn.getresponse()
        assert response.status == 200
        payload = json.loads(response.read())
        assert payload['result']['tools']
        conn.close()
    finally:
        httpd.shutdown()
        httpd.server_close()


# ── P0-1 逐服务对账（与 Rust system_proxy.rs 同一契约）──
# ⚠️ fixture 全部采用 2026-09-23 真机实测（Darwin 25.6）的
# networksetup -getwebproxy 输出格式（Enabled: Yes / Server / Port 行式），
# 且读命令只用 -getwebproxy/-getsecurewebproxy/-getsocksfirewallproxy——
# -get*state 系列读命令真机不存在。上一版 fixture 用臆测的
# "Web Proxy ... is Enabled." + -get*state，测试全绿但真实对账恒失败，教训在此。

class _Proc:
    def __init__(self, stdout='', stderr='', rc=0):
        self.stdout, self.stderr, self.returncode = stdout, stderr, rc


def _proxy_out(on, port=7891):
    """真机格式：开=Enabled: Yes + 端口；关=Enabled: No。"""
    if on:
        return f'Enabled: Yes\nServer: 127.0.0.1\nPort: {port}\nAuthenticated Proxy Enabled: 0'
    return 'Enabled: No\nServer: \nPort: 0\nAuthenticated Proxy Enabled: 0'


def _fake_networksetup(state_map):
    """伪造 networksetup/scutil：state_map[服务][flag]=True/False/'port=XXXX'/Exception。
    写命令（-set*）一律成功；读命令按 state_map 生成真机格式输出。"""
    def runner(cmd, **kw):
        if cmd[0] == '/usr/sbin/scutil':
            return _Proc(stdout='HTTPEnable : 1\nHTTPPort : 7891\nSOCKSPort : 7891\n')
        if cmd[0] != '/usr/sbin/networksetup':
            raise AssertionError(f'unexpected cmd {cmd}')
        flag = cmd[1]
        if flag == '-listallnetworkservices':
            return _Proc(stdout='An asterisk (*) denotes that a network service is disabled.\nWi-Fi\nUSB 10/100 LAN\nAX88179A\nBluetooth-LE\n')
        if flag.startswith('-set'):
            return _Proc()
        svc = cmd[2]
        st = state_map.get(svc, {})
        r = st.get(flag, True)
        if r is Exception:
            raise RuntimeError('mock denied')
        if isinstance(r, str) and r.startswith('port='):
            return _Proc(stdout=_proxy_out(True, int(r[5:])))
        return _Proc(stdout=_proxy_out(bool(r)))
    return runner


def test_set_system_proxy_partial_failure_is_reported_not_swallowed(monkeypatch):
    """3 成功 1 失败：结果必须如实点名失败服务（旧 any_success 会谎报整体成功）。"""
    state_map = {
        'Wi-Fi': {'-getwebproxy': True, '-getsecurewebproxy': True, '-getsocksfirewallproxy': True},
        'USB 10/100 LAN': {'-getwebproxy': True, '-getsecurewebproxy': True, '-getsocksfirewallproxy': True},
        'AX88179A': {'-getwebproxy': True, '-getsecurewebproxy': True, '-getsocksfirewallproxy': False},
    }
    monkeypatch.setattr(server.subprocess, 'run', _fake_networksetup(state_map))
    result = server.set_system_proxy(True)
    assert result['allOk'] is False
    assert result['mismatched'] == ['AX88179A']
    ax = [s for s in result['services'] if s['service'] == 'AX88179A'][0]
    assert ax['httpOn'] is True and ax['socksOn'] is False
    # 蓝牙等虚拟口被过滤，不出现在对账结果里
    assert [s['service'] for s in result['services']] == ['Wi-Fi', 'USB 10/100 LAN', 'AX88179A']


def test_set_system_proxy_all_success(monkeypatch):
    state_map = {svc: {'-getwebproxy': True, '-getsecurewebproxy': True,
                       '-getsocksfirewallproxy': True}
                 for svc in ('Wi-Fi', 'USB 10/100 LAN', 'AX88179A')}
    monkeypatch.setattr(server.subprocess, 'run', _fake_networksetup(state_map))
    result = server.set_system_proxy(True)
    assert result['allOk'] is True
    assert result['mismatched'] == []


def test_set_system_proxy_port_mismatch_is_not_ok(monkeypatch):
    """开着但指向 7890（真机 Thunderbolt Bridge 实测残留场景）：期望 7891 必须判不达标。"""
    state_map = {
        svc: {'-getwebproxy': True, '-getsecurewebproxy': True, '-getsocksfirewallproxy': True}
        for svc in ('Wi-Fi', 'USB 10/100 LAN')
    }
    state_map['AX88179A'] = {'-getwebproxy': 'port=7890', '-getsecurewebproxy': True,
                             '-getsocksfirewallproxy': True}
    monkeypatch.setattr(server.subprocess, 'run', _fake_networksetup(state_map))
    result = server.set_system_proxy(True)
    assert result['allOk'] is False
    assert result['mismatched'] == ['AX88179A']


def test_set_system_proxy_write_denied_still_accounted(monkeypatch):
    """读回恰好达标但写命令报错（权限拒绝）：仍须点名，错误信息不丢。"""
    def runner(cmd, **kw):
        if cmd[0] == '/usr/sbin/scutil':
            return _Proc(stdout='HTTPEnable : 1\nHTTPPort : 7891\nSOCKSPort : 7891\n')
        flag = cmd[1]
        if flag == '-listallnetworkservices':
            return _Proc(stdout='Wi-Fi\n')
        if flag == '-setsocksfirewallproxy':
            return _Proc(stderr='CommandTool: Unable to set SOCKS proxy: denied', rc=1)
        return _Proc(stdout=_proxy_out(True))
    monkeypatch.setattr(server.subprocess, 'run', runner)
    result = server.set_system_proxy(True)
    assert result['allOk'] is False
    assert result['mismatched'] == ['Wi-Fi']


def test_set_system_proxy_read_error_marked_mismatched(monkeypatch):
    """读回抛异常（命令不可用等）：该服务记 errors 并点名，绝不静默通过。"""
    state_map = {
        'Wi-Fi': {'-getwebproxy': True, '-getsecurewebproxy': True, '-getsocksfirewallproxy': True},
        'USB 10/100 LAN': Exception,
        'AX88179A': {'-getwebproxy': True, '-getsecurewebproxy': True, '-getsocksfirewallproxy': True},
    }
    monkeypatch.setattr(server.subprocess, 'run', _fake_networksetup(state_map))
    result = server.set_system_proxy(True)
    assert result['allOk'] is False
    assert 'USB 10/100 LAN' in result['mismatched']
    usb = [s for s in result['services'] if s['service'] == 'USB 10/100 LAN'][0]
    assert usb['errors']


def test_verify_get_state_command_absent_real_machine():
    """真机回归锁：读侧绝不能使用 -get*state 系列命令（真机不存在，读写命令集不对称）。
    直接扫两侧源码，出现即失败。"""
    import pathlib
    py = (pathlib.Path(__file__).resolve().parents[1] / 'mcp' / 'server.py').read_text(encoding='utf-8')
    # verify 函数体内不得出现 get 系 state 读命令（set 系合法）
    seg = py[py.index('def verify_system_proxy'):py.index('def list_network_services')]
    assert '-getwebproxystate' not in seg and '-getsecurewebproxystate' not in seg and '-getsocksfirewallproxystate' not in seg
    rs = (pathlib.Path(__file__).resolve().parents[1] / 'src-tauri' / 'src' / 'system_proxy.rs').read_text(encoding='utf-8')
    seg_rs = rs[rs.index('pub fn verify_system_proxy'):rs.index('fn get_proxy_detail')]
    assert '-getwebproxystate' not in seg_rs and '-getsecurewebproxystate' not in seg_rs and '-getsocksfirewallproxystate' not in seg_rs


# ── P1-1 audit_network（网络体检，与 Rust auditor.rs 同口径）──

_AUDIT_PROXY_ALL_OFF = ('<dictionary> {\n  FTPPassive : 1\n  HTTPEnable : 0\n  HTTPSEnable : 0\n'
                        '  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}')


def _fake_audit_runner(listen_text=None, ps_text=None, proxy_text=_AUDIT_PROXY_ALL_OFF):
    """伪装修检用到的全部子进程输出（真机格式，2026-09-23 取证）。"""
    def runner(cmd, **kw):
        c = cmd[0]
        if c == '/bin/ps':
            return _Proc(stdout=ps_text if ps_text is not None else
                         '  123 /Applications/SomeApp.app/Contents/MacOS/SomeApp\n')
        if c == '/usr/sbin/lsof':
            return _Proc(stdout=listen_text if listen_text is not None else
                         'COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME\n')
        if c == '/usr/sbin/scutil' and cmd[1] == '--proxy':
            return _Proc(stdout=proxy_text)
        if c == '/usr/sbin/scutil' and cmd[1] == '--dns':
            return _Proc(stdout='DNS configuration\n\nresolver #1\n  nameserver[0] : 223.5.5.5\n\nresolver #2\n  domain   : local\n')
        if c == '/sbin/route':
            return _Proc(stdout='   route to: default\ngateway: 172.18.100.1\n  interface: en0\n')
        raise AssertionError(f'unexpected cmd {cmd}')
    return runner


def test_audit_network_clean_machine(monkeypatch):
    monkeypatch.setattr(server.subprocess, 'run', _fake_audit_runner())
    r = server.audit_network()
    assert r['foreignProcs'] == []
    assert r['portConflicts'] == []
    assert r['staleProxy']['detected'] is False
    assert r['pacEnabled'] == 'no'
    assert r['summary'] == '未发现混乱源'
    assert r['degraded'] == []


def test_audit_network_detects_foreign_proxy_and_stale(monkeypatch):
    ps = ('  100 /Applications/FlClash.app/Contents/MacOS/FlClash\n'
          '  101 grep clash\n'  # 铁律：命令行含 clash 但可执行路径不是 → 绝不误报
          '  102 /usr/sbin/cupsd -l\n')
    listen = ('COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME\n'
              'Python  200 x      1u IPv4 0xa 0t0 TCP 127.0.0.1:7891 (LISTEN)\n')
    proxy = ('<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 7891\n'
             '  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}')
    monkeypatch.setattr(server.subprocess, 'run', _fake_audit_runner(ps_text=ps, listen_text=listen, proxy_text=proxy))
    r = server.audit_network()
    assert any('flclash' in p.lower() for p in r['foreignProcs']), r['foreignProcs']
    assert not any('grep' in p for p in r['foreignProcs']), '误杀非代理进程'
    # 7891 被 Python 占用：内核不算在跑 → 指向 7891 的系统代理判为死端口残留
    assert r['portConflicts'] and r['portConflicts'][0]['port'] == 7891
    assert r['staleProxy']['detected'] is True
    assert '7891' in r['staleProxy']['detail']
    assert '第三方代理' in r['summary'] and '死端口' in r['summary']


def test_audit_network_degrades_on_command_failure(monkeypatch):
    """单项采集失败 → 该维度降级，报告整体仍可用（不抛异常）。"""
    def runner(cmd, **kw):
        raise RuntimeError('exec denied')
    monkeypatch.setattr(server.subprocess, 'run', runner)
    r = server.audit_network()
    assert 'foreign_procs' in r['degraded']
    assert 'listen_sockets' in r['degraded']
    assert '降级' in r['summary']
    assert r['route']['state'] == 'unknown'


def test_audit_network_registered_in_tools_and_dispatch():
    """工具注册锁：TOOLS 有描述、schema 有空参、分派可达。"""
    names = [t['name'] for t in server.TOOLS]
    assert 'audit_network' in names
    assert server._TOOL_SCHEMAS['audit_network'] == {
        'type': 'object', 'properties': {}, 'additionalProperties': False,
    }
    import pathlib
    src = (pathlib.Path(__file__).resolve().parents[1] / 'mcp' / 'server.py').read_text(encoding='utf-8')
    assert "'audit_network': _tool_schema()" in src
    assert "elif name == 'audit_network':" in src
