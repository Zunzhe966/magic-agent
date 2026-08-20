'use strict';

const $ = sel => document.querySelector(sel);
const $$ = sel => Array.from(document.querySelectorAll(sel));

const state = {
  boot: null,
  settings: null,
  proxy: null,
  connections: [],
  scanResult: [],
  currentTermId: null,
  termBuf: ''
};

const escapeHtml = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

async function refreshAll() {
  state.boot = await window.magic.getBoot();
  state.settings = state.boot.settings;
  state.proxy = state.boot.proxy;
  render();
}

function render() {
  renderNav();
  renderEnginePill();
  renderHero();
  renderExit();
  renderAppSummary();
  renderApps();
  renderServers();
  renderNodes();
  renderLogs();
}

function renderNav() {
  $$('.nav-item').forEach(b => b.classList.toggle('active', b.dataset.view === state.view));
  $('#page-title').textContent = ({ overview: '总览', apps: '软件分流', servers: '云服务器', nodes: '代理节点', logs: '实时日志' })[state.view || 'overview'];
  $$('.view').forEach(v => v.classList.toggle('hidden', v.id !== 'view-' + (state.view || 'overview')));
}

function renderEnginePill() {
  const pill = $('#engine-pill');
  const on = state.proxy?.running;
  pill.classList.toggle('on', !!on);
  pill.classList.toggle('off', !on);
  pill.textContent = on ? `运行中 PID ${state.proxy.pid}` : '内核未启动';
}

function renderHero() {
  const running = !!state.proxy?.running;
  $('#btn-proxy-toggle').textContent = running ? '停止代理' : '启动代理';
  $('#btn-proxy-toggle').classList.toggle('primary', !running);
  $('#btn-proxy-toggle').classList.toggle('danger', running);
  const totalRx = state.connections.reduce((s, c) => s + (c.download || 0), 0);
  const totalTx = state.connections.reduce((s, c) => s + (c.upload || 0), 0);
  $('#metric-rx').textContent = (totalRx / 1048576).toFixed(1);
  $('#metric-tx').textContent = (totalTx / 1048576).toFixed(1);
  $('#metric-conn').textContent = state.connections.length;
  $('#exit-tag').textContent = state.proxy?.selected || '未选择';
}

function renderExit() {
  const list = $('#exit-list');
  const names = []; 
  (state.settings?.proxies || []).forEach(p => names.push({ name: p.name, kind: 'proxy' }));
  names.push({ name: 'DIRECT', kind: 'direct' });
  list.innerHTML = names.map(n => {
    const active = state.proxy?.selected === n.name;
    return `<div class="exit-row ${active ? 'active' : ''}" data-exit="${escapeHtml(n.name)}">
      <span class="exit-name">${escapeHtml(n.name)}</span><span class="tag">${n.kind === 'direct' ? '本地直连' : '云端出口'}</span></div>`;
  }).join('');
  list.querySelectorAll('.exit-row').forEach(row => row.onclick = async () => {
    await window.magic.proxySelect(row.dataset.exit);
    await refreshAll();
  });
}

function renderAppSummary() {
  const apps = state.settings?.apps || [];
  const proxyCount = apps.filter(a => a.enabled !== false && a.policy === 'proxy').length;
  const directCount = apps.filter(a => a.enabled !== false && a.policy === 'direct').length;
  $('#app-summary').innerHTML = `
    <div class="sum-row"><span>走云代理</span><span class="proxy">${proxyCount} 个软件</span></div>
    <div class="sum-row"><span>本机直连</span><span class="direct">${directCount} 个软件</span></div>
    <div class="sum-row"><span>未匹配流量</span><span>按代理规则智能处理</span></div>`;
}

function renderApps() {
  const table = $('#app-table');
  const q = ($('#filter-apps')?.value || '').toLowerCase();
  const apps = (state.settings?.apps || []).filter(a => !q || (a.name + a.regex).toLowerCase().includes(q));
  if (!apps.length) {
    table.innerHTML = '<div class="sum-row">还没有软件规则，点击“新增软件”添加</div>';
    return;
  }
  table.innerHTML = apps.map(a => `
    <div class="app-row" data-app="${escapeHtml(a.id)}">
      <div class="app-ico">${escapeHtml(a.icon || a.name[0] || '?')}</div>
      <div>
        <div class="app-name">${escapeHtml(a.name)}</div>
        <div class="app-regex" title="${escapeHtml(a.regex)}">${escapeHtml(a.regex)}</div>
      </div>
      <select class="policy-select ${a.policy || 'direct'}" data-field="policy">
        <option value="direct" ${a.policy !== 'proxy' ? 'selected' : ''}>本机直连</option>
        <option value="proxy" ${a.policy === 'proxy' ? 'selected' : ''}>云代理</option>
      </select>
      <label style="display:flex;align-items:center;gap:6px;font-size:12px"><input type="checkbox" data-field="enabled" ${a.enabled !== false ? 'checked' : ''}>启用</label>
    </div>`).join('');
  table.querySelectorAll('[data-app]').forEach(row => {
    const id = row.dataset.app;
    row.querySelectorAll('[data-field]').forEach(el => {
      el.onchange = async () => {
        const val = el.type === 'checkbox' ? el.checked : el.value;
        state.settings = await window.magic.settingsUpdate({ apps: state.settings.apps.map(a => a.id === id ? { ...a, [el.dataset.field]: val } : a) });
        renderApps();
        renderAppSummary();
        if (state.proxy?.running) window.magic.proxyRestart();
      };
    });
  });
}

function renderServers() {
  const grid = $('#server-grid');
  const servers = state.settings?.servers || [];
  if (!servers.length) {
    grid.innerHTML = '<div class="panel" style="grid-column:1/-1">还没有服务器，点“添加服务器”开始</div>';
    return;
  }
  grid.innerHTML = servers.map(s => `
    <div class="server-card">
      <h4>${escapeHtml(s.name || s.host)}</h4>
      <div class="server-meta">${escapeHtml(s.username)}@${escapeHtml(s.host)}:${s.port || 22}<br>${escapeHtml(s.auth === 'key' ? '密钥登录' : '密码登录')}</div>
      <div class="server-actions">
        <button data-act="terminal" data-id="${escapeHtml(s.id)}">终端</button>
        <button data-act="info" data-id="${escapeHtml(s.id)}">状态</button>
        <button data-act="reboot" data-id="${escapeHtml(s.id)}">重启</button>
        <button data-act="poweroff" data-id="${escapeHtml(s.id)}">关机</button>
        <button data-act="del" data-id="${escapeHtml(s.id)}">删除</button>
      </div>
    </div>`).join('');
  grid.querySelectorAll('[data-act]').forEach(b => {
    b.onclick = async () => {
      const id = b.dataset.id;
      const server = servers.find(x => x.id === id);
      if (b.dataset.act === 'del') {
        state.settings = await window.magic.settingsUpdate({ servers: servers.filter(x => x.id !== id) });
        renderServers();
      } else if (b.dataset.act === 'terminal') {
        state.currentTermId = id;
        $('#terminal-panel').classList.remove('hidden');
        $('#terminal').textContent = state.termBuf = '';
        $('#term-prompt').textContent = `${server.username}@${server.host}`;
        const r = await window.magic.sshConnect(server);
        if (!r.ok) appendTerminal(`连接失败: ${r.error}\r\n`);
      } else if (b.dataset.act === 'info') {
        await ensureConn(server);
        const r = await window.magic.sshInfo(id);
        showInfoModal(server, r);
      } else if (b.dataset.act === 'reboot') {
        await ensureConn(server);
        const r = await window.magic.sshReboot(id);
        alertModal('重启', r.stdout || r.error || '已发送');
      } else if (b.dataset.act === 'poweroff') {
        await ensureConn(server);
        const r = await window.magic.sshPoweroff(id);
        alertModal('关机', r.stdout || r.error || '已发送');
      }
    };
  });
}

async function ensureConn(server) {
  const r = await window.magic.sshConnect(server);
  if (!r.ok) alertModal('连接失败', r.error);
  return r;
}

function appendTerminal(text) {
  state.termBuf += text;
  if (state.termBuf.length > 60000) state.termBuf = state.termBuf.slice(-60000);
  const box = $('#terminal');
  box.textContent = state.termBuf;
  box.scrollTop = box.scrollHeight;
}

function showInfoModal(server, r) {
  const text = r.ok ? r.stdout : (r.error || '无输出');
  const lines = text.split('\n').map(l => l.replace(/^===\s*INFO===.*$/g, ''));
  const kv = {};
  lines.forEach(l => {
    const m = l.match(/^([^=]+)=(.+)$/);
    if (m) kv[m[1].trim()] = m[2].trim();
  });
  const body = document.getElementById('modal-body');
  const title = document.getElementById('modal-title');
  title.textContent = `${server.name} 状态`;
  body.innerHTML = `
    <div class="sum-row"><span>主机名</span><span>${escapeHtml(kv.hostname || 'n/a')}</span></div>
    <div class="sum-row"><span>运行时间</span><span>${escapeHtml(kv.uptime || 'n/a')}</span></div>
    <div class="sum-row"><span>内存</span><span>${escapeHtml(kv.mem || 'n/a')}</span></div>
    <div class="sum-row"><span>负载</span><span>${escapeHtml(kv.load || 'n/a')}</span></div>
    <div class="sum-row"><span>磁盘剩余</span><span>${escapeHtml(kv.disk || 'n/a')}</span></div>
    <div class="sum-row"><span>内核</span><span>${escapeHtml(kv.kernel || 'n/a')}</span></div>`;
  document.getElementById('modal-ok').classList.add('hidden');
  document.getElementById('modal-cancel').textContent = '关闭';
  document.getElementById('modal').classList.remove('hidden');
}

function alertModal(title, text) {
  document.getElementById('modal-title').textContent = title;
  document.getElementById('modal-body').innerHTML = `<div class="log-box" style="height:auto;max-height:280px">${escapeHtml(text)}</div>`;
  document.getElementById('modal-ok').classList.add('hidden');
  document.getElementById('modal-cancel').textContent = '关闭';
  document.getElementById('modal').classList.remove('hidden');
}

function renderNodes() {
  const list = $('#node-list');
  const proxies = state.settings?.proxies || [];
  list.innerHTML = proxies.map(p => `
    <div class="node-row">
      <div>
        <div class="exit-name">${escapeHtml(p.name)}</div>
        <div class="node-meta">${escapeHtml(p.type.toUpperCase())} · ${escapeHtml(p.server)}:${p.port} · ${escapeHtml(p.tls ? 'TLS' : '无TLS')}</div>
      </div>
      <div class="server-actions">
        <button data-node-select="${escapeHtml(p.name)}">设为出口</button>
        <button data-node-del="${escapeHtml(p.name)}">删除</button>
      </div>
    </div>`).join('') || '<div class="sum-row">暂无节点</div>';
  list.querySelectorAll('[data-node-select]').forEach(b => b.onclick = async () => {
    await window.magic.proxySelect(b.dataset.nodeSelect);
    await refreshAll();
  });
  list.querySelectorAll('[data-node-del]').forEach(b => b.onclick = async () => {
    state.settings = await window.magic.settingsUpdate({ proxies: proxies.filter(p => p.name !== b.dataset.nodeDel) });
    renderNodes();
    renderExit();
    if (state.proxy?.running) window.magic.proxyRestart();
  });
}

function renderLogs() {
  const box = $('#log-box');
  if (!state.logs) return;
  box.innerHTML = state.logs.map(l => {
    const level = l.match(/\[(info|warn|error)\]/i)?.[1]?.toLowerCase() || 'info';
    return `<div class="log-line ${level}">${escapeHtml(l)}</div>`;
  }).join('');
}

function openModal({ title, fields, onSubmit }) {
  const modal = $('#modal');
  $('#modal-title').textContent = title;
  $('#modal-ok').classList.remove('hidden');
  $('#modal-cancel').textContent = '取消';
  $('#modal-body').innerHTML = fields.map(f => {
    const value = escapeHtml(f.value ?? '');
    if (f.type === 'select') {
      const opts = (f.options || []).map(o => `<option value="${escapeHtml(o.value)}" ${o.value === f.value ? 'selected' : ''}>${escapeHtml(o.label)}</option>`).join('');
      return `<label>${escapeHtml(f.label)}<select class="input" data-field="${f.key}">${opts}</select></label>`;
    }
    return `<label>${escapeHtml(f.label)}<input class="input" data-field="${f.key}" value="${value}" placeholder="${escapeHtml(f.placeholder || '')}"></label>`;
  }).join('');
  modal.classList.remove('hidden');
  const ok = () => {
    const out = {};
    $$('#modal-body [data-field]').forEach(el => out[el.dataset.field] = el.value);
    modal.classList.add('hidden');
    $('#modal-ok').onclick = null;
    $('#modal-cancel').onclick = null;
    onSubmit(out);
  };
  $('#modal-ok').onclick = ok;
  $('#modal-cancel').onclick = () => {
    modal.classList.add('hidden');
    $('#modal-ok').onclick = null;
    $('#modal-cancel').onclick = null;
  };
}

async function startConnectionsPoll() {
  setInterval(async () => {
    if (state.proxy?.running) {
      state.connections = await window.magic.proxyConnections();
      renderHero();
    }
  }, 2500);
}

function wire() {
  $$('.nav-item').forEach(b => b.onclick = () => { state.view = b.dataset.view; renderNav(); });
  document.querySelectorAll('[data-go]').forEach(b => b.onclick = () => { state.view = b.dataset.go; renderNav(); });

  $('#btn-proxy-toggle').onclick = async () => {
    if (state.proxy?.running) {
      const r = await window.magic.proxyStop();
      state.proxy = r.proxy;
    } else {
      const r = await window.magic.proxyStart();
      state.proxy = r.proxy;
    }
    await refreshAll();
    if (state.proxy?.running) loadLogs();
  };

  $('#btn-open-connections').onclick = () => {
    state.view = 'logs'; renderNav();
  };

  $('#btn-scan').onclick = async () => {
    state.scanResult = await window.magic.appsScan();
    alertModal('扫描完成', state.scanResult.map(a => `${a.name}  ${a.path}`).join('\n'));
  };

  $('#btn-open-log').onclick = async () => {
    const p = await window.magic.proxyStatus();
    if (p?.logPath) window.magic.shellOpenPath(p.logPath);
  };

  $('#btn-clear-log').onclick = () => { state.logs = []; renderLogs(); };

  $('#filter-apps').oninput = () => renderApps();

  $('#btn-add-app').onclick = () => openModal({
    title: '新增软件分流规则',
    fields: [
      { key: 'name', label: '软件名称', value: '' },
      { key: 'regex', label: '进程匹配规则（正则，例：^Telegram$）', value: '' },
      { key: 'policy', label: '走代理策略', value: 'direct', type: 'select', options: [{ value: 'direct', label: '本机直连' }, { value: 'proxy', label: '云代理' }] }
    ],
    onSubmit: async v => {
      if (!v.name || !v.regex) return;
      state.settings = await window.magic.settingsUpdate({
        apps: [...(state.settings.apps || []), { id: 'app-' + Date.now(), name: v.name, icon: v.name[0], policy: v.policy, enabled: true, regex: v.regex }]
      });
      renderApps(); renderAppSummary();
      if (state.proxy?.running) window.magic.proxyRestart();
    }
  });

  $('#btn-add-node').onclick = () => openModal({
    title: '添加代理节点（VLESS+Reality）',
    fields: [
      { key: 'name', label: '节点名称', value: '' },
      { key: 'server', label: '服务器地址', value: '' },
      { key: 'port', label: '端口', value: '443' },
      { key: 'uuid', label: 'UUID', value: '' },
      { key: 'flow', label: 'Flow', value: 'xtls-rprx-vision' },
      { key: 'publicKey', label: 'Reality Public Key', value: '' },
      { key: 'shortId', label: 'Reality Short ID', value: '' }
    ],
    onSubmit: async v => {
      if (!v.name || !v.server || !v.uuid) return;
      const node = {
        name: v.name, type: 'vless', server: v.server, port: Number(v.port) || 443,
        uuid: v.uuid, network: 'tcp', tls: true, udp: true, flow: v.flow,
        clientFingerprint: 'chrome', servername: '', realityOpts: { publicKey: v.publicKey, shortId: v.shortId }
      };
      state.settings = await window.magic.settingsUpdate({ proxies: [...(state.settings.proxies || []), node] });
      renderNodes(); renderExit();
      if (state.proxy?.running) window.magic.proxyRestart();
    }
  });

  $('#btn-add-server').onclick = () => openModal({
    title: '添加云服务器',
    fields: [
      { key: 'name', label: '名称', value: '' },
      { key: 'host', label: '主机地址', value: '' },
      { key: 'port', label: 'SSH 端口', value: '22' },
      { key: 'username', label: '用户名', value: 'root' },
      { key: 'auth', label: '登录方式', value: 'password', type: 'select', options: [{ value: 'password', label: '密码' }, { value: 'key', label: '私钥' }] },
      { key: 'password', label: '密码', value: '' },
      { key: 'privateKeyPath', label: '私钥路径', value: '~/.ssh/id_ed25519' }
    ],
    onSubmit: async v => {
      if (!v.host || !v.username) return;
      const server = {
        id: 'srv-' + Date.now(), name: v.name || v.host, host: v.host, port: Number(v.port) || 22,
        username: v.username, auth: v.auth, password: v.password,
        privateKeyPath: v.privateKeyPath === '~/.ssh/id_ed25519' ? v.privateKeyPath : v.privateKeyPath
      };
      state.settings = await window.magic.settingsUpdate({ servers: [...(state.settings.servers || []), server] });
      renderServers();
    }
  });

  $('#btn-close-terminal').onclick = () => {
    if (state.currentTermId) window.magic.sshDisconnect(state.currentTermId);
    $('#terminal-panel').classList.add('hidden');
  };

  $('#term-input').onkeydown = async e => {
    if (e.key !== 'Enter' || !state.currentTermId) return;
    const v = $('#term-input').value;
    $('#term-input').value = '';
    appendTerminal(`${$('#term-prompt').textContent} $ ${v}\r\n`);
    const r = await window.magic.sshRun(state.currentTermId, v);
    appendTerminal(r.ok ? (r.stdout || '') + (r.stderr || '') : '错误: ' + r.error + '\r\n');
    appendTerminal('\r\n');
  };

  $('#modal-close').onclick = () => $('#modal').classList.add('hidden');
  $('#modal-cancel').onclick = () => $('#modal').classList.add('hidden');

  window.magic.onSshData(d => {
    if (d.type === 'ssh:output' && d.id === state.currentTermId) appendTerminal(d.text);
  });
}

async function loadLogs() {
  const logs = await window.magic.proxyLog();
  state.logs = logs;
  renderLogs();
}

window.addEventListener('DOMContentLoaded', async () => {
  state.view = 'overview';
  wire();
  await refreshAll();
  startConnectionsPoll();
  if (state.proxy?.running) loadLogs();
  setInterval(async () => {
    if (state.proxy?.running) {
      const logs = await window.magic.proxyLog();
      if (JSON.stringify(logs) !== JSON.stringify(state.logs)) { state.logs = logs; renderLogs(); }
    }
  }, 4000);
});
