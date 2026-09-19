<template>
  <div class="page">
    <header class="page-head">
      <div><h1>服务器控制台</h1><p>通过 SSH 直接操作云服务器</p></div>
      <div class="head-actions">
        <button v-if="!connected" class="btn primary" @click="connect">连接</button>
        <button v-else class="btn danger" @click="disconnect">断开</button>
      </div>
    </header>
    <section class="panel ssh-panel">
      <div class="ssh-form" v-if="!connected">
        <div v-if="nodeServer" class="muted" style="margin-bottom: 12px; padding: 8px 12px; background: var(--bg-soft, #f5f5f5); border-radius: 6px;">
          已从代理节点「{{ nodeServer.name }}」自动匹配服务器: <b class="mono">{{ nodeServer.server }}</b>（SSH 与代理是同一台机器，主机已自动填入）
        </div>
        <div class="field"><label>主机</label><input v-model="form.host" placeholder="例: 203.0.113.10" /></div>
        <div class="field"><label>端口</label><input v-model.number="form.port" type="number" /></div>
        <div class="field"><label>用户名</label><input v-model="form.user" /></div>
        <div class="field"><label>认证方式</label><select v-model="form.auth"><option value="password">密码</option><option value="key">私钥</option></select></div>
        <div class="field" v-if="form.auth === 'password'"><label>密码</label><input v-model="form.password" type="password" /></div>
        <div class="field" v-else><label>私钥路径</label><input v-model="form.key" placeholder="~/.ssh/id_ed25519" /></div>
      </div>
      <div class="terminal-wrap">
        <div ref="termEl" class="terminal"></div>
      </div>
      <div class="quick-command-row" v-if="connected">
        <span class="muted">快捷命令</span>
        <button v-for="item in quickCommands" :key="item.label" class="btn small" @click="runQuickCommand(item.command)">{{ item.label }}</button>
      </div>
    </section>
  </div>
</template>
<script setup>
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
const quickCommands = [
  { label: '系统信息', command: 'uname -a; uptime\n' },
  { label: '内存', command: 'free -h; nproc\n' },
  { label: '磁盘', command: 'df -h\n' },
  { label: '进程', command: 'ps aux | head -20\n' },
  { label: '监听端口', command: 'ss -tlnp | head -20\n' },
  { label: '最近日志', command: 'journalctl -n 15 --no-pager\n' },
];
import '@xterm/xterm/css/xterm.css';

const props = defineProps({ config: Object });
const emit = defineEmits(['saved']);
const termEl = ref(null);
const connected = ref(false);
const connecting = ref(false);
let term, fit, timer, inputTimer, decoder = new TextDecoder();
const form = ref({
  host: props.config?.sshHost || '',
  port: props.config?.sshPort || 22,
  user: props.config?.sshUser || 'root',
  auth: props.config?.sshAuth || 'password',
  password: props.config?.sshPassword || '',
  key: props.config?.sshPrivateKey || '',
});
// dirty：用户开始编辑 form 后置 true，之后 props.config 的变化不再覆盖 form。
// 防止 App.vue 每 5 秒 refresh() 替换 config 引用触发 watch，把用户正在输入的
// host/port/user/password/key 重置回旧值（immediate 同步在 dirty 置 true 之前完成）。
const dirty = ref(false);
// 从选中的代理节点自动推导 SSH 主机：用户的代理节点就部署在云服务器上，
// SSH 和代理是同一台机器，不需要在两个地方填同一台服务器地址。
const nodeServer = computed(() => {
  const selected = props.config?.selectedNode;
  if (!selected || !props.config?.nodes) return null;
  const node = props.config.nodes.find(n => n.name === selected);
  return node ? { name: node.name, server: node.server } : null;
});
watch(form, () => { dirty.value = true; }, { deep: true });
watch(() => props.config, c => {
  if (!c || dirty.value) return;
  // 主机优先从选中的代理节点推导，其次旧配置，最后保留当前值
  const ns = c.nodes?.find(n => n.name === c.selectedNode)?.server;
  form.value.host = ns || c.sshHost || form.value.host;
  form.value.port = c.sshPort || form.value.port;
  form.value.user = c.sshUser || form.value.user;
  form.value.auth = c.sshAuth || form.value.auth;
  form.value.password = c.sshPassword || form.value.password;
  form.value.key = c.sshPrivateKey || form.value.key;
}, { immediate: true });
onMounted(async () => {
  await nextTick();
  term = new Terminal({ fontSize: 13, fontFamily: 'Menlo, monospace', theme: { background: '#0c111b', foreground: '#d7e0ee', cursor: '#6ea8fe' } });
  fit = new FitAddon();
  term.loadAddon(fit);
  term.open(termEl.value);
  fit.fit();
  term.writeln('输入连接信息后点击“连接”。');
  // 输入缓冲：合并 30ms 内的按键，避免每次击键都发一次 IPC
  let inputBuf = '';
  inputTimer = null;
  term.onData(d => {
    if (!connected.value) return;
    inputBuf += d;
    if (!inputTimer) {
      inputTimer = setTimeout(() => {
        inputTimer = null;
        const data = Array.from(new TextEncoder().encode(inputBuf));
        inputBuf = '';
        invoke('ssh_write', { data }).catch(() => {});
      }, 30);
    }
  });
  timer = setInterval(poll, 200);
});
async function poll() {
  if (!connected.value) return;
  try {
    const data = await invoke('ssh_read');
    if (data && data.length) term.write(decoder.decode(new Uint8Array(data)));
  } catch (e) {
    connected.value = false;
    term.writeln('\r\n连接已断开: ' + e);
  }
}
async function runQuickCommand(command) {
  if (!connected.value) return;
  try {
    await invoke('ssh_write', { data: Array.from(new TextEncoder().encode(command)) });
  } catch (e) {
    term.writeln('\r\n命令发送失败: ' + e);
  }
}
async function connect() {
  if (connecting.value) return;
  connecting.value = true;
  term.writeln('正在连接并验证登录（最多约 12 秒）…');
  try {
    await invoke('ssh_connect', {
      host: form.value.host, port: form.value.port, user: form.value.user,
      auth: form.value.auth, password: form.value.password || null, key: form.value.key || null,
    });
    emit('saved', {
      sshHost: form.value.host,
      sshPort: form.value.port,
      sshUser: form.value.user,
      sshAuth: form.value.auth,
      // 密码只进 Keychain（后端验证通过后存储），绝不进 config——否则下次任意保存会明文落盘
      sshPassword: null,
      sshPrivateKey: form.value.key || null,
    });
    connected.value = true;
    term.clear();
    term.writeln('已连接。');
  } catch (e) {
    term.writeln('\r\n连接失败: ' + e);
  } finally {
    connecting.value = false;
  }
}
async function disconnect() {
  try {
    await invoke('ssh_disconnect');
    connected.value = false;
    term.writeln('\r\n已断开。');
  } catch (e) {
    term.writeln('\r\n断开失败: ' + e);
  }
}
onUnmounted(async () => {
  clearInterval(timer);
  clearTimeout(inputTimer);
  if (connected.value) await invoke('ssh_disconnect').catch(() => {});
  // xterm Terminal 实例必须 dispose，否则切换页面后其内部定时器与 DOM 监听泄漏
  if (term) { term.dispose(); term = null; }
});
</script>
