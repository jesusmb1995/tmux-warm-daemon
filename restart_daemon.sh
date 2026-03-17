#!/usr/bin/env bash
set -euo pipefail

PIDFILE="/tmp/tmux_warm_daemon.pid"
BINARY="$HOME/.tmux_warm_daemon/rust/target/release/tmux_warm_daemon"

pid=$(cat "$PIDFILE" 2>/dev/null || true)
if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
  kill "$pid"
  echo "Killed old daemon (PID $pid)"
else
  echo "No running daemon found"
fi

"$BINARY"
sleep 0.2
new_pid=$(cat "$PIDFILE" 2>/dev/null || true)
echo "Started new daemon (PID $new_pid)"
