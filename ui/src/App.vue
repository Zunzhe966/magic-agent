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
          <div class="sidebar-status-sub">{{ status?.proxyRunning ? `端口 ${status?.proxyPort}` : '未启动' }}</div>
        </div>
      </div>
    </aside>
    <main class="main">
      <Dashboard v-if="view === 'dashboard'" :status="status" :config="config" @start="startProxy" @stop="stopProxy" @toggle-system-proxy="toggleSystemProxy" />
      <AppsView v-else-if="view === 'apps'" :apps="apps" />
      <ServersView v-else-if="view === 'servers'" :config="config" :status="status" />
      <SshView v-else-if="view === 'ssh'" :config="config" />
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
  status.value = await invoke('get_status');
  config.value = await invoke('get_config');
  apps.value = await invoke('scan_apps');
}
async function startProxy(rules, direct, proxy) {
  await invoke('start_proxy', { rules, directApps: direct, proxyApps: proxy });
  await refresh();
}
async function stopProxy() {
  await invoke('stop_proxy');
  await refresh();
}
async function toggleSystemProxy(enabled) {
  await invoke('set_system_proxy', { enabled });
  await refresh();
}
let timer;
onMounted(async () => {
  await refresh();
  timer = setInterval(refresh, 3000);
});
onUnmounted(() => clearInterval(timer));
</script>
