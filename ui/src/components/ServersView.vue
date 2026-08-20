<template>
  <div class="page">
    <header class="page-head"><div><h1>云服务器</h1><p>管理代理节点与 SSH 连接信息</p></div></header>
    <div class="server-grid">
      <div class="server-card" v-for="(node, i) in (config?.nodes || [])" :key="i">
        <div class="server-top">
          <div class="server-avatar">云</div>
          <div><div class="server-name">{{ node.name }}</div><div class="muted">{{ node.server }}:{{ node.port }}</div></div>
          <span class="status-pill" :class="{ ok: i === 0 }">{{ i === 0 ? '默认' : '备用' }}</span>
        </div>
        <div class="kv"><span>协议</span><b>VLESS + Reality</b></div>
        <div class="kv"><span>UUID</span><b class="mono">{{ node.uuid }}</b></div>
        <div class="kv"><span>Public Key</span><b class="mono">{{ node.publicKey }}</b></div>
        <div class="kv"><span>Short ID</span><b class="mono">{{ node.shortId }}</b></div>
      </div>
    </div>
    <section class="panel">
      <div class="panel-head"><h2>SSH 控制</h2><button class="btn" @click="$emit('nav')">打开控制台</button></div>
      <p class="muted">通过 SSH 直接控制云服务器：执行命令、安装软件、查看日志。</p>
    </section>
  </div>
</template>
<script setup>
defineProps({ config: Object, status: Object });
defineEmits(['nav']);
</script>
