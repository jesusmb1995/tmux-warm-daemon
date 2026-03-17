use anyhow::{anyhow, Result};
use chrono::Local;
use daemonize::Daemonize;
use nix::sys::signal::{self, SigSet, SigmaskHow, Signal};
use nix::sys::signalfd;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::time::Duration;

#[derive(Deserialize)]
struct PoolConfig {
    #[serde(default = "default_max_detached")]
    max_detached: usize,
    command: Option<String>,
}

fn default_max_detached() -> usize {
    2
}

#[derive(Deserialize)]
struct Config {
    #[serde(default = "default_pid_file")]
    pid_file: String,
    #[serde(default = "default_log_file")]
    log_file: String,
    pools: HashMap<String, PoolConfig>,
}

fn default_pid_file() -> String {
    "/tmp/tmux_warm_daemon.pid".into()
}
fn default_log_file() -> String {
    "/tmp/tmux_warm_daemon.log".into()
}

impl Default for Config {
    fn default() -> Self {
        let mut pools = HashMap::new();
        pools.insert(
            "warm".into(),
            PoolConfig {
                max_detached: 2,
                command: None,
            },
        );
        Config {
            pid_file: default_pid_file(),
            log_file: default_log_file(),
            pools,
        }
    }
}

impl Config {
    fn load(path: &str) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}

struct Daemon {
    config: Config,
    logger: File,
    signal_fd: signalfd::SignalFd,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Self> {
        let logger = Self::start_daemon(&config.pid_file, &config.log_file)?;
        let signal_fd = Self::setup_signal_handler()?;
        Ok(Self {
            config,
            logger,
            signal_fd,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.ensure_all_pools("startup")?;
        self.main_loop()
    }

    fn start_daemon(pid_file: &str, log_file: &str) -> Result<File> {
        let stdout = File::create(log_file)?;
        let stderr = File::create(log_file)?;

        let daemonize = Daemonize::new()
            .working_directory("/tmp")
            .umask(0o002)
            .pid_file(pid_file)
            .stdout(stdout)
            .stderr(stderr);

        daemonize.start()?;

        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file)?;

        Ok(file)
    }

    fn setup_signal_handler() -> Result<signalfd::SignalFd> {
        let mask = {
            let mut build_mask = SigSet::empty();
            build_mask.add(Signal::SIGUSR1);
            build_mask
        };

        signal::sigprocmask(SigmaskHow::SIG_BLOCK, Some(&mask), None)?;

        let signal_fd = signalfd::SignalFd::with_flags(&mask, signalfd::SfdFlags::SFD_CLOEXEC)
            .map_err(|e| anyhow!("Failed to create signal fd: {}", e))?;
        Ok(signal_fd)
    }

    fn main_loop(&mut self) -> Result<()> {
        loop {
            match self.signal_fd.read_signal() {
                Ok(Some(info)) => {
                    if info.ssi_signo == Signal::SIGUSR1 as u32 {
                        let sess_id = format!("SIGUSR1 {}", Local::now());
                        self.ensure_all_pools(&sess_id)?;
                    }
                }
                Ok(None) => self.log("Received empty signal")?,
                Err(e) => return Err(anyhow!("Error reading signal: {}", e)),
            }
        }
    }

    fn ensure_all_pools(&self, trigger: &str) -> Result<()> {
        for (pool_name, pool_config) in &self.config.pools {
            self.ensure_pool(pool_name, pool_config, trigger)?;
        }
        Ok(())
    }

    fn existing_sessions() -> Result<Vec<(String, bool)>> {
        let output = Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name} #{session_attached}"])
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let result = String::from_utf8(output.stdout)?
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let name = parts.next()?.to_string();
                let attached = parts.next().map(|s| s != "0").unwrap_or(false);
                Some((name, attached))
            })
            .collect();
        Ok(result)
    }

    fn ensure_pool(&self, pool_name: &str, pool_config: &PoolConfig, trigger: &str) -> Result<()> {
        let ws_file = format!("/tmp/tmux_warm_{}_workspaces.json", pool_name);
        self.ensure_workspace_sessions(pool_name, pool_config, &ws_file, trigger)?;

        if pool_config.max_detached == 0 {
            return Ok(());
        }

        let sessions = Self::existing_sessions()?;
        let prefix = format!("{}-", pool_name);

        let detached_count = sessions
            .iter()
            .filter(|(name, attached)| name.starts_with(&prefix) && !attached)
            .count();

        if detached_count >= pool_config.max_detached {
            self.log(&format!(
                "[{}] Skipping: already {} detached session(s)",
                pool_name, detached_count
            ))?;
            return Ok(());
        }

        let existing_names: HashSet<&str> = sessions.iter().map(|(n, _)| n.as_str()).collect();
        let session_name = (0..)
            .map(|i| format!("{}-{}", pool_name, i))
            .find(|name| !existing_names.contains(name.as_str()))
            .unwrap();

        std::thread::sleep(Duration::from_secs(1));

        let mut cmd = Command::new("tmux");
        cmd.args(["new-session", "-d", "-s", &session_name]);
        if let Some(ref command) = pool_config.command {
            cmd.arg(command);
        }
        cmd.status()?;

        self.log(&format!(
            "[{}] Created session '{}' (trigger: {})",
            pool_name, session_name, trigger
        ))?;

        Ok(())
    }

    fn ensure_workspace_sessions(
        &self,
        pool_name: &str,
        pool_config: &PoolConfig,
        ws_file: &str,
        trigger: &str,
    ) -> Result<()> {
        let workspaces: Vec<String> = match fs::read_to_string(ws_file) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(ws) => ws,
                Err(e) => {
                    self.log(&format!(
                        "[{}] Invalid JSON in '{}': {}",
                        pool_name, ws_file, e
                    ))?;
                    return Ok(());
                }
            },
            Err(_) => return Ok(()),
        };

        let sessions = Self::existing_sessions()?;
        let existing_names: HashSet<String> =
            sessions.into_iter().map(|(name, _)| name).collect();

        for workspace in &workspaces {
            let hash = Self::path_hash(workspace)?;
            let session_name = format!("{}@{}", pool_name, hash);

            if existing_names.contains(&session_name) {
                continue;
            }

            std::thread::sleep(Duration::from_secs(1));

            let mut cmd = Command::new("tmux");
            cmd.args(["new-session", "-d", "-s", &session_name, "-c", workspace]);

            if let Some(ref command) = pool_config.command {
                cmd.arg(format!(
                    "{} --workspace {}",
                    command,
                    Self::shell_quote(workspace)
                ));
            }

            cmd.status()?;

            self.log(&format!(
                "[{}] Created workspace session '{}' for '{}' (trigger: {})",
                pool_name, session_name, workspace, trigger
            ))?;
        }

        Ok(())
    }

    fn path_hash(path: &str) -> Result<String> {
        use std::process::Stdio;
        let mut child = Command::new("md5sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to open stdin for md5sum"))?
            .write_all(path.as_bytes())?;

        let output = child.wait_with_output()?;
        let hash_str = String::from_utf8(output.stdout)?;
        Ok(hash_str.chars().take(8).collect())
    }

    fn shell_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }

    fn log(&self, message: &str) -> Result<()> {
        let mut writer = &self.logger;
        writeln!(writer, "{} {}", Local::now().format("%F %T"), message)?;
        Ok(())
    }
}

fn main() -> Result<(), anyhow::Error> {
    let config_path = std::env::args().nth(1).unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        format!("{}/.config/tmux_warm_daemon/config.yaml", home)
    });

    let config = if std::path::Path::new(&config_path).exists() {
        Config::load(&config_path)?
    } else {
        eprintln!("Config not found at {}, using defaults", config_path);
        Config::default()
    };

    let mut daemon = Daemon::new(config)?;
    daemon.run()?;
    Ok(())
}
