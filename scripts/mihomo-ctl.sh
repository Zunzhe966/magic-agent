#!/bin/bash
# 网络管理内核控制器（由 install_privileged_helper 安装到 /usr/local/lib/magic-agent/，root 所有）
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
  # App 安装名可能是「尊者网络管理.app」，历史名「尊者魔法代理.app」/「魔法代理.app」也逐个探测（曾写死导致找不到）
  SRC=''
  for c in "/Applications/尊者网络管理.app/Contents/Resources/bin/mihomo" \
           "/Applications/尊者魔法代理.app/Contents/Resources/bin/mihomo" \
           "/Applications/魔法代理.app/Contents/Resources/bin/mihomo"; do
    if [ -f "$c" ]; then SRC="$c"; break; fi
  done
  if [ ! -f "$BIN" ]; then
    mkdir -p "$RUNTIME/bin"
    if [ -n "$SRC" ]; then cp "$SRC" "$BIN"; fi
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
    # 日志权限：root umask 020 会把日志落成 0644，同机任何账号都能读用户全量连接记录。
    # umask 077 管新建，chmod 600 管已存在的旧文件与轮转出的 .old。
    umask 077
    for f in "$LOG" "$ERR"; do
      if [ -f "$f" ]; then
        if [ "$(stat -f %z "$f")" -gt 10485760 ]; then mv -f "$f" "$f.old"; chmod 600 "$f.old"; else chmod 600 "$f"; fi
      fi
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
