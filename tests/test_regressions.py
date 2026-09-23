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


# ── P1-2 接管账本（与 Rust ledger.rs 共写同一 ledger.json）──

def _audit_proxy_line(on, port=7890, server_ip='127.0.0.1'):
    if on:
        return f'Enabled: Yes\nServer: {server_ip}\nPort: {port}\nAuthenticated Proxy Enabled: 0'
    return 'Enabled: No\nServer: \nPort: 0\nAuthenticated Proxy Enabled: 0'


def _fake_ledger_networksetup(webproxy_on=True, port=7890):
    def runner(cmd, **kw):
        if cmd[0] == '/usr/sbin/networksetup':
            flag = cmd[1]
            if flag == '-listallnetworkservices':
                return _Proc(stdout='Wi-Fi\nUSB 10/100 LAN\n')
            if flag == '-getwebproxy':
                return _Proc(stdout=_audit_proxy_line(webproxy_on, port))
            return _Proc(stdout=_audit_proxy_line(False))
        raise AssertionError(f'unexpected cmd {cmd}')
    return runner


def test_ledger_begin_settle_cycle(tmp_path, monkeypatch):
    """完整周期：begin 记全原值 → 二次 begin 不覆盖 before → settle 结账 → 可再开新会话。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _fake_ledger_networksetup(webproxy_on=True, port=7890))

    assert server.ledger_begin('mcp:start_proxy') is True
    session = server.ledger_open_session()
    assert session is not None
    assert session['reason'] == 'mcp:start_proxy'
    entries = {e['key']: e for e in session['entries']}
    # before 必须携带【完整原值】：开着指向第三方端口 7890
    assert entries['Wi-Fi']['before']['http'] == {'enabled': True, 'server': '127.0.0.1', 'port': 7890}
    assert entries['Wi-Fi']['reversible'] is True

    # 二次接管：幂等，不覆盖最初 before（哪怕现在系统已被改成指向 7891）
    monkeypatch.setattr(server.subprocess, 'run', _fake_ledger_networksetup(webproxy_on=True, port=7891))
    assert server.ledger_begin('mcp:start_proxy2') is False
    session = server.ledger_open_session()
    assert session['reason'] == 'mcp:start_proxy'
    assert {e['key']: e for e in session['entries']}['Wi-Fi']['before']['http']['port'] == 7890

    # 被杀进程入账（不可逆）
    server.ledger_record_killed(['FlClash (PID 123)'])
    session = server.ledger_open_session()
    procs = [e for e in session['entries'] if e['kind'] == 'process']
    assert procs and procs[0]['reversible'] is False
    assert procs[0]['before'] == 'running' and procs[0]['after'] == 'killed'

    assert server.ledger_settle('mcp:stop_proxy') is True
    assert server.ledger_open_session() is None
    assert server.ledger_settle('again') is False  # 幂等

    # 结账后再 begin → 新会话，before 取新原值
    assert server.ledger_begin('mcp:start_proxy3') is True
    ledger, _ = server._ledger_load()
    assert len(ledger['sessions']) == 2


def test_ledger_corrupt_preserved_then_empty(tmp_path, monkeypatch, capsys):
    """损坏账本：保全为 .corrupt 证据（不覆盖已有），从空继续，note 打日志不静默。"""
    path = tmp_path / 'ledger.json'
    (tmp_path / 'ledger.json.corrupt').write_text('OLD EVIDENCE')
    path.write_text('{ not json !!!')
    monkeypatch.setattr(server, 'LEDGER_PATH', str(path))
    ledger, note = server._ledger_load()
    assert ledger['sessions'] == []
    assert note and '损坏' in note
    assert (tmp_path / 'ledger.json.corrupt').read_text() == 'OLD EVIDENCE'  # 证据未被覆盖
    assert not path.exists()  # 损坏文件被移走
    assert any('ledger' in f.name for f in tmp_path.iterdir() if '.corrupt.' in f.name)  # 时间戳副本


def test_ledger_schema_camel_case_contract_with_rust(tmp_path, monkeypatch):
    """跨引擎契约锁：Python 写的账本 JSON 键必须是 Rust 侧同一套 camelCase。
    改任一侧字段名 = 破坏共账，必须有测试挡。"""
    import pathlib
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _fake_ledger_networksetup())
    server.ledger_begin('contract-test')
    server.ledger_settle('contract-test-end')  # settledTs/settledReason 仅结账后序列化（两侧同语义）
    text = pathlib.Path(path).read_text(encoding='utf-8')
    for key in ['"version"', '"sessions"', '"startedTs"', '"settledTs"', '"settledReason"',
                '"reversible"', '"before"', '"system_proxy"', '"service"', '"http"', '"errors"']:
        assert key in text, f'账本 JSON 缺契约键 {key}'
    for bad in ['"started_ts"', '"settled_ts"']:
        assert bad not in text, f'账本 JSON 出现非契约键 {bad}'
    # 权限：接管账本含网络配置原值，必须 0600
    import stat
    assert stat.S_IMODE(pathlib.Path(path).stat().st_mode) == 0o600


# ── P1-3 回滚原语（与 Rust rollback_session 同口径） ──

def test_ledger_rollback_restores_original_values(tmp_path, monkeypatch):
    """回滚 = 把 before 快照逐服务回放回系统 + 结账；错误如实返回不假装成功。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _fake_ledger_networksetup(webproxy_on=True, port=7890))
    server.ledger_begin('rollback-test')

    calls = []

    def recorder(cmd, **kw):
        # 回放阶段的 networksetup 调用全部记录（含写命令参数）
        calls.append(list(cmd))
        return _Proc()
    monkeypatch.setattr(server.subprocess, 'run', recorder)

    errs, had, had_procs = server.ledger_rollback('verify 不达标')
    assert had and not had_procs
    assert errs == []
    # Wi-Fi 原值开着指向 7890，回放必须按原值恢复而不是关成直连
    wifi_restore = [c for c in calls if '-setwebproxy' in c]
    assert any('Wi-Fi' in c and '7890' in c for c in wifi_restore), wifi_restore
    assert any('-setwebproxystate' in c and 'on' in c for c in calls), '开着的原值必须恢复为 on'
    # 回滚即结账
    assert server.ledger_open_session() is None


def test_ledger_rollback_reports_write_errors_not_swallowed(tmp_path, monkeypatch):
    """回放写命令失败 → 错误如实返回（绝不假装还原成功），且仍然结账。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _fake_ledger_networksetup())
    server.ledger_begin('rollback-fail')

    def failer(cmd, **kw):
        return _Proc(stderr='denied', rc=1)
    monkeypatch.setattr(server.subprocess, 'run', failer)

    errs, had, _ = server.ledger_rollback('boom')
    assert had
    assert errs, '写失败必须逐条点名'
    assert server.ledger_open_session() is None  # 有失败也结账——接管已结束


def test_ledger_rollback_without_session(tmp_path, monkeypatch):
    """无未结账本 → (空, False)：如实报告"没账可回"，不假装还原成功。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    errs, had, had_procs = server.ledger_rollback('nothing')
    assert errs == [] and had is False and had_procs is False


def test_start_proxy_rolls_back_when_system_proxy_verify_fails(tmp_path, monkeypatch):
    """端到端（MCP 分派层）：系统代理对账不达标 → 停内核 + 回滚 + ok:False。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    # 准备：config 有节点且 systemProxy=true
    saved_config = {'nodes': [{'name': 'n1', 'server': '1.2.3.4', 'port': 443, 'uuid': 'u',
                               'network': 'tcp', 'tls': True, 'udp': True, 'flow': '',
                               'fingerprint': 'chrome', 'publicKey': '', 'shortId': '', 'sni': ''}],
                    'selectedNode': 'n1', 'systemProxy': True, 'apps': [], 'domainRules': []}
    monkeypatch.setattr(server, 'read_config', lambda: dict(saved_config))
    monkeypatch.setattr(server, 'write_config', lambda cfg: None)
    monkeypatch.setattr(server, 'mihomo_running', lambda: False)
    monkeypatch.setattr(server, 'regenerate_config', lambda: None)
    monkeypatch.setattr(server, 'start_mihomo', lambda: 4242)
    stop_calls = []
    monkeypatch.setattr(server, 'stop_mihomo', lambda: stop_calls.append(1))
    # 系统代理"设置成功但对账不达标"（allOk False）
    monkeypatch.setattr(server, 'set_system_proxy',
                        lambda enable=True, port=7891: {'enabled': enable, 'allOk': False,
                                                        'mismatched': ['Wi-Fi'], 'services': []})
    # 回滚本身用真账本（tmp_path 已隔离）
    r = server.call_tool('start_proxy', {})
    assert r['ok'] is False
    assert r['rolledBack'] is True
    assert stop_calls == [1], '内核必须被停掉'
    assert 'Wi-Fi' in r['message'] and '回滚' in r['message']
    assert server.ledger_open_session() is None, '回滚后不得留未结账本'


# ── P1-4 一键还原 restore_network / 漂移归位 / 巡检（与 Rust 同语义） ──

def _networksetup_full(webproxy_on=True, port=7890):
    """get 返回真机格式；set 一律成功（回放/归位路径）。"""
    def runner(cmd, **kw):
        if cmd[0] == '/usr/sbin/networksetup':
            flag = cmd[1]
            if flag == '-listallnetworkservices':
                return _Proc(stdout='Wi-Fi\nUSB 10/100 LAN\n')
            if flag == '-getwebproxy':
                return _Proc(stdout=_audit_proxy_line(webproxy_on, port))
            if flag.startswith('-get'):
                return _Proc(stdout=_audit_proxy_line(False))
            return _Proc(stdout='')  # set* 成功
        raise AssertionError(f'unexpected cmd {cmd}')
    return runner


def test_restore_network_with_ledger_replays_and_persists(tmp_path, monkeypatch):
    """端到端：有未结账本 → 停内核 + 按 before 回放 + 结账 + 意图落 systemProxy=false。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _networksetup_full(webproxy_on=True, port=7890))
    server.ledger_begin('mcp:start_proxy')
    server.ledger_record_killed(['FlClash (PID 9)'])
    stop_calls = []
    monkeypatch.setattr(server, 'stop_mihomo', lambda: stop_calls.append(1))
    written = {}
    monkeypatch.setattr(server, 'read_config', lambda: {'systemProxy': True, 'nodes': []})
    monkeypatch.setattr(server, 'write_config', lambda cfg: written.update(cfg))

    r = server.call_tool('restore_network', {})
    assert r['ok'] is True and r['restored'] is True
    assert stop_calls == [1], '还原必须停内核'
    assert server.ledger_open_session() is None, '还原后结账'
    assert written.get('systemProxy') is False, '意图落盘防 App 启动联动覆盖'
    assert 'FlClash' in ' '.join(r['irreversible']) and '不会自动复活' in r['message']


def test_restore_network_without_ledger_is_honest(tmp_path, monkeypatch):
    """无账本：不瞎写，退回关系统代理回直连并如实说明，restored=False。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _networksetup_full(webproxy_on=False, port=0))
    monkeypatch.setattr(server, 'stop_mihomo', lambda: None)
    monkeypatch.setattr(server, 'read_config', lambda: {'systemProxy': True})
    monkeypatch.setattr(server, 'write_config', lambda cfg: None)
    r = server.call_tool('restore_network', {})
    assert r['ok'] is True and r['restored'] is False
    assert '无未结接管账本' in r['message'] and '手动恢复' in r['message']


def test_reapply_takeover_guardrails(tmp_path, monkeypatch):
    """归位双检：内核未跑 → 拒；有账但内核在跑 → 设回并对账。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _networksetup_full())
    monkeypatch.setattr(server, 'stop_mihomo', lambda: None)
    server.ledger_begin('mcp:start_proxy')
    # 内核没跑 → 拒绝（绝不把系统代理指向死端口）
    monkeypatch.setattr(server, 'mihomo_running', lambda: False)
    r = server.call_tool('reapply_takeover', {})
    assert r['ok'] is False and '未在运行' in r['error']
    # 内核在跑 → 走 set_system_proxy(True) 归位
    monkeypatch.setattr(server, 'mihomo_running', lambda: True)
    monkeypatch.setattr(server, 'set_system_proxy',
                        lambda enable=True, port=7891: {'allOk': True, 'mismatched': [], 'services': []})
    monkeypatch.setattr(server, 'system_proxy_enabled', lambda: True)
    r = server.call_tool('reapply_takeover', {})
    assert r['ok'] is True and r['allOk'] is True


def test_probe_drift_only_when_taking_over(tmp_path, monkeypatch):
    """巡检守卫：无账本 / 内核未跑 一律 None；接管中读回不达标才报漂移。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    # 无账本 → None（不放大开销）
    assert server.probe_drift() is None
    monkeypatch.setattr(server.subprocess, 'run', _networksetup_full(webproxy_on=True, port=7890))
    server.ledger_begin('mcp:start_proxy')
    monkeypatch.setattr(server, 'mihomo_running', lambda: False)
    assert server.probe_drift() is None, '内核未跑交给崩溃看门狗，巡检不抢处置权'
    # 接管中 + 读回指向 7890（≠ 接管端口 7891）→ 全服务不达标 → 报漂移
    monkeypatch.setattr(server, 'mihomo_running', lambda: True)
    drift = server.probe_drift()
    assert drift and 'Wi-Fi' in drift


def test_status_exposes_drift(tmp_path, monkeypatch):
    """status 联动：接管中被外部改动 → 返回 drift.services 供 AI 处置。"""
    path = str(tmp_path / 'ledger.json')
    monkeypatch.setattr(server, 'LEDGER_PATH', path)
    monkeypatch.setattr(server.subprocess, 'run', _networksetup_full(webproxy_on=True, port=7890))
    server.ledger_begin('mcp:start_proxy')
    monkeypatch.setattr(server, 'mihomo_running', lambda: True)
    monkeypatch.setattr(server, 'system_proxy_enabled', lambda: True)
    r = server.call_tool('status', {})
    assert r.get('drift') and 'Wi-Fi' in r['drift']['services']
    assert r.get('openLedger') is True


def test_p14_tools_registered():
    """双引擎一致性红线：restore_network / reapply_takeover 必须注册且有 schema。"""
    names = [t['name'] for t in server.TOOLS]
    assert 'restore_network' in names and 'reapply_takeover' in names
    assert server._TOOL_SCHEMAS['restore_network'] == server._tool_schema()
    assert server._TOOL_SCHEMAS['reapply_takeover'] == server._tool_schema()
