// 全局唯一的更新流程模块。
//
// 历史教训：启动静默更新（App.vue）和设置页手动更新（SettingsView.vue）
// 曾经各写一套——各自 invoke、各自弹窗、各自监听进度，行为不一致还互相抢状态。
// 现在统一到这里：
//   - checkUpdate({ manual: false })  启动时静默检查（用户可「跳过此版本」）
//   - checkUpdate({ manual: true })   设置页手动检查（带进度条、必弹 toast）
//   - selectChannel(id)               切换 local / github 通道
// 所有页面共享同一个 updaterState，进度事件只注册一次监听。
import { reactive } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { toast } from './toast.js';

const SKIP_KEY = 'magic-agent.update.skippedVersion';

export const updaterState = reactive({
  // idle | checking | available | downloading | installing | latest | error
  state: 'idle',
  busy: false,
  latestVersion: '',
  notes: '',
  progress: 0,
  channel: 'github',
  channelLabel: 'GitHub 公开发布',
});

// 下载/安装进度是 Rust 侧全局广播事件，整个 App 只监听一次。
let listening = false;
function ensureProgressListener() {
  if (listening) return;
  listening = true;
  listen('update-progress', (evt) => {
    const p = evt.payload || {};
    if (p.event === 'progress') {
      const len = p.contentLength || 0;
      if (len) updaterState.progress = Math.min(99, Math.round((p.downloaded / len) * 100));
    } else if (p.event === 'finished') {
      updaterState.progress = 100;
      updaterState.state = 'installing';
    }
  }).catch((e) => console.error('listen update-progress failed', e));
}

export async function refreshChannel() {
  try {
    const info = await invoke('get_update_channel');
    updaterState.channel = info.channel;
    updaterState.channelLabel = info.label;
  } catch (e) {
    console.error('get_update_channel failed', e);
  }
}

export async function selectChannel(id) {
  if (updaterState.busy || id === updaterState.channel) return;
  try {
    const info = await invoke('set_update_channel', { channel: id });
    updaterState.channel = info.channel;
    updaterState.channelLabel = info.label;
    // 切通道后清掉旧的检查结果，避免把 A 源的「有新版本」误算到 B 源头上
    updaterState.state = 'idle';
    updaterState.latestVersion = '';
    updaterState.progress = 0;
    toast(`已切换到「${info.label}」通道`);
  } catch (e) {
    toast('切换更新通道失败: ' + e);
  }
}

// manual=true 表示用户在设置页主动点的：所有反馈都用 toast 明示；
// manual=false 表示启动静默检查：失败只记日志，用户点「否」=跳过该版本，
// 之后每次启动不再打扰，直到服务端出现更新的版本。
export async function checkUpdate({ manual = false } = {}) {
  if (updaterState.busy) return;
  updaterState.busy = true;
  updaterState.state = 'checking';
  updaterState.progress = 0;
  try {
    const res = await invoke('check_channel_update');
    if (!res.available) {
      updaterState.state = 'latest';
      if (manual) toast('已是最新版本');
      return;
    }

    updaterState.latestVersion = res.version || '';
    updaterState.notes = res.notes || '';

    if (!manual && localStorage.getItem(SKIP_KEY) === res.version) {
      // 该版本已被用户跳过：静默，不弹窗
      updaterState.state = 'idle';
      return;
    }

    const notesLine = res.notes ? `\n\n${res.notes}` : '';
    const ok = await ask(
      `发现新版本 v${res.version}，立即更新？\n（${updaterState.channelLabel}）${notesLine}`,
      { title: '软件更新', kind: 'info' },
    );
    if (!ok) {
      if (!manual) localStorage.setItem(SKIP_KEY, res.version);
      updaterState.state = 'idle';
      return;
    }
    localStorage.removeItem(SKIP_KEY);

    updaterState.state = 'downloading';
    ensureProgressListener();
    await invoke('install_channel_update');
    toast('更新已安装，即将重启…');
    // 给前端一点时间渲染 100% 进度条再重启
    await new Promise((r) => setTimeout(r, 300));
    await relaunch();
  } catch (e) {
    updaterState.state = 'error';
    if (manual) toast('更新失败: ' + e);
    else console.error('update check failed', e);
  } finally {
    updaterState.busy = false;
  }
}
