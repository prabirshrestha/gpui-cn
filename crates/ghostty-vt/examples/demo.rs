//! Spawn a shell in a pty, run a command through the engine and print the
//! rendered screen and the queued events.
//!
//!     cargo run -p ghostty-vt --example demo -- 'ls --color=always'

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ghostty_vt::render::{CellIterator, RenderState, RowIterator};
use ghostty_vt::{Event, Terminal, TerminalOptions};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ls -la".to_owned());
    let (cols, rows) = (100u16, 30u16);

    let mut terminal = Terminal::new(TerminalOptions {
        cols,
        rows,
        ..Default::default()
    })?;
    terminal.resize(cols, rows, 8, 16)?;
    terminal.set_terminfo_name(Some(ghostty_vt::terminfo::TERM))?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let pty = native_pty_system().openpty(PtySize {
        rows,
        cols,
        pixel_width: 800,
        pixel_height: 480,
    })?;
    let mut cmd = CommandBuilder::new(&shell);
    cmd.args(["-c", &command]);
    cmd.env("TERM", ghostty_vt::terminfo::TERM);
    cmd.env("TERMINFO", ghostty_vt::terminfo::dir()?);
    cmd.env(
        "GHOSTTY_RESOURCES_DIR",
        ghostty_vt::shell_integration::resources_dir()?,
    );
    let mut child = pty.slave.spawn_command(cmd)?;
    drop(pty.slave);

    let mut reader = pty.master.try_clone_reader()?;
    let mut writer = pty.master.take_writer()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut bytes = 0usize;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                bytes += chunk.len();
                terminal.vt_write(&chunk);
                // Answer queries the program sends, such as device attributes.
                for event in terminal.events() {
                    match event {
                        Event::PtyWrite(reply) => writer.write_all(&reply)?,
                        other => println!("event: {other:?}"),
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.try_wait()?.is_some() || Instant::now() > deadline {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();

    let mut state = RenderState::new()?;
    let mut row_iter = RowIterator::new()?;
    let mut cell_iter = CellIterator::new()?;
    let snapshot = state.update(&terminal)?;
    let colors = snapshot.colors()?;
    println!(
        "{shell} -c {command:?}: {bytes} bytes, {}x{} cells, fg #{:02x}{:02x}{:02x}",
        snapshot.cols()?,
        snapshot.rows()?,
        colors.foreground.r,
        colors.foreground.g,
        colors.foreground.b
    );
    println!("+{}+", "-".repeat(cols as usize));
    let mut rows_iter = row_iter.update(&snapshot)?;
    while let Some(row) = rows_iter.next() {
        let mut line = String::new();
        let mut cells = cell_iter.update(row)?;
        while let Some(cell) = cells.next() {
            let before = line.len();
            cell.graphemes_utf8(&mut line)?;
            if line.len() == before {
                line.push(' ');
            }
        }
        println!("|{line}|");
    }
    println!("+{}+", "-".repeat(cols as usize));
    Ok(())
}
