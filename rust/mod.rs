use anyhow::{anyhow, Result};
use chrono::Local;
use daemonize::Daemonize;
use nix::sys::signal::{self, SigSet, SigmaskHow, Signal};
use nix::sys::signalfd;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Daemon {
    pid_file: String,
    log_file: String,
    logger: File,
    signal_fd: Option<signalfd::SignalFd>,
}

impl Daemon {
    pub fn new(pid_file: &str, log_file: &str) -> Result<Self> {
        let logger = Daemon::start_daemon(pid_file, log_file);
        let signal_fd = Daemon::setup_signal_handler();
        Ok(Self {
            pid_file: pid_file.to_string(),
            log_file: log_file.to_string(),
            logger,
            signal_fd ,
        })
    }

    pub fn run() -> Result<()> {
        daemon.start_tmux_session("Initial")?;
        daemon.main_loop()
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
        let mut mask = SigSet::empty();
        mask.add(Signal::SIGUSR1);

        signal::sigprocmask(SigmaskHow::SIG_BLOCK, Some(&mask), None)?;

        let signal_fd = signalfd::SignalFd::with_flags(&mask, signalfd::SfdFlags::SFD_CLOEXEC)
            .map_err(|e| anyhow!("Failed to create signal fd: {}", e))?;
        Ok(singal_fd)
    }

    fn main_loop(&mut self) -> Result<()> {
        loop {
            match self.signal_fd.as_mut().unwrap().read_signal() {
                Ok(Some(info)) => {
                    if info.ssi_signo == Signal::SIGUSR1 as u32 {
                        let sess_id = format!("SIGUSR1 {}", Local::now());
                        self.start_tmux_session(&sess_id)?;
                    }
                }
                Ok(None) => self.log_message("Received empty signal")?,
                Err(e) => return Err(anyhow!("Error reading signal: {}", e)),
            }
        }
    }

    fn count_detached_sessions(&self) -> Result<usize> {
        let output = Command::new("sh")
            .arg("-c")
            .arg("tmux list-sessions | grep -v 'attached' | wc -l")
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("Failed to count detached tmux sessions"));
        }

        let count_str = String::from_utf8(output.stdout)?.trim().to_string();
        count_str.parse::<usize>().map_err(|e| anyhow!(e))
    }

    fn start_tmux_session(&self, sess_id: &str) -> Result<()> {
        let count = self.count_detached_sessions()?;
        if count >= 2 {
            self.log_message(&format!(
                "Skipping session creation: already {count} detached sessions"
            ))?;
            return Ok(());
        }

        std::thread::sleep(Duration::from_secs(1));

        Command::new("tmux").arg("new-session").arg("-d").status()?;

        self.log_message(&format!(
            "Started tmux session at {}: {}",
            Local::now(),
            sess_id
        ))?;
        Ok(())
    }

    fn log_message(&self, message: &str) -> Result<()> {
        writeln!(self.file, "{}", message)?;
        Ok(())
    }
}

fn main() -> Result<(), anyhow::Error> {
    let daemon = Daemon::new("/tmp/tmux_warm_daemon.pid", "/tmp/tmux_warm_daemon.log")?;
    daemon.run()?;
}
