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
          <div class="stat-sub">{{ state === 'available' ? `新版本 ${latestVersion}` : '本地开发链路' }}</div>
          <button class="btn primary" :disabled="busy" @click="checkForUpdate(false)">{{ busy ? '检查中…' : '检查更新' }}</button>
        </div>
      </div>
      <div v-if="state === 'downloading' || state === 'installing'" class="progress-bar">
        <div class="progress-fill" :style="{ width: progress + '%' }"></div>
        <span class="progress-text">{{ state === 'downloading' ? `下载中 ${progress}%` : '安装中…' }}</span>
      </div>
    </section>
    <section class="panel">
      <h2>关于</h2>
      <p class="muted">尊者魔法代理 / Magic Agent</p>
      <p class="muted">macOS Tauri 2 桌面代理软件 · Rust + Vue3</p>
      <p class="muted">更新通道：本地开发测试（http://127.0.0.1:7878）</p>
    </section>
  </div>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';
import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { toast } from '../toast.js';

const version = ref('');
const state = ref('idle');        // idle | checking | available | downloading | installing | latest | error
const latestVersion = ref('');
const progress = ref(0);
const busy = ref(false);

const statusText = computed(() => ({
  idle: '未检查',
  checking: '检查中',
  available: '有新版本',
  downloading: '下载中',
  installing: '安装中',
  latest: '已是最新',
  error: '检查失败',
}[state.value] || '—'));

async function checkForUpdate(quiet) {
  if (busy.value) return;
  busy.value = true;
  state.value = 'checking';
  try {
    const update = await check();
    if (!update) {
      state.value = 'latest';
      if (!quiet) toast('已是最新版本');
      return;
    }
    latestVersion.value = update.version;
    state.value = 'available';
    const ok = await ask(
      `发现新版本 v${update.version}，立即更新？`,
      { title: '软件更新', kind: 'info' }
    );
    if (!ok) {
      state.value = 'idle';
      return;
    }
    state.value = 'downloading';
    progress.value = 0;
    let downloaded = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === 'Started' && event.data.contentLength) {
        downloaded = 0;
        progress.value = 0;
      } else if (event.event === 'Progress') {
        downloaded += event.data.chunkLength;
        const len = event.data.contentLength || 1;
        progress.value = Math.min(99, Math.round((downloaded / len) * 100));
      } else if (event.event === 'Finished') {
        progress.value = 100;
        state.value = 'installing';
      }
    });
    toast('更新已安装，即将重启…');
    await relaunch();
  } catch (e) {
    state.value = 'error';
    if (!quiet) toast('检查更新失败: ' + e);
  } finally {
    busy.value = false;
  }
}

defineExpose({ checkForUpdate });

onMounted(async () => {
  try {
    version.value = await getVersion();
  } catch (e) {
    console.error('getVersion failed', e);
  }
});
</script>
