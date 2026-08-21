<template>
  <div class="page">
    <header class="page-head">
      <div><h1>软件分流</h1><p>按软件精确控制流量走向，Chrome 走代理、Safari 直连</p></div>
      <div class="head-actions">
        <button class="btn" @click="$emit('refresh')">重新扫描</button>
        <button class="btn primary" @click="save">保存并应用</button>
      </div>
    </header>
    <div class="toolbar">
      <input class="search" v-model="q" placeholder="搜索软件…" />
      <div class="seg">
        <button v-for="m in modes" :key="m.value" class="seg-btn" :class="{ on: filter === m.value }" @click="filter = m.value">{{ m.label }}</button>
      </div>
    </div>
    <p class="muted" style="margin-bottom: 10px;">默认策略：所有软件直连。只有你在下面明确选为「代理」的软件才会走代理。</p>
    <div class="apps-table">
      <div class="apps-row head">
        <span>软件</span><span>类别</span><span>模式</span><span>当前连接</span><span>路径</span>
      </div>
      <div class="apps-row" v-for="app in filtered" :key="app.id">
        <div class="app-name">
          <div class="app-avatar">{{ app.name.slice(0, 1) }}</div>
          <div><div>{{ app.name }}</div><div class="muted">{{ app.running ? '运行中' : '未运行' }}</div></div>
        </div>
        <span class="tag">{{ app.category }}</span>
        <select class="mode-select" v-model="app.mode">
          <option value="proxy">代理</option><option value="direct">直连</option>
        </select>
        <span class="conn-status" :class="connClass(app)">{{ connText(app) }}</span>
        <span class="path muted">{{ app.path }}</span>
      </div>
    </div>
  </div>
</template>
<script setup>
import { ref, computed } from 'vue';
const props = defineProps({ apps: Array });
const emit = defineEmits(['change', 'refresh']);
const q = ref('');
const filter = ref('all');
const modes = [
  { value: 'all', label: '全部' }, { value: 'proxy', label: '走代理' }, { value: 'direct', label: '直连' }
];
const filtered = computed(() => {
  const query = q.value.toLowerCase();
  return props.apps.filter(a => (!query || a.name.toLowerCase().includes(query)) && (filter.value === 'all' || a.mode === filter.value));
});
function connText(app) {
  if (!app.running) return '未运行';
  if (!app.confirmed) return '默认直连';
  if (app.mode === 'proxy') return app.node ? ('代理：' + app.node) : '代理';
  return '直连';
}
function connClass(app) {
  if (!app.running) return 'idle';
  if (!app.confirmed) return 'direct';
  if (app.mode === 'proxy') return 'proxy';
  return 'direct';
}
function save() {
  emit('change', props.apps);
}
</script>
