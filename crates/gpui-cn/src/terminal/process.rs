//! The processes behind a local terminal.
//!
//! A shell in a pty leads its own session, and every job it starts stays in
//! that session, even one in another process group. Closing the terminal
//! ends them all, as herdr does: SIGHUP, which a shell passes on to its
//! jobs, then SIGTERM, then SIGKILL, each to every process left in the
//! session, with up to 250 ms between steps.

use std::time::{Duration, Instant};

/// How long each signal gets before the next, stronger one.
const STEP: Duration = Duration::from_millis(250);

/// Ends every process in the session `leader` leads. `reap` collects the
/// leader's exit when it has one, since a zombie still answers a signal
/// probe; it returns whether the leader has exited.
#[cfg(unix)]
pub(crate) fn end_session(leader: libc::pid_t, mut reap: impl FnMut() -> bool) {
    for signal in [libc::SIGHUP, libc::SIGTERM, libc::SIGKILL] {
        let pids = session(leader);
        if pids.is_empty() {
            return;
        }
        for pid in &pids {
            // SAFETY: kill only sends a signal; a pid that is gone fails.
            unsafe { libc::kill(*pid, signal) };
        }
        let deadline = Instant::now() + STEP;
        loop {
            reap();
            if session(leader).is_empty() || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    reap();
}

/// The live processes in session `sid`, the leader included while it
/// lives.
#[cfg(unix)]
pub(crate) fn session(sid: libc::pid_t) -> Vec<libc::pid_t> {
    all_pids()
        .into_iter()
        .filter(|pid| {
            // SAFETY: getsid only reads the process table.
            unsafe { libc::getsid(*pid) == sid }
        })
        .filter(|pid| alive(*pid))
        .collect()
}

/// Whether `pid` is running: it exists and is not a zombie.
#[cfg(unix)]
fn alive(pid: libc::pid_t) -> bool {
    // SAFETY: a zero signal only probes the pid.
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    !zombie(pid)
}

#[cfg(target_os = "macos")]
fn zombie(pid: libc::pid_t) -> bool {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: the buffer is a proc_bsdinfo of the size given.
    let read =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    // SZOMB is 5 in the BSD process states.
    read == size && info.pbi_status == 5
}

#[cfg(all(unix, not(target_os = "macos")))]
fn zombie(pid: libc::pid_t) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            let state = stat.rsplit_once(')')?.1.trim_start().chars().next()?;
            Some(state == 'Z')
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn all_pids() -> Vec<libc::pid_t> {
    // SAFETY: a null buffer asks for the count.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if count <= 0 {
        return Vec::new();
    }
    // Room for processes started since the count.
    let mut pids = vec![0 as libc::pid_t; count as usize + 64];
    let bytes = (pids.len() * std::mem::size_of::<libc::pid_t>()) as libc::c_int;
    // SAFETY: the buffer holds `bytes` bytes of pids.
    let found = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    pids.truncate(found.max(0) as usize);
    pids.retain(|pid| *pid > 0);
    pids
}

#[cfg(all(unix, not(target_os = "macos")))]
fn all_pids() -> Vec<libc::pid_t> {
    std::fs::read_dir("/proc")
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::BufRead as _;
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};

    #[test]
    fn ending_a_session_ends_its_jobs_in_other_process_groups() {
        // A session leader, as a shell in a pty is, whose job runs in its
        // own process group, as a shell with job control starts it.
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "set -m; sleep 300 & echo $!; exec sleep 300"])
            .stdout(Stdio::piped());
        // SAFETY: setsid is async-signal-safe.
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        let leader = child.id() as libc::pid_t;
        let mut line = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let job: libc::pid_t = line.trim().parse().unwrap();
        let pids = session(leader);
        assert!(pids.contains(&leader) && pids.contains(&job), "{pids:?}");
        // SAFETY: getpgid only reads the process table.
        assert_ne!(
            unsafe { libc::getpgid(job) },
            leader,
            "the job has its own process group"
        );

        end_session(leader, || matches!(child.try_wait(), Ok(Some(_))));
        assert!(session(leader).is_empty(), "the whole session ended");
        assert!(!alive(job));
    }
}
