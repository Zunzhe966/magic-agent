// 轻量全局 toast：替代原生 alert，避免阻塞与体验不一致。
let container = null;

function ensureContainer() {
  if (container) return container;
  container = document.createElement('div');
  container.className = 'toast-container';
  document.body.appendChild(container);
  return container;
}

export function toast(message, type = 'info') {
  const c = ensureContainer();
  const el = document.createElement('div');
  el.className = `toast toast-${type}`;
  el.textContent = message;
  c.appendChild(el);
  // 触发进入动画
  requestAnimationFrame(() => el.classList.add('toast-show'));
  // 3 秒后淡出并移除
  setTimeout(() => {
    el.classList.remove('toast-show');
    el.classList.add('toast-hide');
    setTimeout(() => el.remove(), 300);
  }, 3000);
}
