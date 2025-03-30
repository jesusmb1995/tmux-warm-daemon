"""
Creates detached tmux sessions when signal is received.
"""

import os
import time
import signal
import subprocess
import daemon
import lockfile

# Configuration
PID_FILE = "/tmp/tmux_warm_daemon.pid"
LOG_FILE = "/tmp/tmux_warm_daemon.log"


def count_detached_sessions():
    """
    Use the command line to find number of detched sessions
    """
    try:
        output = subprocess.check_output(
            "tmux list-sessions | grep -v 'attached' | wc -l",
            shell=True,
            text=True
        )
        return int(output.strip())
    except subprocess.CalledProcessError:
        return 0

def start_tmux_session(sess_id):
    """
    Function to start a detached tmux session
    """
    detached_count = count_detached_sessions()
    if detached_count >= 2:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(f"Skipping session creation: already {detached_count} detached sessions\n")
        return
    time.sleep(1)
    subprocess.run(["tmux", "new-session", "-d"], check=True)
    with open(LOG_FILE, "a", encoding="utf-8") as f:
        f.write(f"Started tmux session at {time.ctime()}: {sess_id}\n")


def handle_usr1_signal(signum, frame):
    """
    Signal handler for USR1
    """
    sess_id = str(signum) + str(frame)
    start_tmux_session(sess_id)


def run_daemon():
    """
    Daemon main function
    """
    with open(PID_FILE, "w", encoding="ascii") as pid_file:
        pid_file.write(str(os.getpid()))

    # Register signal handler for SIGUSR1
    signal.signal(signal.SIGUSR1, handle_usr1_signal)

    # Initial check: ensure a warm session exists at startup
    start_tmux_session("Initial")

    # Wait indefinitely for signals
    while True:
        signal.pause()  # Suspends execution until a signal is received


if __name__ == "__main__":
    # Run as a daemon
    ENCODING = "utf-8"
    with daemon.DaemonContext(
        working_directory="/tmp",
        umask=0o002,
        pidfile=lockfile.FileLock(PID_FILE),
        stdout=open(LOG_FILE, "a", encoding=ENCODING),
        stderr=open(LOG_FILE, "a", encoding=ENCODING),
    ):
        run_daemon()
