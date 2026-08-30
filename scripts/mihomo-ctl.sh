#!/bin/bash
# 魔法代理内核控制器（由 install_privileged_helper 安装到 /usr/local/lib/magic-agent/，root 所有）
# 经 /etc/sudoers.d/ 白名单免密调用，实现零弹窗启停。
# 以 sudo 运行（root），$HOME 是 /var/root，须从 SUDO_USER 反推真实用户 home，避免硬编码用户名。
REAL_USER="${SUDO_USER:-$(/usr/bin/stat -f '%Su' /dev/console)}"
[ -n "$REAL_USER" ] || REAL_USER="$(/usr/bin/logname 2>/dev/null)"
USER_HOME="$(/usr/bin/dscl . -read "/Users/$REAL_USER" NFSHomeDirectory 2>/dev/null | /usr/bin/awk '{print $2}')"
[ -n "$USER_HOME" ] || USER_HOME="/Users/$REAL_USER"
RUNTIME="$USER_HOME/Library/Application Support/magic-agent/runtime"
BIN="$RUNTIME/bin/mihomo"
CONF="$RUNTIME/mihomo.yaml"
LOG="$RUNTIME/mihomo.log"
ERR="$RUNTIME/mihomo.err.log"
# 精确匹配「本 App 特有的 runtime 常驻副本」内核路径。
# 绝不能用宽泛的 resources/bin/mihomo——FlClash/Clash Verge 等第三方代理的内核
# 常放在它们自己的 .app/Contents/Resources/ 下，用 resources 关键词会误杀它们。
# start 只启动 $BIN（runtime 副本），所以按 runtime 精确路径匹配即可。
PATTERN='magic-agent/runtime/bin/mihomo'

ensure_bin() {
  SRC='/Applications/魔法代理.app/Contents/Resources/bin/mihomo'
  if [ ! -f "$BIN" ]; then
    mkdir -p "$RUNTIME/bin"
    if [ -f "$SRC" ]; then cp "$SRC" "$BIN"; fi
    chmod 755 "$BIN" 2>/dev/null
  fi
}

case "$1" in
  start)
    if pgrep -f "$PATTERN" >/dev/null 2>&1; then
      pgrep -f "$PATTERN" | head -1
      exit 0
    fi
    ensure_bin
    [ -f "$BIN" ] || { echo "kernel-not-found"; exit 1; }
    # 日志轮转（超 10MB 归档 .old）
    for f in "$LOG" "$ERR"; do
      [ -f "$f" ] && [ "$(stat -f %z "$f")" -gt 10485760 ] && mv -f "$f" "$f.old"
    done
    "$BIN" -f "$CONF" -d "$RUNTIME" >> "$LOG" 2>> "$ERR" &
    echo $!
    ;;
  stop)
    pkill -f "$PATTERN" 2>/dev/null
    exit 0
    ;;
  reload)
    pkill -HUP -f "$PATTERN" 2>/dev/null
    exit 0
    ;;
  status)
    pgrep -f "$PATTERN" | head -1
    ;;
  *)
    echo "usage: $0 {start|stop|reload|status}"
    exit 1
    ;;
esac
