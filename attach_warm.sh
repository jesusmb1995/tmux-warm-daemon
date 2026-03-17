#!/usr/bin/env bash
# Attach to a pre-warmed tmux session from a named pool.
# Usage: attach_warm.sh [pool_name]
#   pool_name defaults to "agent"

set -euo pipefail

pool="${1:-agent}"
session=$(tmux list-sessions -F '#{session_name} #{session_attached}' 2>/dev/null \
  | awk -v pfx="${pool}-" '$1 ~ "^"pfx && $2 == 0 { print $1; exit }')

if [ -z "$session" ]; then
  echo "No warm session available for pool '${pool}'" >&2
  exit 1
fi

pid=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null || true)
[ -n "$pid" ] && kill -USR1 "$pid" 2>/dev/null || true

if [ -n "${TMUX:-}" ]; then
  tmux switch-client -t "$session"
else
  tmux attach -t "$session"
fi
