<template>
  <div class="page">
    <header class="page-head">
      <div>
        <h1>云服务器仪表盘</h1>
        <p>远程实时查看云服务器状态：CPU / 内存 / 磁盘 / 网络</p>
      </div>
      <div class="head-actions">
        <button class="btn primary" :disabled="loading" @click="probe">{{ loading ? '探测中…' : '刷新探测' }}</button>
      </div>
      <div v-if="error" class="proxy-error" role="alert">{{ error }}</div>
    </header>

    <!-- 未配置服务器引导 -->
    <section v-if="!hasServer" class="panel">
      <div class="panel-head"><h2>尚未配置云服务器</h2></div>
      <p class="muted">
        先到「云服务器」页添加 SSH 连接（host / 端口 / 用户名 / 密码），
        之后这里就能实时显示服务器的 CPU、内存、磁盘、网络状态。
      </p>
      <button class="btn primary" @click="$emit('goto-servers')">去配置云服务器</button>
    </section>

    <template v-else>
      <!-- 服务器信息 -->
      <section class="panel" v-if="metrics?.server">
        <div class="panel-head">
          <h2>{{ metrics.server.name }}</h2>
          <span class="status-pill ok">已连接</span>
        </div>
        <div class="kv"><span>地址</span><b class="mono">{{ metrics.server.user }}@{{ metrics.server.host }}</b></div>
        <div class="kv" v-if="metrics.uptime"><span>在线时长</span><b>{{ metrics.uptime }}</b></div>
      </section>

      <!-- 状态卡片 -->
      <div class="stat-grid" v-if="metrics">
        <div class="stat-card">
          <div class="stat-label">CPU 使用率</div>
          <div class="stat-value">{{ metrics.cpuUsagePct ?? '—' }}%</div>
          <div class="bar"><div class="bar-fill" :style="{ width: clamp(metrics.cpuUsagePct) + '%' }" :class="barClass(metrics.cpuUsagePct)"></div></div>
        </div>
        <div class="stat-card">
          <div class="stat-label">内存使用率</div>
          <div class="stat-value">{{ metrics.memUsagePct ?? '—' }}%</div>
          <div class="stat-sub">{{ metrics.memUsedMb }} / {{ metrics.memTotalMb }} MB</div>
          <div class="bar"><div class="bar-fill" :style="{ width: clamp(metrics.memUsagePct) + '%' }" :class="barClass(metrics.memUsagePct)"></div></div>
        </div>
        <div class="stat-card">
          <div class="stat-label">磁盘使用率</div>
          <div class="stat-value">{{ metrics.diskUsagePct ?? '—' }}%</div>
          <div class="stat-sub">{{ metrics.diskUsed }} / {{ metrics.diskSize }}（可用 {{ metrics.diskAvail }}）</div>
          <div class="bar"><div class="bar-fill" :style="{ width: clamp(metrics.diskUsagePct) + '%' }" :class="barClass(metrics.diskUsagePct)"></div></div>
        </div>
        <div class="stat-card">
          <div class="stat-label">负载（1m / 5m / 15m）</div>
          <div class="stat-value load">{{ metrics.load1m ?? '—' }} / {{ metrics.load5m ?? '—' }} / {{ metrics.load15m ?? '—' }}</div>
          <div class="stat-sub">系统平均负载</div>
        </div>
      </div>

      <!-- 网络流量 -->
      <section class="panel" v-if="netInterfaces.length">
        <div class="panel-head"><h2>网络流量（累计）</h2></div>
        <div class="net-row" v-for="n in netInterfaces" :key="n.name">
          <div class="net-name">{{ n.name }}</div>
          <div class="net-kv"><span>下行 RX</span><b>{{ fmtBytes(n.rx) }}</b></div>
          <div class="net-kv"><span>上行 TX</span><b>{{ fmtBytes(n.tx) }}</b></div>
        </div>
      </section>

      <p class="muted" v-if="lastProbe">上次探测：{{ lastProbe }}</p>
    </template>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { toast } from '../toast.js';

const props = defineProps({ config: Object });
const emit = defineEmits(['goto-servers']);

const metrics = ref(null);
const error = ref('');
const loading = ref(false);
const lastProbe = ref('');

const hasServer = computed(() => {
  const s = props.config?.servers || [];
  return s.length > 0 || props.config?.sshHost;
});

const netInterfaces = computed(() => {
  if (!metrics.value) return [];
  const out = [];
  for (const [k, v] of Object.entries(metrics.value)) {
    const m = k.match(/^net_(.+)_rx_bytes$/);
    if (m) {
      out.push({ name: m[1], rx: v, tx: metrics.value[`net_${m[1]}_tx_bytes`] || 0 });
    }
  }
  return out;
});

function clamp(v) {
  if (v === null || v === undefined || isNaN(v)) return 0;
  return Math.max(0, Math.min(100, Number(v)));
}
function barClass(v) {
  const n = Number(v);
  if (isNaN(n)) return '';
  if (n >= 85) return 'bar-danger';
  if (n >= 60) return 'bar-warn';
  return 'bar-ok';
}
function fmtBytes(b) {
  if (!b && b !== 0) return '—';
  const n = Number(b);
  if (n >= 1e9) return (n / 1e9).toFixed(2) + ' GB';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + ' MB';
  if (n >= 1e3) return (n / 1e3).toFixed(2) + ' KB';
  return n + ' B';
}

async function probe() {
  if (loading.value) return;
  loading.value = true;
  error.value = '';
  try {
    // 优先走 Rust 后端 server_metrics（secret/凭据由后端持有）
    const data = await invoke('server_metrics');
    metrics.value = data;
    lastProbe.value = new Date().toLocaleTimeString();
  } catch (e) {
    error.value = '探测失败：' + String(e);
  } finally {
    loading.value = false;
  }
}

let timer;
onMounted(() => {
  if (hasServer.value) {
    probe();
    timer = setInterval(probe, 10000); // 每 10 秒自动刷新
  }
});
onUnmounted(() => clearInterval(timer));
</script>

<style scoped>
.bar {
  margin-top: 10px;
  height: 8px;
  background: var(--border, #e5e7eb);
  border-radius: 4px;
  overflow: hidden;
}
.bar-fill {
  height: 100%;
  border-radius: 4px;
  transition: width 0.4s ease;
}
.bar-ok { background: #10b981; }
.bar-warn { background: #f59e0b; }
.bar-danger { background: #ef4444; }
.stat-value.load { font-size: 18px; }
.net-row {
  display: flex;
  align-items: center;
  gap: 24px;
  padding: 10px 0;
  border-bottom: 1px solid var(--border, #eee);
}
.net-row:last-child { border-bottom: none; }
.net-name {
  font-weight: 600;
  min-width: 80px;
  font-family: monospace;
}
.net-kv { display: flex; gap: 8px; align-items: baseline; }
.net-kv span { color: var(--muted, #888); font-size: 13px; }
.net-kv b { font-family: monospace; }
.kv { display: flex; gap: 12px; padding: 4px 0; }
.kv span { color: var(--muted, #888); min-width: 70px; }
</style>
