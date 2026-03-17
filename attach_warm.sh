#!/usr/bin/env bash
# Attach to a pre-warmed tmux session from a named pool.
# Usage: attach_warm.sh [pool_name] [init_cmd] [workspace_path]
#   pool_name      defaults to "agent"
#   init_cmd       sent to the session before attaching (skipped if empty)
#                  defaults to "cd <current working directory>"
#   workspace_path if provided, try workspace-specific session (pool@hash) first

set -euo pipefail

pool="${1:-agent}"
init_cmd="${2-cd $(printf '%q' "$PWD")}"
workspace="${3:-}"

session=""

if [ -n "$workspace" ]; then
  if [ ! -f "$workspace/.was_agent" ]; then
    touch "$workspace/.was_agent"
  fi

  ws_file="/tmp/tmux_warm_${pool}_workspaces.json"
  if [ -f "$ws_file" ]; then
    if ! grep -qF "\"$workspace\"" "$ws_file" 2>/dev/null; then
      tmp=$(mktemp)
      python3 -c "
import json,sys
ws=json.load(open('$ws_file'))
ws.append('$workspace')
json.dump(ws,open('$tmp','w'))
" 2>/dev/null && mv "$tmp" "$ws_file" || rm -f "$tmp"
    fi
  else
    printf '["%s"]\n' "$workspace" > "$ws_file"
  fi

  # Try workspace-specific session
  hash=$(printf '%s' "$workspace" | md5sum | cut -c1-8)
  ws_session="${pool}@${hash}"
  if tmux has-session -t "$ws_session" 2>/dev/null; then
    echo "Found session for ${hash}" >&2
    # Session exists — use it if detached
    session=$(tmux list-sessions -F '#{session_name} #{session_attached}' 2>/dev/null \
      | awk -v name="${ws_session}" '$1 == name && $2 == 0 { print $1; exit }')
  else
    echo "No warm session for workspace '${workspace}' (${hash}), signalling daemon" >&2
    pid=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null || true)
    [ -n "$pid" ] && kill -USR1 "$pid" 2>/dev/null || true
    exit 1
  fi
  if [ -n "$session" ]; then
    init_cmd=""
  fi
fi

# Fall back to any detached generic pool session (no workspace requested)
if [ -z "$session" ]; then
  session=$(tmux list-sessions -F '#{session_name} #{session_attached}' 2>/dev/null \
    | awk -v pfx="${pool}-" '$1 ~ "^"pfx && $2 == 0 { print $1; exit }')
fi

if [ -z "$session" ]; then
  echo "No warm session available for pool '${pool}'" >&2
  exit 1
fi

pid=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null || true)

signal_daemon() {
  [ -n "$pid" ] && (sleep 1 && kill -USR1 "$pid" 2>/dev/null) &
}

if [ -n "$init_cmd" ]; then
  tmux send-keys -t "$session" C-c
  sleep 0.1
  tmux send-keys -t "$session" "$init_cmd" Enter
fi

if [ -t 1 ]; then
  if [ -n "${TMUX:-}" ]; then
    tmux switch-client -t "$session"
    signal_daemon
  else
    signal_daemon
    tmux attach -t "$session"
  fi
else
  # Non-interactive (e.g. vim.fn.system): print session name
  echo "$session"
  signal_daemon
fi
