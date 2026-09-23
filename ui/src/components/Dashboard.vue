<template>
  <div class="page">
    <header class="page-head">
      <div>
        <h1>总览</h1>
        <p>代理引擎与应用分流状态</p>
      </div>
      <div class="head-actions">
        <button v-if="!status?.proxyRunning" class="btn primary" :disabled="proxyBusy" @click="$emit('start')">{{ proxyBusy ? '正在启动…' : '启动代理' }}</button>
        <button v-else class="btn danger" :disabled="proxyBusy" @click="$emit('stop')">{{ proxyBusy ? '正在停止…' : '停止代理' }}</button>
      </div>
      <div v-if="proxyError" class="proxy-error" role="alert">{{ proxyError }}</div>
    </header>
    <!-- 第三方代理清理：启动魔法代理前，把系统里其他代理全部关掉，回到干净状态 -->
    <section class="panel" v-if="foreignProxies.length || cleanedNotice">
      <div class="panel-head">
        <h2>第三方代理</h2>
        <button class="btn small" :disabled="cleaning" @click="refreshForeign">{{ cleaning ? '清理中…' : '重新检测' }}</button>
      </div>
      <p class="muted" v-if="cleanedNotice">✓ {{ cleanedNotice }}</p>
      <template v-if="foreignProxies.length">
        <p class="muted">检测到以下第三方代理正在运行，启动时会自动关闭它们，让系统回到干净状态：</p>
        <ul class="foreign-list">
          <li v-for="(p, i) in foreignProxies" :key="i">{{ p }}</li>
        </ul>
        <button class="btn primary" :disabled="cleaning" @click="cleanNow">立即清理并恢复干净状态</button>
      </template>
    </section>
    <div class="stat-grid">
      <div class="stat-card">
        <div class="stat-label">代理内核</div>
        <div class="stat-value" :class="{ good: status?.proxyRunning }">{{ status?.proxyRunning ? '运行中' : '已停止' }}</div>
        <div class="stat-sub">mihomo v1.19.29</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">混合端口</div>
        <div class="stat-value">{{ status?.proxyPort ?? '—' }}</div>
        <div class="stat-sub">HTTP / SOCKS5</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">系统代理</div>
        <div class="stat-value" :class="{ good: status?.systemProxy }">{{ status?.systemProxy ? '已开启' : '未开启' }}</div>
        <div class="stat-sub">macOS networksetup</div>
        <button class="btn small" :disabled="proxyBusy" @click="$emit('toggle-system-proxy', !status?.systemProxy)">{{ status?.systemProxy ? '关闭系统代理' : '开启系统代理' }}</button>
      </div>
      <div class="stat-card">
        <div class="stat-label">已识别软件</div>
        <div class="stat-value">{{ status?.appsCount ?? '—' }}</div>
        <div class="stat-sub">/Applications 自动扫描</div>
      </div>
    </div>
    <section class="panel">
      <div class="panel-head">
        <h2>当前分流策略</h2>
        <div class="mode-pill">双路显式选择</div>
      </div>
      <div class="policy-row" v-if="config">
        <div class="policy-item">
          <div class="policy-dot green"></div>
          <div><strong>本机与内网</strong><span>银行、本地服务、内网、系统软件自动直连</span></div>
        </div>
        <div class="policy-item">
          <div class="policy-dot blue"></div>
          <div><strong>节点代理 / 本机直连</strong><span>端口 7893 全部走节点；端口 7892 全部本机直连</span></div>
        </div>
        <div class="policy-item">
          <div class="policy-dot orange"></div>
          <div><strong>显式软件/域名规则</strong><span>按软件、域名或指定节点精确分流</span></div>
        </div>
      </div>
    </section>
  </div>
</template>
<script setup>
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { toast } from '../toast.js';

const props = defineProps({
  status: Object,
  config: Object,
  proxyBusy: Boolean,
  proxyError: String,
});
const emit = defineEmits(['start', 'stop', 'toggle-system-proxy']);

// 第三方代理检测与清理
const foreignProxies = ref([]);
const cleaning = ref(false);
const cleanedNotice = ref('');

async function refreshForeign() {
  try {
    foreignProxies.value = await invoke('list_foreign_proxies');
  } catch (e) {
    console.error('list foreign proxies failed', e);
  }
}
async function cleanNow() {
  if (cleaning.value) return;
  cleaning.value = true;
  try {
    const cleaned = await invoke('kill_foreign_proxies');
    cleanedNotice.value = cleaned.length
      ? `已关闭 ${cleaned.length} 个第三方代理进程，系统代理已恢复为未设置`
      : '未发现第三方代理，系统代理已恢复为未设置';
    toast(cleanedNotice.value, 'success');
    await refreshForeign();
  } catch (e) {
    toast('清理失败：' + e, 'error');
  } finally {
    cleaning.value = false;
  }
}
onMounted(refreshForeign);
</script>
