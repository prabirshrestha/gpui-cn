//! The terminal engine with a real pty. Skips when no POSIX shell is
//! available.

#![cfg(all(unix, feature = "ghostty"))]

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_cn::terminal::{
    Engine, EngineOptions, FrameSink, FrameSource, LocalPty, LocalTerminalOptions,
    ShellIntegration, StartOptions, TerminalSnapshot, TerminalStatus, Viewport,
};

fn shell() -> Option<&'static str> {
    ["/bin/sh", "/usr/bin/sh"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
}

/// Wait until a snapshot satisfies `done`, or fail with the last frame.
fn wait_for(
    sink: &FrameSink,
    wake: &gpui_kit::base::async_util::Receiver<()>,
    handle: &dyn gpui_cn::terminal::FrameHandle,
    done: impl Fn(&TerminalSnapshot) -> bool,
) -> Arc<TerminalSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last: Option<Arc<TerminalSnapshot>> = None;
    while Instant::now() < deadline {
        let _ = wake.recv_blocking_timeout(Duration::from_millis(50));
        if let Some(snapshot) = sink.take() {
            let hit = done(&snapshot);
            last = Some(snapshot);
            handle.request_frame();
            if hit {
                return last.unwrap();
            }
        }
    }
    panic!(
        "timed out; last frame:\n{}",
        last.map_or_else(String::new, |s| s.frame.text())
    );
}

trait RecvTimeout {
    fn recv_blocking_timeout(&self, timeout: Duration) -> bool;
}

impl RecvTimeout for gpui_kit::base::async_util::Receiver<()> {
    fn recv_blocking_timeout(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if self.try_recv().is_ok() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn a_resize_reaches_the_program_and_the_grid_together() {
    let Some(shell) = shell() else {
        eprintln!("skipping: no POSIX shell");
        return;
    };
    let options = LocalTerminalOptions::default()
        .with_program(shell, ["-c", "while true; do stty size; sleep 0.2; done"])
        .with_shell_integration(ShellIntegration::None);
    let (sink, wake) = FrameSink::new();
    let handle = Box::new(Engine::new(
        LocalPty::new(options),
        EngineOptions::default(),
    ))
    .start(
        sink.clone(),
        StartOptions::default().with_viewport(Viewport::new(60, 10, 8, 16).unwrap()),
    )
    .unwrap();
    let first = wait_for(&sink, &wake, handle.as_ref(), |s| {
        s.frame.text().contains("10 60")
    });
    assert_eq!(first.status, TerminalStatus::Live);

    handle.resize(Viewport::new(100, 30, 8, 16).unwrap());
    let resized = wait_for(&sink, &wake, handle.as_ref(), |s| {
        s.frame.text().contains("30 100")
    });
    assert_eq!(resized.frame.viewport.rows(), 30);
    assert_eq!(resized.frame.rows.len(), 30);

    handle.close();
    let exited = wait_for(&sink, &wake, handle.as_ref(), |s| {
        matches!(s.status, TerminalStatus::Exited(_))
    });
    assert!(matches!(exited.status, TerminalStatus::Exited(_)));
}

#[test]
fn closing_the_terminal_ends_the_jobs_its_shell_started() {
    let Some(shell) = shell() else {
        eprintln!("skipping: no POSIX shell");
        return;
    };
    // A job in its own process group, as a shell with job control starts it.
    let options = LocalTerminalOptions::default()
        .with_program(
            shell,
            ["-c", "set -m; sleep 300 & echo JOB $!; exec sleep 300"],
        )
        .with_shell_integration(ShellIntegration::None);
    let (sink, wake) = FrameSink::new();
    let handle = Box::new(Engine::new(
        LocalPty::new(options),
        EngineOptions::default(),
    ))
    .start(sink.clone(), StartOptions::default())
    .unwrap();
    let shown = wait_for(&sink, &wake, handle.as_ref(), |s| {
        s.frame.text().contains("JOB ")
    });
    let job: i32 = shown
        .frame
        .text()
        .split("JOB ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|pid| pid.parse().ok())
        .expect("the job's pid");
    let alive = |pid: i32| {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    };
    assert!(alive(job));
    handle.close();
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(job) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(job), "the job ended with the terminal");
}
