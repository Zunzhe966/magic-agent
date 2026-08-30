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
        <button class="btn small" @click="$emit('toggle-system-proxy', !status?.systemProxy)">{{ status?.systemProxy ? '关闭系统代理' : '开启系统代理' }}</button>
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
        <div class="mode-pill">{{ config?.autoGlobal === 'global' ? '全局' : '智能' }}</div>
      </div>
      <div class="policy-row" v-if="config">
        <div class="policy-item">
          <div class="policy-dot green"></div>
          <div><strong>本机直连</strong><span>银行、本地服务、内网、系统软件</span></div>
        </div>
        <div class="policy-item">
          <div class="policy-dot blue"></div>
          <div><strong>自动分流</strong><span>按目标域名/GeoIP 判断</span></div>
        </div>
        <div class="policy-item">
          <div class="policy-dot orange"></div>
          <div><strong>指定软件代理</strong><span>Chrome、Telegram、AI 工具等</span></div>
        </div>
      </div>
    </section>
  </div>
</template>
<script setup>
const props = defineProps({
  status: Object,
  config: Object,
  proxyBusy: Boolean,
  proxyError: String,
});
const emit = defineEmits(['start', 'stop', 'toggle-system-proxy']);
</script>
