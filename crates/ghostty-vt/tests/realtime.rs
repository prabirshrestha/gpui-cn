//! Real-time tests through a real pty and a real shell.
//!
//! These skip when no POSIX shell is available, such as on Windows.

mod common;

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

fn shell() -> Option<&'static str> {
    ["/bin/sh", "/bin/bash", "/bin/zsh"]
        .into_iter()
        .find(|path| std::path::Path::new(path).exists())
}

struct Session {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    rx: mpsc::Receiver<Vec<u8>>,
    _master: Box<dyn portable_pty::MasterPty + Send>,
}

impl Session {
    fn spawn(shell: &str, terminfo: &std::path::Path) -> Session {
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 640,
                pixel_height: 384,
            })
            .expect("openpty");
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", ghostty_vt::terminfo::TERM);
        cmd.env("TERMINFO", terminfo);
        cmd.env("PS1", "$ ");
        cmd.env_remove("PROMPT_COMMAND");
        cmd.env_remove("ENV");
        cmd.env_remove("BASH_ENV");
        cmd.env("HOME", std::env::temp_dir());
        let child = pty.slave.spawn_command(cmd).expect("spawn shell");
        drop(pty.slave);
        let mut reader = pty.master.try_clone_reader().expect("reader");
        let writer = pty.master.take_writer().expect("writer");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Session {
            child,
            writer,
            rx,
            _master: pty.master,
        }
    }

    /// Read until `marker` appears or the deadline passes.
    fn read_until(&self, marker: &[u8], timeout: Duration) -> Vec<u8> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while !out.windows(marker.len()).any(|w| w == marker) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(bytes) => out.extend(bytes),
                Err(_) => break,
            }
        }
        out
    }

    fn run(&mut self, command: &str) -> String {
        // The marker is assembled by the shell so that the echo of the typed
        // command line does not contain it.
        let marker = format!("__done_{}__", command.len());
        writeln!(
            self.writer,
            "{command}; printf '__done_%s__\\n' {}",
            command.len()
        )
        .unwrap();
        let out = self.read_until(marker.as_bytes(), Duration::from_secs(10));
        String::from_utf8_lossy(&out).into_owned()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn cache_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ghostty-vt-realtime-{}", std::process::id()));
    // SAFETY: Set before any other thread in this test binary reads it.
    unsafe { std::env::set_var("GHOSTTY_VT_CACHE_DIR", &dir) };
    dir
}

#[test]
fn shell_sees_xterm_ghostty_terminfo_and_the_pty_size() {
    let Some(shell) = shell() else {
        eprintln!("skipping: no POSIX shell");
        return;
    };
    let _cache = cache_dir();
    let terminfo = ghostty_vt::terminfo::dir().unwrap();
    let mut session = Session::spawn(shell, &terminfo);
    session.run("true");

    let printf = session.run("printf 'x%sy' hello");
    assert!(printf.contains("xhelloy"), "{printf}");

    let size = session.run("stty size");
    assert!(size.contains("24 80"), "{size}");

    if std::path::Path::new("/usr/bin/tput").exists() {
        let colors = session.run("tput colors");
        assert!(colors.contains("256"), "{colors}");
    }

    let term = session.run("printf '%s' \"$TERM\"");
    assert!(term.contains("xterm-ghostty"), "{term}");
}

#[test]
fn idle_shell_produces_no_output_for_two_seconds() {
    let Some(shell) = shell() else {
        eprintln!("skipping: no POSIX shell");
        return;
    };
    let _cache = cache_dir();
    let terminfo = ghostty_vt::terminfo::dir().unwrap();
    let mut session = Session::spawn(shell, &terminfo);
    session.run("true");
    // Drain whatever the prompt printed.
    let _ = session.read_until(b"\0never\0", Duration::from_millis(300));

    let idle = session.read_until(b"\0never\0", Duration::from_secs(2));
    assert!(idle.is_empty(), "idle shell wrote {:?}", idle);

    // The terminal side is idle too: nothing to drain and nothing dirty.
    let mut terminal = common::terminal(80, 24);
    let mut state = ghostty_vt::RenderState::new().unwrap();
    state.update(&terminal).unwrap().clean().unwrap();
    assert!(!terminal.has_events());
    assert_eq!(
        state.update(&terminal).unwrap().dirty().unwrap(),
        ghostty_vt::render::Dirty::Clean
    );
    terminal.vt_write(&idle);
}
