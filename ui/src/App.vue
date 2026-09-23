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
      <DomainRulesView v-else-if="view === 'domain-rules'" :config="config" @update="saveDomainRules" />
      <ConnectionsView v-else-if="view === 'connections'" :config="config" />
      <SshView v-else-if="view === 'ssh'" :config="config" @saved="onSshSaved" />
      <SettingsView v-else-if="view === 'settings'" :config="config" @update="saveConfig" />
    </main>
  </div>
</template>
<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';
import { toast } from './toast.js';

import Dashboard from './components/Dashboard.vue';
import AppsView from './components/AppsView.vue';
import ServersView from './components/ServersView.vue';
import ServerDashboard from './components/ServerDashboard.vue';
import DomainRulesView from './components/DomainRulesView.vue';
import ConnectionsView from './components/ConnectionsView.vue';
import SshView from './components/SshView.vue';
import SettingsView from './components/SettingsView.vue';

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
    // P0-4 崩溃自愈提示：后端一次性下发（读取即清空），toast 告知用户
    if (s.selfHealNotice) {
      toast(s.selfHealNotice, 'warn');
    }
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
  // P1-3 确认模式：开关开启时，先跑【只读】归序前体检，把"接下来要动什么"
  // 列给用户过目，确认后才真正接管（记账+清理+启动）。关闭时沿用快速模式。
  if (config.value?.confirmTakeover) {
    try {
      const plan = await invoke('takeover_plan');
      if (plan.hasSources) {
        const lines = plan.foreign.map((f) => `• 将关闭：${f}`).join('\n');
        const stale = plan.staleProxy ? '\n• 将清理：系统代理残留（指向已停止的端口）' : '';
        const ok = await ask(
          `检测到以下网络混乱源，启动代理将接管并清理它们：\n${lines}${stale}\n\n体检结论：${plan.summary}\n\n注意：被关闭的第三方代理进程不会自动复活（系统代理设置会记账、失败时自动还原）。确认继续？`,
          { title: '接管确认', kind: 'warning' },
        );
        if (!ok) return;
      }
    } catch (e) {
      // 体检失败不拦启动（它是辅助信息），但必须让用户知道没体检成
      toast('接管前体检失败：' + e + '（将直接启动）', 'warn');
    }
  }
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
    const r = await invoke('set_system_proxy', { enabled });
    // P0-1：部分服务未达标时如实提示，不再默认"成功"
    if (r && r.allOk === false && r.mismatched && r.mismatched.length) {
      toast(`系统代理${enabled ? '开启' : '关闭'}不完全：${r.mismatched.join('、')} 未生效，请检查这些网络的服务设置`, 'warn');
    }
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
  const previous = config.value;
  config.value = { ...config.value, ...patch };
  try {
    await invoke('save_config', { config: config.value });
    return true;
  } catch (e) {
    // 后端返回 Err（如规则热更新失败）必须让用户看到，否则用户以为已保存
    config.value = previous;
    toast('保存失败：' + e, 'error');
    return false;
  }
}

async function saveDomainRules(patch) {
  const ok = await saveConfig(patch);
  if (ok) await refresh();
  return ok;
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
// 启动时不自动检查更新：检查更新只在设置页手动触发，避免一开就查本地/GitHub。

onMounted(async () => {
  // 先拉轻量数据（状态+配置），界面立即可用。
  await refresh();
  
  // 自动启动代理：无论之前状态如何，打开 App 就把代理和系统代理启动好。
  // 这是用户核心诉求：打开就可用，不需要手动点"启动"。
  if (!status.value?.proxyRunning) {
    console.log('[App] 代理未运行，自动启动...');
    await startProxy();
  } else if (!status.value?.systemProxy) {
    // 代理在跑但系统代理没开（如 App 重启后），立即开启
    console.log('[App] 代理在跑但系统代理未开，自动开启...');
    await toggleSystemProxy(true);
  }
  
  // App 扫描放后台异步跑，不阻塞首屏
  refreshApps();
  // 定时器只做轻量状态轮询；只有停留在软件分流页时才刷新 App 运行状态
  timer = setInterval(() => {
    refresh();
    if (view.value === 'apps') refreshApps();
  }, 5000);
});
onUnmounted(() => clearInterval(timer));
</script>
