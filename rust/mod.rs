
use anyhow::{anyhow, Result};
use daemonize::Daemonize;
use nix::sys::signal::{self, SigSet, SigmaskHow, Signal};
use nix::sys::signalfd;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::time::Duration;
use chrono::Local;

fn main() -> Result<()> {
    start_daemon()?;
    let mut signal_fd = setup_signal_handler()?;
    start_tmux_session("Initial")?;
    main_loop(&mut signal_fd)
}

fn start_daemon() -> Result<()> {
    let stdout = File::create("/tmp/tmux_warm_daemon.log")?;
    let stderr = File::create("/tmp/tmux_warm_daemon.log")?;

    let daemonize = Daemonize::new()
        .working_directory("/tmp")
        .umask(0o002)
        .pid_file("/tmp/tmux_warm_daemon.pid")
        .stdout(stdout)
        .stderr(stderr);

    daemonize.start()
}

fn setup_signal_handler() -> Result<signalfd::SignalFd> {
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGUSR1);

    signal::sigprocmask(SigmaskHow::SIG_BLOCK, Some(&mask), None)?;

    signalfd::SignalFd::with_flags(&mask, signalfd::SfdFlags::SFD_CLOEXEC)
        .map_err(|e| anyhow!("Failed to create signal fd: {}", e))
}

fn main_loop(signal_fd: &mut signalfd::SignalFd) -> Result<()> {
    loop {
        match signal_fd.read_signal() {
            Ok(Some(info)) => {
                if info.ssi_signo == Signal::SIGUSR1 as u32 {
                    let sess_id = format!("SIGUSR1 {}", Local::now());
                    start_tmux_session(&sess_id)?;
                }
            }
            Ok(None) => {}
            Err(e) => return Err(anyhow!("Error reading signal: {}", e)),
        }
    }
}

fn count_detached_sessions() -> Result<usize> {
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

fn start_tmux_session(sess_id: &str) -> Result<()> {
    let count = count_detached_sessions()?;
    if count >= 2 {
        log_message(&format!("Skipping session creation: already {count} detached sessions"))?;
        return Ok(());
    }

    // The sleep allows terminals that trigger a call to this daemon
    // to have time to attach to existing sessions.
    std::thread::sleep(Duration::from_secs(1));

    Command::new("tmux")
        .arg("new-session")
        .arg("-d")
        .status()?;

    log_message(&format!("Started tmux session at {}: {}", Local::now(), sess_id))?;
    Ok(())
}

fn log_message(message: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("/tmp/tmux_warm_daemon.log")?;
    writeln!(file, "{message}")?;
    Ok(())
}
