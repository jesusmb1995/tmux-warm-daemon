Pre-allocated sessions of TMUX for ultra-fast zsh terminal start-up.

Sessions of tmux not explicitely closed with `exit` will be available for later use. When starting a re-used terminal `cd` is automatically applied into the curret pwd.

## Installation

Build Rust backend:
```
cd rust && cargo build --release
```

Apply mod to tmux plugin (originally based on `ef96242b9baad6b2211c386cb9af9418ace5d876` upstream):
```
tmux_warm_daemon_dir="$(pwd)"
(cd "${HOME}/.oh-my-zsh/plugins/tmux" && git apply "${tmux_warm_daemon_dir}/tmux.plugin.zsh.diff")
```

Set up tmux plugin in `.zshrc`, update `warm_path` to your cloned project directory:

```
plugins=(git tmux zshmarks)

if [ -z "$ZSH_TMUX_AUTOSTART" ]; then
	export ZSH_TMUX_AUTOSTART=true
fi

# Autoconnect to avoid startup time
export ZSH_TMUX_AUTOCONNECT=true

# Change directory to current after attaching
# This is a custom mod of plugin at `$HOME/.oh-my-zsh/plugins/tmux/tmux.plugin.zsh`
export ZSH_TMUX_CD=true

# Prepare new warm tmux session on the background 
# Check for TMUX to avoid infinite loop
if [ -z "$TMUX" ]; then
  export TMUX_WARM_DAEMON=$(cat /tmp/tmux_warm_daemon.pid)
  ps -p ${TMUX_WARM_DAEMON} > /dev/null 2>&1
  if [ $? -ne 0  ]; then
    warm_path="/path/to/repo/tmux_warm_daemon"

    # Make sure in the future sessions can benefit from the daemon
    #"${warm_path}/python/.venv/bin/python" "${warm_path}/python/tmux_warmer_daemon.py"
    "${warm_path}"/rust/target/release/tmux_warm_daemon
  fi
  
  export TMUX_PREATTACH_PATH="$(pwd)"
  kill -USR1 ${TMUX_WARM_DAEMON} 
fi

source $ZSH/oh-my-zsh.sh
```
