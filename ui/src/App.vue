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
      <SettingsView v-else-if="view === 'settings'" ref="settingsView" />
    </main>
  </div>
</template>
<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { check } from '@tauri-apps/plugin-updater';
import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { toast } from './toast.js';
import Dashboard from './components/Dashboard.vue';
import AppsView from './components/AppsView.vue';
import ServersView from './components/ServersView.vue';
import ServerDashboard from './components/ServerDashboard.vue';
import DomainRulesView from './components/DomainRulesView.vue';
import ConnectionsView from './components/ConnectionsView.vue';
import SshView from './components/SshView.vue';
import SettingsView from './components/SettingsView.vue';

const settingsView = ref(null);

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
  { id: 'settings', label: '设置', icon: '⚙' },
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
    const fresh = await invoke('scan_apps');
    // 只刷新运行状态，保留用户正在编辑但尚未保存的 mode/node/confirmed，
    // 避免 5 秒轮询把用户刚切换的下拉框值冲回旧值
    const byId = new Map(apps.value.map(a => [a.id, a]));
    apps.value = fresh.map(f => {
      const old = byId.get(f.id);
      if (old) {
        f.mode = old.mode;
        f.node = old.node;
        f.confirmed = old.confirmed;
      }
      return f;
    });
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
  try {
    await invoke('save_config', { config: config.value });
    toast('已保存并应用分流规则');
  } catch (e) {
    toast('保存失败：' + e, 'error');
  }
  await refresh();
}
async function saveConfig(patch) {
  if (!config.value) return;
  config.value = { ...config.value, ...patch };
  try {
    await invoke('save_config', { config: config.value });
  } catch (e) {
    // 后端返回 Err（如规则热更新失败）必须让用户看到，否则用户以为已保存
    toast('保存失败：' + e, 'error');
  }
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
// 启动时静默检查更新：有新版弹原生对话框，用户确认后下载安装+重启
// 失败静默吞掉，不打扰用户；手动检查走设置页 SettingsView 自己的 UI 流程
async function checkForUpdateQuiet() {
  try {
    const update = await check();
    if (!update) return;
    const ok = await ask(
      `发现新版本 v${update.version}，立即更新？`,
      { title: '软件更新', kind: 'info' }
    );
    if (!ok) return;
    toast('正在下载更新…');
    await update.downloadAndInstall();
    toast('更新已安装，即将重启…');
    await relaunch();
  } catch (e) {
    console.error('update check failed', e);
  }
}

onMounted(async () => {
  // 先只拉轻量数据（状态+配置），让界面立刻可用可点击。
  // scan_apps 是重操作（扫全盘 App + lsof 全机连接），绝不阻塞首屏交互。
  await refresh();
  // App 扫描放后台异步跑，不 await：用户点导航/按钮时不会被扫描卡住
  refreshApps();
  // 定时器只做轻量状态轮询；只有停留在软件分流页时才刷新 App 运行状态
  timer = setInterval(() => {
    refresh();
    if (view.value === 'apps') refreshApps();
  }, 5000);
  // 启动静默检查更新（不打扰，失败静默吞掉）
  checkForUpdateQuiet();
});
onUnmounted(() => clearInterval(timer));
</script>
