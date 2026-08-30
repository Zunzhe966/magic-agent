<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark"></div>
        <div>
          <div class="brand-name">魔法代理</div>
          <div class="brand-sub">Magic Agent</div>
        </div>
      </div>
      <nav class="nav">
        <button v-for="item in navs" :key="item.id" class="nav-item" :class="{ active: view === item.id }" @click="view = item.id">
          <span class="nav-icon">{{ item.icon }}</span>
          <span>{{ item.label }}</span>
        </button>
      </nav>
      <div class="sidebar-status">
        <div class="dot" :class="{ on: status?.proxyRunning }"></div>
        <div>
          <div class="sidebar-status-title">{{ status?.proxyRunning ? '代理运行中' : '代理已停止' }}</div>
          <div class="sidebar-status-sub">{{ status && status.proxyRunning ? `端口 ${status.proxyPort}` : '未启动' }}</div>
        </div>
      </div>
    </aside>
    <main class="main">
      <Dashboard v-if="view === 'dashboard'" :status="status" :config="config" :proxy-busy="proxyBusy" :proxy-error="proxyError" @start="startProxy" @stop="stopProxy" @toggle-system-proxy="toggleSystemProxy" />
      <AppsView v-else-if="view === 'apps'" :apps="apps" :nodes="config?.nodes || []" @change="applyApps" @refresh="refreshApps" />
      <ServersView v-else-if="view === 'servers'" :config="config" @update="saveConfig" @select-server="selectSshServer" @delete-server="deleteSshServer" @nav="view = 'ssh'" />
      <ServerDashboard v-else-if="view === 'server-dashboard'" :config="config" @goto-servers="view = 'servers'" />
      <DomainRulesView v-else-if="view === 'domain-rules'" :config="config" @update="saveConfig" />
      <ConnectionsView v-else-if="view === 'connections'" :config="config" />
      <SshView v-else-if="view === 'ssh'" :config="config" @saved="onSshSaved" />
    </main>
  </div>
</template>
<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { toast } from './toast.js';
import Dashboard from './components/Dashboard.vue';
import AppsView from './components/AppsView.vue';
import ServersView from './components/ServersView.vue';
import ServerDashboard from './components/ServerDashboard.vue';
import DomainRulesView from './components/DomainRulesView.vue';
import ConnectionsView from './components/ConnectionsView.vue';
import SshView from './components/SshView.vue';

const view = ref('dashboard');
const status = ref(null);
const config = ref(null);
const apps = ref([]);
const proxyBusy = ref(false);
const proxyError = ref('');
const navs = [
  { id: 'dashboard', label: '总览', icon: '⌂' },
  { id: 'connections', label: '实时连接', icon: '⇅' },
  { id: 'apps', label: '软件分流', icon: '◎' },
  { id: 'domain-rules', label: '域名分流', icon: '⇄' },
  { id: 'servers', label: '云服务器', icon: '⛁' },
  { id: 'server-dashboard', label: '服务器仪表盘', icon: '▤' },
  { id: 'ssh', label: '控制台', icon: '❯' },
];

async function refresh() {
  // 轻量刷新：只拉状态和配置，不扫描 App（scan_apps 是重操作，放 refreshApps 单独做）
  try {
    const [s, c] = await Promise.all([invoke('get_status'), invoke('get_config')]);
    status.value = s;
    config.value = c;
  } catch (e) {
    console.error('refresh failed', e);
  }
}
async function refreshApps() {
  try {
    apps.value = await invoke('scan_apps');
  } catch (e) {
    console.error('scan apps failed', e);
  }
}
async function startProxy() {
  if (proxyBusy.value) return;
  proxyBusy.value = true;
  proxyError.value = '';
  try {
    await invoke('start_proxy');
    await refresh();
  } catch (e) {
    proxyError.value = '启动失败：' + String(e);
    console.error('start proxy failed', e);
  } finally {
    proxyBusy.value = false;
  }
}
async function stopProxy() {
  if (proxyBusy.value) return;
  proxyBusy.value = true;
  proxyError.value = '';
  try {
    await invoke('stop_proxy');
    await refresh();
  } catch (e) {
    proxyError.value = '停止失败：' + String(e);
    console.error('stop proxy failed', e);
  } finally {
    proxyBusy.value = false;
  }
}
async function toggleSystemProxy(enabled) {
  try {
    await invoke('set_system_proxy', { enabled });
  } catch (e) {
    proxyError.value = (enabled ? '开启' : '关闭') + '系统代理失败：' + String(e);
    console.error('toggle system proxy failed', e);
  }
  await refresh();
}
async function applyApps(list) {
  if (!config.value) return;
  // 只把"用户明确设为代理"的软件标记为已确认；直连的保持原有 confirmed 状态，
  // 避免一次"保存并应用"把所有未设置的软件都误标为 confirmed，导致规则表爆炸。
  const prevConfirmed = new Set((config.value.apps || []).filter(a => a.confirmed).map(a => a.id));
  config.value.apps = list.map(a => ({
    id: a.id,
    mode: a.mode,
    confirmed: a.mode === 'proxy' || prevConfirmed.has(a.id),
    node: a.node || null,
  }));
  await invoke('save_config', { config: config.value });
  await refresh();
}
async function saveConfig(patch) {
  if (!config.value) return;
  config.value = { ...config.value, ...patch };
  await invoke('save_config', { config: config.value });
  await refresh();
}
function onSshSaved(next) {
  config.value = { ...config.value, ...next };
}
let timer;
async function selectSshServer(serverId) {
  try {
    const server = await invoke('select_ssh_server', { serverId });
    config.value = {
      ...config.value,
      activeServerId: server.id,
      sshHost: server.host,
      sshPort: server.port,
      sshUser: server.user,
      sshAuth: server.auth,
      sshPassword: null,
      sshPrivateKey: server.keyPath || null,
    };
    view.value = 'ssh';
  } catch (e) {
    toast('选择服务器失败：' + e, 'error');
  }
}
async function deleteSshServer(serverId) {
  try {
    await invoke('delete_ssh_server', { serverId });
    await refresh();
  } catch (e) {
    toast('删除服务器失败：' + e, 'error');
  }
}
onMounted(async () => {
  await refresh();
  // App 扫描只加载一次，之后用户手动点"重新扫描"才刷新
  await refreshApps();
  // 定时器只做轻量状态轮询；只有停留在软件分流页时才刷新 App 运行状态
  timer = setInterval(() => {
    refresh();
    if (view.value === 'apps') refreshApps();
  }, 5000);
});
onUnmounted(() => clearInterval(timer));
</script>
