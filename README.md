Pre-allocated sessions of TMUX for ultra-fast zsh terminal start-up.

Sessions of tmux not explicitely closed with `exit` will be available for later use. When starting a re-used terminal `cd` is automatically applied into the curret pwd.

## Configuration

The daemon reads a YAML config from `~/.config/tmux_warm_daemon/config.yaml`:

```yaml
pid_file: /tmp/tmux_warm_daemon.pid
log_file: /tmp/tmux_warm_daemon.log

pools:
  warm:
    max_detached: 2
  agent:
    max_detached: 1
    command: agent
```

Each pool maintains its own set of pre-warmed tmux sessions:

- **`warm`** — default pool for regular shell sessions (no command, just zsh).
  Sessions are named `warm-0`, `warm-1`, etc.
- **`agent`** — pool with `agent` already started and waiting for input.
  Sessions are named `agent-0`, `agent-1`, etc.

Pool options:
- `max_detached` — how many detached (ready) sessions to keep warm (default: 2)
- `command` — optional command to run in the session (omit for a regular shell)

If no config file exists, the daemon falls back to a single `warm` pool with `max_detached: 2`.

A custom config path can be passed as a CLI argument:
```
tmux_warm_daemon /path/to/config.yaml
```

## Installation

Build Rust backend:
```
cd rust && cargo build --release
```

Install the binary (pick one):
```bash
# System-wide
sudo cp rust/target/release/tmux_warm_daemon /usr/bin/tmux_warm_daemon

# Or reference directly from .zshrc (no install needed):
#   $HOME/.tmux_warm_daemon/rust/target/release/tmux_warm_daemon
```

Apply mod to tmux plugin (originally based on `ef96242b9baad6b2211c386cb9af9418ace5d876` upstream):
```
tmux_warm_daemon_dir="$(pwd)"
(cd "${HOME}/.oh-my-zsh/plugins/tmux" && git apply "${tmux_warm_daemon_dir}/tmux.plugin.zsh.diff")
```

Set up tmux plugin in `.zshrc`:

```zsh
plugins=(git tmux zshmarks)

if [ -z "$ZSH_TMUX_AUTOSTART" ]; then
	export ZSH_TMUX_AUTOSTART=true
fi

export ZSH_TMUX_AUTOCONNECT=true
export ZSH_TMUX_CD=true

if [ -z "$TMUX" ]; then
  export TMUX_WARM_DAEMON=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null)
  ps -p ${TMUX_WARM_DAEMON:-0} > /dev/null 2>&1
  if [ $? -ne 0  ]; then
    $HOME/.tmux_warm_daemon/rust/target/release/tmux_warm_daemon
    export TMUX_WARM_DAEMON=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null)
  fi

  # Target a warm-pool session to avoid attaching to agent/other pool sessions
  _warm_session=$(tmux list-sessions -F '#{session_name} #{session_attached}' 2>/dev/null \
    | awk '$1 ~ /^warm-/ && $2 == 0 { print $1; exit }')
  [ -n "$_warm_session" ] && export ZSH_TMUX_DEFAULT_SESSION_NAME="$_warm_session"

  export TMUX_PREATTACH_PATH="$(pwd)"
  kill -USR1 ${TMUX_WARM_DAEMON}
fi

source $ZSH/oh-my-zsh.sh
```

## Attaching to non-default pools

Use `attach_warm.sh` to attach to a pre-warmed session from any pool:

```bash
# Attach to an agent session (default)
bash attach_warm.sh

# Attach to a specific pool
bash attach_warm.sh agent

# Works from inside tmux too (uses switch-client)
```

Or add a shell alias:

```bash
alias wa='bash $HOME/.tmux_warm_daemon/attach_warm.sh agent'
```
