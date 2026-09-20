<template>
  <div class="page">
    <header class="page-head">
      <div><h1>设置</h1><p>应用版本与软件更新</p></div>
    </header>

    <section class="panel">
      <div class="stat-grid">
        <div class="stat-card">
          <div class="stat-label">当前版本</div>
          <div class="stat-value">{{ version || '—' }}</div>
          <div class="stat-sub">com.magic.agent</div>
        </div>
        <div class="stat-card">
          <div class="stat-label">更新状态</div>
          <div class="stat-value" :class="{ good: state === 'idle' || state === 'latest' }">{{ statusText }}</div>
          <div class="stat-sub">{{ state === 'available' ? `新版本 ${latestVersion}` : channelLabel }}</div>
          <button class="btn primary" :disabled="busy" @click="checkForUpdate(false)">{{ busy ? '检查中…' : '检查更新' }}</button>
        </div>
      </div>
      <div v-if="state === 'downloading' || state === 'installing'" class="progress-bar">
        <div class="progress-fill" :style="{ width: progress + '%' }"></div>
        <span class="progress-text">{{ state === 'downloading' ? `下载中 ${progress}%` : '安装中…' }}</span>
      </div>
    </section>

    <section class="panel">
      <h2>更新通道</h2>
      <p class="muted">选择从哪个源检查更新。两个通道使用同一把签名公钥校验，切换只改变下载来源。</p>
      <div class="channel-list">
        <label
          v-for="ch in channels"
          :key="ch.id"
          class="channel-item"
          :class="{ active: channel === ch.id, disabled: busy }"
        >
          <input
            type="radio"
            name="update-channel"
            :value="ch.id"
            :checked="channel === ch.id"
            :disabled="busy"
            @change="selectChannel(ch.id)"
          />
          <div class="channel-body">
            <div class="channel-name">{{ ch.name }}</div>
            <div class="channel-desc">{{ ch.desc }}</div>
            <code class="channel-endpoint">{{ ch.endpoint }}</code>
          </div>
        </label>
      </div>
      <p v-if="channelHint" class="muted channel-hint">{{ channelHint }}</p>
    </section>

    <section class="panel">
      <h2>关于</h2>
      <p class="muted">尊者魔法代理 / Magic Agent</p>
      <p class="muted">macOS Tauri 2 桌面代理软件 · Rust + Vue3</p>
      <p class="muted">当前更新通道：{{ channelLabel }}</p>
    </section>
  </div>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { toast } from '../toast.js';

const version = ref('');
const state = ref('idle');        // idle | checking | available | downloading | installing | latest | error
const latestVersion = ref('');
const progress = ref(0);
const busy = ref(false);

// ── 更新通道 ──────────────────────────────────────────────
// local  = 本地开发测试（自己用：本机起 http.server 7878）
// github = GitHub Releases（真实用户用：公开分发）
const channel = ref('github');
const lastEndpoint = ref('');

const channels = [
  {
    id: 'local',
    name: '本地开发测试',
    desc: '从本机 127.0.0.1:7878 拉取更新包，供开发者调试发版流程。',
    endpoint: 'http://127.0.0.1:7878/latest.json',
  },
  {
    id: 'github',
    name: 'GitHub 公开发布',
    desc: '从 GitHub Releases 拉取更新包，面向所有终端用户，无需本机服务。',
    endpoint: 'https://github.com/Zunzhe966/magic-agent/releases/latest/download/latest.json',
  },
];

const channelLabel = computed(
  () => channels.find((c) => c.id === channel.value)?.name || channel.value,
);
const channelHint = computed(() =>
  channel.value === 'local'
    ? '提示：本地通道需要先在终端运行 cd scripts/updater-feed && python3 -m http.server 7878 --bind 127.0.0.1'
    : '',
);

const statusText = computed(() => ({
  idle: '未检查',
  checking: '检查中',
  available: '有新版本',
  downloading: '下载中',
  installing: '安装中',
  latest: '已是最新',
  error: '检查失败',
}[state.value] || '—'));

async function selectChannel(id) {
  if (busy.value || id === channel.value) return;
  try {
    const info = await invoke('set_update_channel', { channel: id });
    channel.value = info.channel;
    // 切换通道后重置状态，避免残留上一次的「已是最新」误导用户
    state.value = 'idle';
    latestVersion.value = '';
    toast(`已切换到「${info.label}」通道`);
  } catch (e) {
    toast('切换更新通道失败: ' + e);
  }
}

async function checkForUpdate(quiet) {
  if (busy.value) return;
  busy.value = true;
  state.value = 'checking';
  try {
    const res = await invoke('check_channel_update');
    lastEndpoint.value = res.endpoint || '';
    if (!res.available) {
      state.value = 'latest';
      if (!quiet) toast('已是最新版本');
      return;
    }
    latestVersion.value = res.version;
    state.value = 'available';
    const ok = await ask(
      `发现新版本 v${res.version}，立即更新？${
        channel.value === 'local' ? '\n（本地开发测试通道）' : '\n（GitHub 公开发布通道）'
      }`,
      { title: '软件更新', kind: 'info' }
    );
    if (!ok) {
      state.value = 'idle';
      return;
    }
    state.value = 'downloading';
    progress.value = 0;
    await invoke('install_channel_update');
    toast('更新已安装，即将重启…');
    // 给前端一点时间渲染 100% 进度条
    await new Promise((r) => setTimeout(r, 300));
    await relaunch();
  } catch (e) {
    state.value = 'error';
    if (!quiet) toast('检查更新失败: ' + e);
  } finally {
    busy.value = false;
  }
}

defineExpose({ checkForUpdate });

let unlistenProgress = null;

onMounted(async () => {
  try {
    version.value = await getVersion();
  } catch (e) {
    console.error('getVersion failed', e);
  }
  // 读取当前通道（持久化在 config.json，重启后保持）
  try {
    const info = await invoke('get_update_channel');
    channel.value = info.channel;
  } catch (e) {
    console.error('get_update_channel failed', e);
  }
  // 订阅 Rust 侧推送的下载进度
  try {
    unlistenProgress = await listen('update-progress', (evt) => {
      const p = evt.payload || {};
      if (p.event === 'progress') {
        const len = p.contentLength || 0;
        progress.value = len
          ? Math.min(99, Math.round((p.downloaded / len) * 100))
          : progress.value;
      } else if (p.event === 'finished') {
        progress.value = 100;
        state.value = 'installing';
      }
    });
  } catch (e) {
    console.error('listen update-progress failed', e);
  }
});

onBeforeUnmount(() => {
  if (unlistenProgress) unlistenProgress();
});
</script>

<style scoped>
.channel-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 12px;
}
.channel-item {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  padding: 12px 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
  background: var(--panel2);
}
.channel-item:hover {
  border-color: var(--accent);
}
.channel-item.active {
  border-color: var(--accent);
  background: rgba(77, 124, 254, 0.14);
}
.channel-item.disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.channel-item input[type='radio'] {
  margin-top: 3px;
  accent-color: var(--accent);
}
.channel-body {
  flex: 1;
  min-width: 0;
}
.channel-name {
  font-weight: 600;
  margin-bottom: 3px;
}
.channel-desc {
  font-size: 12.5px;
  color: var(--muted);
  line-height: 1.5;
}
.channel-endpoint {
  display: block;
  margin-top: 6px;
  font-size: 11.5px;
  color: var(--muted);
  word-break: break-all;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.channel-hint {
  margin-top: 10px;
  font-size: 12px;
}
</style>
