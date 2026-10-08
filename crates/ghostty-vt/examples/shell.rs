//! An interactive shell in your console, driven by the engine.
//!
//! The pty output goes through `Terminal::vt_write`, the screen is painted
//! from the render state, and your keystrokes go to the pty unchanged (the
//! console you run this in has already encoded them). Exit by exiting the
//! shell.
//!
//!     cargo run -p ghostty-vt --example shell

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Duration;

use crossterm::{cursor, execute, queue, style, terminal};
use ghostty_vt::render::{CellIterator, CursorVisualStyle, RenderState, RowIterator};
use ghostty_vt::screen::CellWide;
use ghostty_vt::style::{RgbColor, Underline};
use ghostty_vt::{Event, Terminal, TerminalOptions};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

enum Input {
    Pty(Vec<u8>),
    Keys(Vec<u8>),
    Exit,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = terminal::size()?;
    let mut term = Terminal::new(TerminalOptions {
        cols,
        rows,
        ..Default::default()
    })?;
    term.resize(cols, rows, 8, 16)?;
    term.set_terminfo_name(Some(ghostty_vt::terminfo::TERM))?;
    term.set_xtversion(Some(format!("ghostty-vt {}", env!("CARGO_PKG_VERSION"))));

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let pty = native_pty_system().openpty(PtySize {
        rows,
        cols,
        pixel_width: u16::try_from(u32::from(cols) * 8).unwrap_or(u16::MAX),
        pixel_height: u16::try_from(u32::from(rows) * 16).unwrap_or(u16::MAX),
    })?;
    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-l");
    cmd.env("TERM", ghostty_vt::terminfo::TERM);
    cmd.env("TERMINFO", ghostty_vt::terminfo::dir()?);
    cmd.env(
        "GHOSTTY_RESOURCES_DIR",
        ghostty_vt::shell_integration::resources_dir()?,
    );
    cmd.env("TERM_PROGRAM", "ghostty-vt");
    let mut child = pty.slave.spawn_command(cmd)?;
    drop(pty.slave);
    let mut pty_reader = pty.master.try_clone_reader()?;
    let mut pty_writer = pty.master.take_writer()?;

    let (tx, rx) = mpsc::channel();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 65536];
            loop {
                match pty_reader.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        if tx.send(Input::Pty(buf[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                    _ => {
                        let _ = tx.send(Input::Exit);
                        return;
                    }
                }
            }
        });
    }
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 4096];
        while let Ok(n) = stdin.read(&mut buf) {
            if n == 0 || tx.send(Input::Keys(buf[..n].to_vec())).is_err() {
                return;
            }
        }
    });

    let mut out = std::io::stdout().lock();
    terminal::enable_raw_mode()?;
    execute!(
        out,
        terminal::EnterAlternateScreen,
        terminal::Clear(terminal::ClearType::All)
    )?;
    let result = run(&mut term, &mut out, &rx, &mut pty_writer, &mut child);
    let _ = execute!(
        out,
        style::ResetColor,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
    let _ = child.kill();
    result
}

fn run(
    term: &mut Terminal,
    out: &mut impl Write,
    rx: &mpsc::Receiver<Input>,
    pty_writer: &mut Box<dyn Write + Send>,
    child: &mut Box<dyn portable_pty::Child + Send + Sync>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = RenderState::new()?;
    let mut row_iter = RowIterator::new()?;
    let mut cell_iter = CellIterator::new()?;
    let mut title = String::new();
    let mut needs_paint = true;

    loop {
        // Drain everything that is ready, then paint once.
        let first = match rx.recv_timeout(Duration::from_millis(16)) {
            Ok(input) => Some(input),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        };
        for input in first.into_iter().chain(rx.try_iter()) {
            match input {
                Input::Pty(bytes) => {
                    term.vt_write(&bytes);
                    needs_paint = true;
                    for event in term.events() {
                        match event {
                            Event::PtyWrite(reply) => pty_writer.write_all(&reply)?,
                            Event::TitleChanged(t) => title = t,
                            Event::Bell => execute!(out, style::Print("\x07"))?,
                            _ => {}
                        }
                    }
                }
                Input::Keys(bytes) => pty_writer.write_all(&bytes)?,
                Input::Exit => return Ok(()),
            }
        }
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if !needs_paint {
            continue;
        }
        needs_paint = false;

        let snapshot = state.update(term)?;
        let colors = snapshot.colors()?;
        let mut rows = row_iter.update(&snapshot)?;
        while let Some((y, row)) = rows.next_dirty() {
            queue!(out, cursor::MoveTo(0, y))?;
            let selection = row.selection()?;
            let mut cells = cell_iter.update(row)?;
            let mut x = 0u16;
            let mut text = String::new();
            while let Some(cell) = cells.next() {
                let raw = cell.raw_cell()?;
                if raw.wide()? == CellWide::SpacerTail {
                    x += 1;
                    continue;
                }
                let style = cell.style()?;
                let mut fg = cell.fg_color()?.unwrap_or(colors.foreground);
                let mut bg = cell.bg_color()?.unwrap_or(colors.background);
                if style.inverse || selection.is_some_and(|s| x >= s.start_x && x <= s.end_x) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                queue!(
                    out,
                    style::SetForegroundColor(color(fg)),
                    style::SetBackgroundColor(color(bg)),
                    style::SetAttribute(if style.bold {
                        style::Attribute::Bold
                    } else {
                        style::Attribute::NormalIntensity
                    }),
                    style::SetAttribute(if style.italic {
                        style::Attribute::Italic
                    } else {
                        style::Attribute::NoItalic
                    }),
                    style::SetAttribute(if style.underline == Underline::None {
                        style::Attribute::NoUnderline
                    } else {
                        style::Attribute::Underlined
                    }),
                )?;
                text.clear();
                cell.graphemes_utf8(&mut text)?;
                if text.is_empty() {
                    text.push(' ');
                }
                queue!(out, style::Print(&text))?;
                x += 1;
            }
            queue!(
                out,
                style::ResetColor,
                style::SetAttribute(style::Attribute::Reset)
            )?;
        }
        let cursor_state = snapshot.cursor()?;
        match cursor_state.viewport {
            Some(pos) if cursor_state.visible => {
                let shape = match cursor_state.visual_style {
                    CursorVisualStyle::Bar => cursor::SetCursorStyle::SteadyBar,
                    CursorVisualStyle::Underline => cursor::SetCursorStyle::SteadyUnderScore,
                    _ => cursor::SetCursorStyle::SteadyBlock,
                };
                queue!(out, cursor::MoveTo(pos.x, pos.y), shape, cursor::Show)?;
            }
            _ => queue!(out, cursor::Hide)?,
        }
        if !title.is_empty() {
            queue!(out, terminal::SetTitle(&title))?;
        }
        out.flush()?;
        snapshot.clean()?;
    }
}

fn color(c: RgbColor) -> style::Color {
    style::Color::Rgb {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}
