import './index.css';

const api = window.magicAPI;
let state = null;
let activeView = 'overview';

const app = document.getElementById('app');
app.innerHTML = `
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-icon">魔</div>
        <div>
          <div class="brand-name">魔法代理</div>
          <div class="brand-sub">Magic Proxy</div>
        </div>
      </div>
      <nav class="nav">
        <button class="nav-item active" data-view="overview"><span class="nav-ico">◇</span>总览</button>
        <button class="nav-item" data-view="apps"><span class="nav-ico">◈</span>软件分流</button>
        <button class="nav-item" data-view="nodes"><span class="nav-ico">◎</span>云服务器</button>
        <button class="nav-item" data-view="console"><span class="nav-ico">▣</span>控制台</button>
      </nav>
      <div class="sidebar-footer">本机代理 · 按软件精确分流<br/>内核 mihomo v1.19.30</div>
    </aside>
    <main class="main">
      <header class="topbar">
        <h1 id="page-title">总览</h1>
        <div class="topbar-actions">
          <span class="status-pill" id="proxy-status"><span class="dot"></span><span id="proxy-status-text">代理未启动</span></span>
          <button class="btn primary" id="btn-toggle-proxy">启动代理</button>
        </div>
      </header>
      <div class="page" id="page-content"></div>
    </main>
  </div>
  <div id="modal-root"></div>
`;

const pageTitles = {
  overview: '总览',
  apps: '软件分流',
  nodes: '云服务器',
  console: '控制台'
};

function el(html) {
  const t = document.createElement('template');
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

function setStatus() {
  const on = !!(state && state.proxy && state.proxy.enabled);
  const pill = document.getElementById('proxy-status');
  const txt = document.getElementById('proxy-status-text');
  pill.classList.toggle('on', on);
  txt.textContent = on ? '代理已启动' : '代理未启动';
  document.getElementById('btn-toggle-proxy').textContent = on ? '停止代理' : '启动代理';
}

function renderShell() {
  const title = pageTitles[activeView] || '总览';
  document.getElementById('page-title').textContent = title;
  document.querySelectorAll('.nav-item').forEach((n) => n.classList.toggle('active', n.dataset.view === activeView));
  const page = document.getElementById('page-content');
  if (activeView === 'overview') page.innerHTML = viewOverview();
  else if (activeView === 'apps') page.innerHTML = viewApps();
  else if (activeView === 'nodes') page.innerHTML = viewNodes();
  else if (activeView === 'console') page.innerHTML = viewConsole();
  bindPage();
  setStatus();
}

function viewOverview() {
  if (!state) return '<div class="empty">正在加载…</div>';
  const proxy = state.proxy || {};
  const nodes = proxy.nodes || [];
  const servers = state.servers || [];
  const runningApps = state.runningApps || [];
  const logs = (proxy.logs || []).slice(-6).reverse();
  const on = !!proxy.enabled;
  return `
    <div class="grid cols-3">
      <div class="stat"><div class="stat-label">代理状态</div><div class="stat-value ${on ? 'good' : 'warn'}">${on ? '运行中' : '已停止'}</div></div>
      <div class="stat"><div class="stat-label">分流模式</div><div class="stat-value">${proxy.mode === 'global' ? '全局' : '智能分流'}</div></div>
      <div class="stat"><div class="stat-label">节点数量</div><div class="stat-value">${nodes.length}</div></div>
      <div class="stat"><div class="stat-label">正在运行软件</div><div class="stat-value">${runningApps.length}</div></div>
      <div class="stat"><div class="stat-label">云服务器</div><div class="stat-value">${servers.length}</div></div>
      <div class="stat"><div class="stat-label">本地监听</div><div class="stat-value">7897</div></div>
    </div>
    <div class="card" style="margin-top:18px">
      <h2>分流策略 <span class="hint">按软件而非按域名</span></h2>
      <div class="seg">
        <button data-mode="auto" class="${proxy.mode !== 'global' ? 'active' : ''}">智能分流</button>
        <button data-mode="global" class="${proxy.mode === 'global' ? 'active' : ''}">全局代理</button>
      </div>
    </div>
    <div class="card">
      <h2>软件流量分类 <span class="hint">当前运行</span></h2>
      ${runningApps.length ? `
      <table class="table">
        <thead><tr><th>软件</th><th>类型</th><th>分组</th><th>策略</th></tr></thead>
        <tbody>
          ${runningApps.map((a) => `
            <tr>
              <td><b>${a.name}</b></td>
              <td><span class="badge blue">${a.kind}</span></td>
              <td>${a.group}</td>
              <td><span class="badge ${a.policy === '代理' ? 'purple' : 'green'}">${a.policy}</span></td>
            </tr>`).join('')}
        </tbody>
      </table>` : '<div class="empty">没有检测到已知软件</div>'}
    </div>
    <div class="card">
      <h2>运行日志 <span class="hint">最近 6 条</span></h2>
      <div class="log-box">${logs.length ? logs.map((l) => `<div class="log-line"><span class="log-time">${l.time || ''}</span><span class="log-level ${l.level || 'info'}">${l.level || 'info'}</span><span>${escapeHtml(l.text || '')}</span></div>`).join('') : '<div class="muted">暂无日志</div>'}</div>
    </div>`;
}

function viewApps() {
  if (!state) return '<div class="empty">正在加载…</div>';
  const apps = state.appProfiles || [];
  const running = new Set((state.runningApps || []).map((a) => a.key));
  return `
    <div class="notice">规则按进程匹配：Chrome 走代理、Safari/微信/QQ 直连，已按你本机正在运行的软件整理。</div>
    <div class="card">
      <h2>软件分流规则 <span class="hint">${apps.length} 个应用</span></h2>
      <table class="table">
        <thead><tr><th>软件</th><th>类型</th><th>状态</th><th>分组</th><th>策略</th></tr></thead>
        <tbody>
          ${apps.map((a) => `
            <tr>
              <td><b>${a.name}</b> ${running.has(a.key) ? '<span class="badge green">运行中</span>' : ''}</td>
              <td><span class="badge blue">${a.kind}</span></td>
              <td>${a.policy}</td>
              <td><input class="input" style="width:110px" value="${escapeAttr(a.group)}" data-group="${escapeAttr(a.key)}" /></td>
              <td>
                <div class="seg">
                  <button data-policy="${escapeAttr(a.key)}" data-val="代理" class="${a.policy === '代理' ? 'active' : ''}">代理</button>
                  <button data-policy="${escapeAttr(a.key)}" data-val="直连" class="${a.policy === '直连' ? 'active' : ''}">直连</button>
                </div>
              </td>
            </tr>`).join('')}
        </tbody>
      </table>
    </div>`;
}

function viewNodes() {
  if (!state) return '<div class="empty">正在加载…</div>';
  const nodes = (state.proxy && state.proxy.nodes) || [];
  return `
    <div class="card">
      <h2>代理节点 <span class="hint">VLESS + Reality</span></h2>
      <div class="grid cols-2">
        ${nodes.map((n, i) => `
          <div class="node-card">
            <div class="name">${escapeHtml(n.name)}</div>
            <div class="meta">${escapeHtml(n.type)} · ${escapeHtml(n.server)}:${n.port} · ${escapeHtml(n.uuid || '')}</div>
            <div class="meta" style="margin-top:8px"><span class="tag">${escapeHtml(n.flow || '')}</span><span class="tag">${escapeHtml(n.clientFingerprint || 'chrome')}</span></div>
            <div style="margin-top:10px"><button class="btn small danger" data-remove-node="${i}">删除节点</button></div>
          </div>`).join('')}
        ${nodes.length === 0 ? '<div class="empty">还没有节点</div>' : ''}
      </div>
      <div class="section-title">添加节点</div>
      <div class="grid cols-4">
        <div class="form-row"><label>名称</label><input id="node-name" class="input" placeholder="例如 东京一区" /></div>
        <div class="form-row"><label>服务器</label><input id="node-server" class="input" placeholder="1.2.3.4" /></div>
        <div class="form-row"><label>端口</label><input id="node-port" class="input" value="443" /></div>
        <div class="form-row"><label>UUID</label><input id="node-uuid" class="input" /></div>
      </div>
      <div class="grid cols-3">
        <div class="form-row"><label>Flow</label><input id="node-flow" class="input" value="xtls-rprx-vision" /></div>
        <div class="form-row"><label>Public Key</label><input id="node-pk" class="input" /></div>
        <div class="form-row"><label>Short ID</label><input id="node-sid" class="input" /></div>
      </div>
      <button class="btn primary" id="btn-add-node">添加节点</button>
      <button class="btn" id="btn-import-node" style="margin-left:8px">从配置粘贴</button>
    </div>`;
}

function viewConsole() {
  if (!state) return '<div class="empty">正在加载…</div>';
  const servers = state.servers || [];
  return `
    <div class="card">
      <h2>云服务器 <span class="hint">SSH 直连控制</span></h2>
      ${servers.length ? `
      <table class="table">
        <thead><tr><th>名称</th><th>地址</th><th>用户</th><th>状态</th><th>操作</th></tr></thead>
        <tbody>
          ${servers.map((s) => `
            <tr>
              <td><b>${escapeHtml(s.name)}</b></td>
              <td>${escapeHtml(s.host)}:${s.port || 22}</td>
              <td>${escapeHtml(s.username)}</td>
              <td><span class="badge ${s.connected ? 'green' : 'gray'}">${s.connected ? '已连接' : '未连接'}</span></td>
              <td style="white-space:nowrap">
                <button class="btn small" data-connect="${escapeAttr(s.id)}">${s.connected ? '断开' : '连接'}</button>
                <button class="btn small danger" data-remove-server="${escapeAttr(s.id)}">删除</button>
              </td>
            </tr>`).join('')}
        </tbody>
      </table>` : '<div class="empty">还没有云服务器，先添加一台</div>'}
      <div class="section-title">添加云服务器</div>
      <div class="grid cols-4">
        <div class="form-row"><label>名称</label><input id="srv-name" class="input" placeholder="搬瓦工" /></div>
        <div class="form-row"><label>地址</label><input id="srv-host" class="input" placeholder="104.160.40.35" /></div>
        <div class="form-row"><label>端口</label><input id="srv-port" class="input" value="22" /></div>
        <div class="form-row"><label>用户名</label><input id="srv-user" class="input" placeholder="root" /></div>
      </div>
      <div class="form-row"><label>SSH 私钥路径（可选）</label><input id="srv-key" class="input" placeholder="/Users/you/.ssh/id_ed25519" /></div>
      <button class="btn primary" id="btn-add-server">添加服务器</button>
    </div>
    <div class="card">
      <h2>远程终端 <span class="hint">输出预览</span></h2>
      <div class="terminal" id="terminal-box"><div class="muted">连接服务器后，这里显示远程输出。</div></div>
    </div>`;
}

function bindPage() {
  document.querySelectorAll('.nav-item').forEach((b) => b.addEventListener('click', () => {
    activeView = b.dataset.view;
    renderShell();
  }));
  const toggle = document.getElementById('btn-toggle-proxy');
  if (toggle) toggle.addEventListener('click', async () => {
    const on = state && state.proxy && state.proxy.enabled;
    toggle.disabled = true;
    state.proxy = on ? await api.stopProxy() : await api.startProxy();
    toggle.disabled = false;
    renderShell();
  });
  document.querySelectorAll('[data-mode]').forEach((b) => b.addEventListener('click', async () => {
    const mode = b.dataset.mode;
    const resp = await api.setProxyMode(mode);
    if (resp) state.proxy = resp;
    renderShell();
  }));
  document.querySelectorAll('[data-policy]').forEach((b) => b.addEventListener('click', async () => {
    const appKey = b.dataset.policy;
    const policy = b.dataset.val;
    const resp = await api.setAppPolicy(appKey, policy);
    if (resp && resp.profiles) state.appProfiles = resp.profiles;
    if (resp && resp.proxy) state.proxy = resp.proxy;
    renderShell();
  }));
  document.querySelectorAll('[data-group]').forEach((input) => input.addEventListener('change', async () => {
    const appKey = input.dataset.group;
    const resp = await api.setAppGroup(appKey, input.value.trim() || '自定义');
    if (resp && resp.profiles) state.appProfiles = resp.profiles;
  }));
  document.querySelectorAll('[data-remove-node]').forEach((b) => b.addEventListener('click', async () => {
    const resp = await api.removeNode(Number(b.dataset.removeNode));
    if (resp) state.proxy = resp;
    renderShell();
  }));
  const addNode = document.getElementById('btn-add-node');
  if (addNode) addNode.addEventListener('click', async () => {
    const node = {
      name: val('node-name') || '新节点',
      type: 'vless',
      server: val('node-server'),
      port: Number(val('node-port') || 443),
      uuid: val('node-uuid'),
      flow: val('node-flow') || 'xtls-rprx-vision',
      publicKey: val('node-pk'),
      shortId: val('node-sid'),
      clientFingerprint: 'chrome',
      tls: true,
      udp: true
    };
    if (!node.server || !node.uuid) { alert('请填写服务器地址和 UUID'); return; }
    const resp = await api.addNode(node);
    if (resp) state.proxy = resp;
    renderShell();
  });
  const importNode = document.getElementById('btn-import-node');
  if (importNode) importNode.addEventListener('click', () => openImportModal());
  document.querySelectorAll('[data-connect]').forEach((b) => b.addEventListener('click', async () => {
    const id = b.dataset.connect;
    const s = (state.servers || []).find((x) => x.id === id);
    if (s && s.connected) await api.disconnectServer(id);
    else await api.connectServer(id);
    await refresh();
    if (state.servers.find((x) => x.id === id)?.connected) {
      api.setServerConfig({ onEvent: ({ type, data }) => {
        const box = document.getElementById('terminal-box');
        if (box) {
          const div = document.createElement('div');
          div.textContent = `[${type}] ${data}`;
          box.appendChild(div);
          box.scrollTop = box.scrollHeight;
        }
      } });
    }
  }));
  document.querySelectorAll('[data-remove-server]').forEach((b) => b.addEventListener('click', async () => {
    await api.removeServer(b.dataset.removeServer);
    await refresh();
  }));
  const addServer = document.getElementById('btn-add-server');
  if (addServer) addServer.addEventListener('click', async () => {
    const server = {
      name: val('srv-name') || '新服务器',
      host: val('srv-host'),
      port: Number(val('srv-port') || 22),
      username: val('srv-user') || 'root',
      privateKey: val('srv-key')
    };
    if (!server.host) { alert('请填写服务器地址'); return; }
    await api.addServer(server);
    await refresh();
  });
}

function openImportModal() {
  const root = document.getElementById('modal-root');
  root.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal">
        <h3>粘贴代理配置 JSON</h3>
        <div class="form-row"><label>配置内容（{"nodes":[...]} 或 Clash YAML）</label><textarea class="textarea" id="import-text" placeholder='{"nodes":[{"name":"节点","type":"vless","server":"1.2.3.4","port":443,"uuid":"...","publicKey":"...","shortId":"..."}]}'></textarea></div>
        <div class="modal-actions">
          <button class="btn" id="modal-cancel">取消</button>
          <button class="btn primary" id="modal-ok">导入</button>
        </div>
      </div>
    </div>`;
  document.getElementById('modal-cancel').addEventListener('click', () => root.innerHTML = '');
  document.getElementById('modal-ok').addEventListener('click', async () => {
    const text = document.getElementById('import-text').value;
    await api.importConfig(text);
    root.innerHTML = '';
    await refresh();
  });
}

function val(id) {
  const elm = document.getElementById(id);
  return elm ? elm.value.trim() : '';
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
function escapeAttr(s) {
  return escapeHtml(s);
}

async function refresh() {
  state = await api.getState();
  renderShell();
}

api.onLog(() => { if (activeView === 'overview') setTimeout(refresh, 400); });

refresh();
