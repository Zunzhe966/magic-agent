#!/bin/bash
# 魔法代理 MCP HTTP 桥接守护脚本
# 用法: magic-agent-mcp.sh {start|stop|status|restart}
# 拉起 mcp/server.py --http 19092，让 WorkBuddy 等 HTTP MCP 客户端接入。
# 用 launchd（scripts 同级的 plist）或手动 start 常驻。

PORT=19092
# 自动定位 mcp/server.py：优先取脚本所在目录的上级（scripts/..），否则回退到 $HOME 下的项目路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER="${PROJECT_DIR}/mcp/server.py"
[ -f "$SERVER" ] || SERVER="${HOME}/Desktop/魔法代理/mcp/server.py"
PIDFILE="/tmp/magic-agent-mcp.pid"
LOG="/tmp/magic-agent-mcp.log"

is_running() {
  [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null
}

health() {
  curl -s --noproxy '*' --max-time 2 "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1
}

start() {
  if is_running && health; then
    echo "已在运行 (PID $(cat "$PIDFILE"))"
    return 0
  fi
  # 清理僵尸 pidfile
  [ -f "$PIDFILE" ] && rm -f "$PIDFILE"
  nohup python3 "$SERVER" --http "$PORT" >>"$LOG" 2>&1 &
  echo $! > "$PIDFILE"
  # 等待就绪
  for _ in $(seq 1 20); do
    health && { echo "已启动 (PID $(cat "$PIDFILE"), http://127.0.0.1:${PORT}/mcp)"; return 0; }
    sleep 0.25
  done
  echo "启动失败，日志:" >&2
  tail -5 "$LOG" >&2
  return 1
}

stop() {
  if is_running; then
    kill "$(cat "$PIDFILE")" 2>/dev/null
    rm -f "$PIDFILE"
    echo "已停止"
  else
    echo "未在运行"
  fi
}

status() {
  if is_running && health; then
    echo "运行中 (PID $(cat "$PIDFILE"), http://127.0.0.1:${PORT}/mcp)"
  else
    echo "未运行"
  fi
}

case "$1" in
  start) start ;;
  stop) stop ;;
  restart) stop; sleep 0.5; start ;;
  status) status ;;
  *) echo "用法: $0 {start|stop|status|restart}"; exit 1 ;;
esac
