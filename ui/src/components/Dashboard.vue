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
    <!-- P1-1 网络体检：只读看清本机网络秩序现状，不改任何状态 -->
    <section class="panel">
      <div class="panel-head">
        <h2>网络体检</h2>
        <button class="btn small" :disabled="auditing" @click="runAudit">{{ auditing ? '体检中…' : '开始体检' }}</button>
      </div>
      <p class="muted">只读检查第三方代理、端口占用、崩溃残留、路由 / DNS / PAC，不会改动任何设置。</p>
      <template v-if="audit">
        <p class="muted">结论：<strong>{{ audit.summary }}</strong>（{{ auditTimeText }}）</p>
        <ul class="foreign-list" v-if="audit.foreignProcs.length || audit.portConflicts.length || audit.staleProxy.detected || audit.ownPortLanExposed.length || audit.pacEnabled === 'yes' || audit.openLedger">
          <li v-for="(p, i) in audit.foreignProcs" :key="'f' + i">第三方代理：{{ p }}</li>
          <li v-for="(c, i) in audit.portConflicts" :key="'c' + i">端口 {{ c.port }} 被 {{ c.holderCommand }} (PID {{ c.holderPid }}) 占用</li>
          <li v-for="(s, i) in audit.ownPortLanExposed" :key="'l' + i">端口 {{ s.port }} 对局域网暴露（{{ s.command }}）</li>
          <li v-if="audit.staleProxy.detected">系统代理残留：{{ audit.staleProxy.detail }}</li>
          <li v-if="audit.pacEnabled === 'yes'">PAC 自动代理已启用</li>
          <li v-if="audit.openLedger">未结接管账本：{{ audit.openLedger.reason }}（{{ audit.openLedger.entries }} 项在册）</li>
        </ul>
        <p class="muted" v-if="audit.degraded.length">⚠ 部分项目未能采集：{{ audit.degraded.join('、') }}</p>
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

// P1-1 网络体检（手动触发，内部最坏 ~10 秒）
const audit = ref(null);
const auditing = ref(false);
const auditTimeText = ref('');
async function runAudit() {
  if (auditing.value) return;
  auditing.value = true;
  try {
    audit.value = await invoke('audit_network');
    auditTimeText.value = new Date().toLocaleTimeString();
  } catch (e) {
    toast('体检失败：' + e, 'error');
  } finally {
    auditing.value = false;
  }
}
</script>
