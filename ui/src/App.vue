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
      <Dashboard v-if="view === 'dashboard'" :status="status" :config="config" @start="startProxy" @stop="stopProxy" @toggle-system-proxy="toggleSystemProxy" />
      <AppsView v-else-if="view === 'apps'" :apps="apps" @change="applyApps" @refresh="refreshApps" />
      <ServersView v-else-if="view === 'servers'" :config="config" @update="saveConfig" @select-server="selectSshServer" @delete-server="deleteSshServer" @nav="view = 'ssh'" />
      <SshView v-else-if="view === 'ssh'" :config="config" @saved="onSshSaved" />
    </main>
  </div>
</template>
<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import Dashboard from './components/Dashboard.vue';
import AppsView from './components/AppsView.vue';
import ServersView from './components/ServersView.vue';
import SshView from './components/SshView.vue';

const view = ref('dashboard');
const status = ref(null);
const config = ref(null);
const apps = ref([]);
const navs = [
  { id: 'dashboard', label: '总览', icon: '⌂' },
  { id: 'apps', label: '软件分流', icon: '◎' },
  { id: 'servers', label: '云服务器', icon: '⛁' },
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
  try {
    await invoke('start_proxy');
  } catch (e) {
    alert('启动失败：' + e);
    return;
  }
  await refresh();
}
async function stopProxy() {
  try {
    await invoke('stop_proxy');
  } catch (e) {
    alert('停止失败：' + e);
    return;
  }
  await refresh();
}
async function toggleSystemProxy(enabled) {
  await invoke('set_system_proxy', { enabled });
  await refresh();
}
async function applyApps(list) {
  if (!config.value) return;
  config.value.apps = list.map(a => ({
    id: a.id,
    mode: a.mode,
    // 用户在界面保存过该软件，视为已确认；节点保持原选择
    confirmed: true,
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
    alert('选择服务器失败：' + e);
  }
}
async function deleteSshServer(serverId) {
  try {
    await invoke('delete_ssh_server', { serverId });
    await refresh();
  } catch (e) {
    alert('删除服务器失败：' + e);
  }
}
onMounted(async () => {
  await refresh();
  // App 扫描只加载一次，之后用户手动点"重新扫描"才刷新
  await refreshApps();
  // 定时器只做轻量状态轮询，不再扫描 App，避免界面卡顿
  timer = setInterval(refresh, 5000);
});
onUnmounted(() => clearInterval(timer));
</script>
