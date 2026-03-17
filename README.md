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
  Generic sessions are named `agent-0`, `agent-1`, etc.
  Workspace sessions are named `agent@<hash>` (see below).

Pool options:
- `max_detached` — how many generic detached sessions to keep warm (default: 2, set to 0 to disable)
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

Apply mod to tmux plugin:
```
tmux_warm_daemon_dir="$(pwd)"
(cd "${HOME}/.oh-my-zsh/plugins/tmux" && git apply "${tmux_warm_daemon_dir}/tmux.plugin.zsh.diff")
```

The plugin patch adds two config variables:
- `ZSH_TMUX_CD` — send `cd` to the attached session to match the launching terminal's pwd
- `ZSH_TMUX_WARM_SESSION_PREFIX` — when set, auto-attach targets a detached session
  whose name starts with this prefix (e.g. `"warm"` matches `warm-0`, `warm-1`),
  preventing accidental attachment to sessions from other pools like `agent-*`

Set up tmux plugin in `.zshrc`:

```zsh
plugins=(git tmux zshmarks)

if [ -z "$ZSH_TMUX_AUTOSTART" ]; then
	export ZSH_TMUX_AUTOSTART=true
fi

export ZSH_TMUX_AUTOCONNECT=true
export ZSH_TMUX_CD=true
export ZSH_TMUX_WARM_SESSION_PREFIX="warm"

if [ -z "$TMUX" ]; then
  export TMUX_WARM_DAEMON=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null)
  ps -p ${TMUX_WARM_DAEMON:-0} > /dev/null 2>&1
  if [ $? -ne 0  ]; then
    $HOME/.tmux_warm_daemon/rust/target/release/tmux_warm_daemon
    export TMUX_WARM_DAEMON=$(cat /tmp/tmux_warm_daemon.pid 2>/dev/null)
  fi

  export TMUX_PREATTACH_PATH="$(pwd)"
  kill -USR1 ${TMUX_WARM_DAEMON}
fi

source $ZSH/oh-my-zsh.sh
```

## Workspace-specific sessions

For any pool named `<pool>`, the daemon automatically checks
`/tmp/tmux_warm_<pool>_workspaces.json` on every SIGUSR1. If the file exists,
it creates a session per workspace path. The JSON file is a plain array of
absolute paths:

```json
["/home/user/project-alpha", "/home/user/project-beta"]
```

For example, writing the above to `/tmp/tmux_warm_agent_workspaces.json` creates:
- `agent@<8-char md5 hash>` running `agent --workspace /home/user/project-alpha`
- `agent@<8-char md5 hash>` running `agent --workspace /home/user/project-beta`

These coexist with generic `agent-0` sessions (controlled by `max_detached`).
Set `max_detached: 0` to only use workspace-specific sessions.

The JSON file can be managed by external tooling (IDE hooks, scripts, cron).

## Attaching to non-default pools

Use `attach_warm.sh` to attach to a pre-warmed session from any pool:

```bash
# Usage: attach_warm.sh [pool_name] [init_cmd] [workspace_path]

# Attach to a generic agent session
bash attach_warm.sh agent

# Attach with workspace — tries agent@<hash> first, falls back to generic
bash attach_warm.sh agent "agent --workspace /my/project" /my/project

# Shell pool — cd into current directory
bash attach_warm.sh warm "cd $PWD"

# Works from inside tmux too (uses switch-client)
```

Or add shell aliases:

```bash
# Agent pool — prefer workspace session, fall back to generic
alias wa='bash $HOME/.tmux_warm_daemon/attach_warm.sh agent "" "$PWD"'
# Shell pool — cd into current directory
alias ww='bash $HOME/.tmux_warm_daemon/attach_warm.sh warm "cd $PWD"'
```
