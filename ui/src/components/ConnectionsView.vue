<template>
  <div>
    <section class="panel">
      <div class="panel-head">
        <h2>实时连接</h2>
        <div class="card-actions">
          <span class="muted" style="margin-right: 8px;">共 {{ total }} 条，每 3 秒刷新</span>
          <button class="btn small" @click="paused = !paused">{{ paused ? '继续' : '暂停' }}</button>
        </div>
      </div>
      <p class="muted" style="margin-bottom: 8px;">当前每条流量走的主机、命中的规则和出口节点。按下载量排序，只显示前 40 条。</p>
      <div v-if="error" class="muted" style="color: var(--color-text-danger);">{{ error }}</div>
      <table v-else class="conn-table">
        <thead>
          <tr><th>主机</th><th>进程</th><th>命中规则</th><th>出口</th><th style="text-align:right;">↓</th><th style="text-align:right;">↑</th></tr>
        </thead>
        <tbody>
          <tr v-for="(c, i) in conns" :key="i">
            <td class="mono">{{ c.host }}</td>
            <td class="muted">{{ c.proc || '-' }}</td>
            <td>{{ c.rule }}</td>
            <td><span class="pill" :class="{ proxy: c.node !== 'DIRECT' }">{{ c.node }}</span></td>
            <td style="text-align:right;" class="mono">{{ fmt(c.down) }}</td>
            <td style="text-align:right;" class="mono">{{ fmt(c.up) }}</td>
          </tr>
          <tr v-if="!conns.length"><td colspan="6" class="muted">暂无连接（代理未运行或刚启动）</td></tr>
        </tbody>
      </table>
    </section>
  </div>
</template>
<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
const props = defineProps({ config: Object });
const conns = ref([]);
const total = ref(0);
const error = ref('');
const paused = ref(false);
let timer = null;

async function poll() {
  if (paused.value) return;
  try {
    // 不再直接 fetch 19091：控制 API 走 Rust 后端 proxy_api，secret 由后端持有，前端拿不到
    const [status, body] = await invoke('proxy_api', { path: '/connections' });
    if (status !== 200) throw new Error('HTTP ' + status);
    const data = JSON.parse(body);
    const list = (data.connections || []).map(c => {
      const md = c.metadata || {};
      return {
        host: (md.host || md.destinationIP || '?') + ':' + (md.destinationPort || ''),
        proc: (md.processPath || '').split('/').pop(),
        rule: (c.rule || '?') + (c.rulePayload ? `(${c.rulePayload})` : ''),
        node: (c.chains && c.chains[0]) || '?',
        up: c.upload || 0,
        down: c.download || 0,
      };
    }).sort((a, b) => b.down - a.down);
    total.value = list.length;
    conns.value = list.slice(0, 40);
    error.value = '';
  } catch (e) {
    error.value = '读取失败：代理可能未运行';
    conns.value = [];
    total.value = 0;
  }
}
function fmt(n) {
  if (!n) return '0';
  if (n < 1024) return n + 'B';
  if (n < 1048576) return (n / 1024).toFixed(1) + 'K';
  return (n / 1048576).toFixed(1) + 'M';
}
onMounted(() => {
  poll();
  timer = setInterval(poll, 3000);
});
onUnmounted(() => clearInterval(timer));
</script>
<style scoped>
.conn-table { width: 100%; border-collapse: collapse; font-size: 12.5px; }
.conn-table th { text-align: left; font-weight: 500; color: var(--color-text-secondary); padding: 6px 8px; border-bottom: 1px solid var(--color-border-secondary); }
.conn-table td { padding: 5px 8px; border-bottom: 0.5px solid var(--color-border-tertiary); }
.mono { font-family: var(--font-mono); font-size: 12px; }
.pill { padding: 1px 8px; border-radius: 999px; background: var(--color-background-tertiary); font-size: 12px; }
.pill.proxy { background: rgba(29, 158, 117, 0.15); color: #0F6E56; }
</style>
