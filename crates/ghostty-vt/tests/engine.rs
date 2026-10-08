//! Headless engine tests: bytes in, rows, dirty flags, events, encoders,
//! search and snapshots out.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use ghostty_vt::fmt::{Format, Formatter, FormatterOptions};
use ghostty_vt::key;
use ghostty_vt::mouse;
use ghostty_vt::render::{CellIterator, Dirty, RenderState, RowIterator};
use ghostty_vt::search::{Search, Status};
use ghostty_vt::selection::{FormatOptions, Selection};
use ghostty_vt::snapshot;
use ghostty_vt::style::{RgbColor, StyleColor, Underline};
use ghostty_vt::terminal::{
    ClipboardLocation, ClipboardWritePolicy, ColorScheme, Mode, Point, PointCoordinate, PointSpace,
    ProgressState, ScrollViewport,
};
use ghostty_vt::{Event, Terminal, TerminalOptions};

fn events(terminal: &mut Terminal) -> Vec<Event> {
    terminal.events().collect()
}

fn pty_output(terminal: &mut Terminal) -> Vec<u8> {
    events(terminal)
        .into_iter()
        .filter_map(|event| match event {
            Event::PtyWrite(bytes) => Some(bytes),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn rows_reflect_cursor_movement_and_erase() {
    let mut terminal = common::terminal(20, 4);
    terminal.vt_write(b"line one\r\nline two\r\n");
    terminal.vt_write(b"\x1b[1;1Hfirst\x1b[2K");
    assert_eq!(common::rows(&terminal), vec!["", "line two", "", ""]);
    terminal.vt_write(b"\x1b[1;1HA\x1b[1;20HZ");
    assert_eq!(common::rows(&terminal)[0], "A                  Z");
}

#[test]
fn styles_and_colors_reach_the_render_state() {
    let mut terminal = common::terminal(10, 1);
    terminal.vt_write(b"\x1b[1;4;38;2;255;128;0mx\x1b[0m");
    let mut state = RenderState::new().unwrap();
    let mut rows = RowIterator::new().unwrap();
    let mut cells = CellIterator::new().unwrap();
    let snapshot = state.update(&terminal).unwrap();
    let mut row_iter = rows.update(&snapshot).unwrap();
    let row = row_iter.next().unwrap();
    let mut cell_iter = cells.update(row).unwrap();
    let cell = cell_iter.next().unwrap();
    let style = cell.style().unwrap();
    assert!(style.bold);
    assert_eq!(style.underline, Underline::Single);
    assert_eq!(
        style.fg_color,
        StyleColor::Rgb(RgbColor {
            r: 255,
            g: 128,
            b: 0
        })
    );
    assert_eq!(
        cell.fg_color().unwrap(),
        Some(RgbColor {
            r: 255,
            g: 128,
            b: 0
        })
    );
    let mut text = String::new();
    cell.graphemes_utf8(&mut text).unwrap();
    assert_eq!(text, "x");
}

#[test]
fn dirty_flags_track_rows_and_clean_resets_them() {
    let mut terminal = common::terminal(10, 3);
    let mut state = RenderState::new().unwrap();
    let mut rows = RowIterator::new().unwrap();

    terminal.vt_write(b"a");
    {
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Full);
        snapshot.clean().unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Clean);
    }

    terminal.vt_write(b"\x1b[3;1Hc");
    {
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Partial);
        let mut dirty_rows = Vec::new();
        let mut row_iter = rows.update(&snapshot).unwrap();
        while let Some((y, row)) = row_iter.next_dirty() {
            assert!(row.dirty().unwrap());
            assert_eq!(i32::from(y), row.viewport_y().unwrap());
            dirty_rows.push(y);
        }
        // The row the cursor left and the row it landed on.
        assert_eq!(dirty_rows, vec![0, 2]);
        snapshot.clean().unwrap();
    }

    {
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Clean);
        let mut row_iter = rows.update(&snapshot).unwrap();
        assert!(row_iter.next_dirty().is_none());
    }
}

#[test]
fn row_ids_survive_scrolling_and_overscan_reports_history() {
    let mut terminal = common::terminal(10, 2);
    let mut state = RenderState::new().unwrap();
    let mut rows = RowIterator::new().unwrap();
    state
        .set_overscan(ghostty_vt::render::Overscan { above: 1, below: 1 })
        .unwrap();

    terminal.vt_write(b"one\r\ntwo\r\n");
    let ids_before: Vec<_> = {
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.overscan().unwrap().above, 1);
        assert_eq!(snapshot.overscan().unwrap().below, 0);
        let mut row_iter = rows.update(&snapshot).unwrap();
        let mut ids = Vec::new();
        while let Some(row) = row_iter.next() {
            ids.push((row.viewport_y().unwrap(), row.id().unwrap()));
        }
        ids
    };
    assert_eq!(ids_before.len(), 3);
    assert_eq!(ids_before[0].0, -1);
    assert!(ids_before.iter().all(|(_, id)| !id.is_none()));

    terminal.vt_write(b"three\r\n");
    let snapshot = state.update(&terminal).unwrap();
    let mut row_iter = rows.update(&snapshot).unwrap();
    let mut ids_after = Vec::new();
    while let Some(row) = row_iter.next() {
        ids_after.push((row.viewport_y().unwrap(), row.id().unwrap()));
    }
    // The row that was at viewport y 0 is now the overscan row above.
    assert_eq!(ids_after[0].1, ids_before[1].1);
}

#[test]
fn events_are_queued_in_stream_order() {
    let mut terminal = common::terminal(20, 3);
    terminal.vt_write(b"\x07\x1b]2;Title\x1b\\\x1b]7;file:///tmp\x1b\\\x07");
    let got = events(&mut terminal);
    assert_eq!(
        got,
        vec![
            Event::Bell,
            Event::TitleChanged("Title".to_owned()),
            Event::PwdChanged("file:///tmp".to_owned()),
            Event::Bell,
        ]
    );
    assert_eq!(terminal.title().unwrap(), "Title");
    assert!(events(&mut terminal).is_empty());
}

#[test]
fn queries_are_answered_from_configured_values() {
    let mut terminal = common::terminal(20, 3);
    // DA1 uses Ghostty's default.
    terminal.vt_write(b"\x1b[c");
    assert_eq!(pty_output(&mut terminal), b"\x1b[?62;22c");

    // Without configured attributes, libghostty answers with its own.
    terminal.set_device_attributes(None);
    terminal.vt_write(b"\x1b[>c");
    assert_eq!(pty_output(&mut terminal), b"\x1b[>1;0;0c");

    terminal.set_xtversion(Some("myterm 9.9".to_owned()));
    terminal.vt_write(b"\x1b[>q");
    assert_eq!(pty_output(&mut terminal), b"\x1bP>|myterm 9.9\x1b\\");

    terminal.set_enquiry_response(Some(b"ENQ!".to_vec()));
    terminal.vt_write(b"\x05");
    assert_eq!(pty_output(&mut terminal), b"ENQ!");

    terminal.set_color_scheme(Some(ColorScheme::Dark));
    terminal.vt_write(b"\x1b[?996n");
    assert_eq!(pty_output(&mut terminal), b"\x1b[?997;1n");

    // Size reports need a resize first.
    terminal.vt_write(b"\x1b[18t");
    assert!(pty_output(&mut terminal).is_empty());
    terminal.resize(20, 3, 8, 16).unwrap();
    terminal.vt_write(b"\x1b[18t");
    assert_eq!(pty_output(&mut terminal), b"\x1b[8;3;20t");
    terminal.set_size_reports(false);
    terminal.vt_write(b"\x1b[18t");
    let out = pty_output(&mut terminal);
    assert!(
        out.is_empty(),
        "{}",
        String::from_utf8_lossy(&out).escape_debug()
    );
}

#[test]
fn clipboard_writes_follow_the_policy() {
    let mut terminal = common::terminal(20, 3);
    // "hello" in base64.
    terminal.vt_write(b"\x1b]52;c;aGVsbG8=\x1b\\");
    let got = events(&mut terminal);
    assert_eq!(got.len(), 1);
    let Event::ClipboardWrite(write) = &got[0] else {
        panic!("expected a clipboard write, got {got:?}");
    };
    assert_eq!(write.location, ClipboardLocation::Standard);
    assert_eq!(write.contents.len(), 1);
    assert_eq!(write.contents[0].mime, "text/plain");
    assert_eq!(write.contents[0].data, b"hello");
    assert!(!write.granted);

    terminal.set_clipboard_write_policy(ClipboardWritePolicy::Deny);
    terminal.vt_write(b"\x1b]52;c;aGVsbG8=\x1b\\");
    assert!(events(&mut terminal).is_empty());
}

#[test]
fn progress_and_notification_events() {
    let mut terminal = common::terminal(20, 3);
    terminal.vt_write(b"\x1b]9;4;1;42\x1b\\\x1b]777;notify;Hi;There\x1b\\");
    let got = events(&mut terminal);
    assert_eq!(
        got,
        vec![
            Event::ProgressReport {
                state: ProgressState::Set,
                progress: Some(42),
            },
            Event::DesktopNotification {
                title: "Hi".to_owned(),
                body: "There".to_owned(),
            },
        ]
    );
}

#[test]
fn render_hold_callback_captures_the_frame() {
    let mut terminal = common::terminal(10, 2);
    let captured = Rc::new(Cell::new(String::new()));
    let seen = Rc::new(Cell::new(0));
    {
        let captured = captured.clone();
        let seen = seen.clone();
        terminal.set_render_hold(Some(Box::new(move |terminal, held| {
            seen.set(seen.get() + 1);
            if held {
                captured.set(common::screen(terminal));
            }
        })));
    }
    terminal.vt_write(b"before\x1b[?2026hafter\x1b[?2026l");
    assert_eq!(seen.get(), 2);
    assert_eq!(captured.take(), "before");
    assert_eq!(common::screen(&terminal), "beforeafte\nr");
    let holds: Vec<_> = events(&mut terminal)
        .into_iter()
        .filter(|e| matches!(e, Event::RenderHold(_)))
        .collect();
    assert_eq!(
        holds,
        vec![Event::RenderHold(true), Event::RenderHold(false)]
    );
}

#[test]
fn modes_and_options_round_trip() {
    let mut terminal = common::terminal(10, 2);
    assert!(terminal.mode(Mode::GRAPHEME_CLUSTER).unwrap());
    terminal.set_mode(Mode::BRACKETED_PASTE, true).unwrap();
    assert!(terminal.mode(Mode::BRACKETED_PASTE).unwrap());
    terminal.reset();
    assert!(!terminal.mode(Mode::BRACKETED_PASTE).unwrap());
    assert!(terminal.mode(Mode::GRAPHEME_CLUSTER).unwrap());

    terminal.set_scrollback_max_lines(Some(100)).unwrap();
    assert_eq!(terminal.scrollback_max_lines().unwrap(), Some(100));
    terminal.set_scrollback_max_bytes(None).unwrap();
    assert_eq!(terminal.scrollback_max_bytes().unwrap(), None);
    terminal.set_clipboard_write_max_bytes(Some(1024)).unwrap();
    assert_eq!(terminal.clipboard_write_max_bytes().unwrap(), 1024);
    terminal.set_terminfo_name(Some("xterm-ghostty")).unwrap();
    terminal.set_title_report(true).unwrap();
    terminal.set_resize_pull_scrollback(false).unwrap();
    assert!(terminal.is_viewport_active().unwrap());
    assert!(!terminal.has_vt_processing_error().unwrap());
}

#[test]
fn key_and_mouse_encoders_follow_terminal_modes() {
    let mut terminal = common::terminal(10, 2);
    let mut encoder = key::Encoder::new().unwrap();
    let mut event = key::Event::new().unwrap();
    event
        .set_action(key::Action::Press)
        .set_key(key::Key::ArrowUp);
    let mut out = Vec::new();
    encoder.encode_to_vec(&event, &mut out).unwrap();
    assert_eq!(out, b"\x1b[A");

    terminal.set_mode(Mode::DECCKM, true).unwrap();
    encoder.set_options_from_terminal(&terminal);
    out.clear();
    encoder.encode_to_vec(&event, &mut out).unwrap();
    assert_eq!(out, b"\x1bOA");

    // Kitty keyboard flags come from the terminal too.
    terminal.vt_write(b"\x1b[>1u");
    assert_eq!(
        terminal.kitty_keyboard_flags().unwrap(),
        key::KittyKeyFlags::DISAMBIGUATE
    );
    encoder.set_options_from_terminal(&terminal);
    event.set_key(key::Key::Escape);
    out.clear();
    encoder.encode_to_vec(&event, &mut out).unwrap();
    assert_eq!(
        out,
        b"\x1b[27u",
        "{}",
        String::from_utf8_lossy(&out).escape_debug()
    );

    // Mouse tracking must come through the VT stream for the encoder to
    // see it; a mode set through the option API does not update the
    // terminal's tracking flags.
    let mut mouse_encoder = mouse::Encoder::new().unwrap();
    terminal.vt_write(b"\x1b[?1000h\x1b[?1006h");
    assert!(terminal.is_mouse_tracking().unwrap());
    mouse_encoder
        .set_options_from_terminal(&terminal)
        .set_size(mouse::EncoderSize {
            screen_width: 80,
            screen_height: 32,
            cell_width: 8,
            cell_height: 16,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        });
    let mut mouse_event = mouse::Event::new().unwrap();
    mouse_event
        .set_action(mouse::Action::Press)
        .set_button(Some(mouse::Button::Left))
        .set_position(mouse::Position { x: 20.0, y: 20.0 });
    out.clear();
    mouse_encoder.encode_to_vec(&mouse_event, &mut out).unwrap();
    assert_eq!(out, b"\x1b[<0;3;2M");
}

#[test]
fn selection_and_formatter_produce_text() {
    let mut terminal = common::terminal(10, 3);
    terminal.vt_write(b"hello world\r\nsecond");
    let all = terminal.select_all().unwrap().unwrap();
    let text = terminal
        .format_selection_alloc(
            FormatOptions::new()
                .with_selection(&all)
                .with_emit_format(Format::Plain)
                .with_trim(true),
        )
        .unwrap()
        .unwrap();
    assert_eq!(std::str::from_utf8(&text).unwrap(), "hello worl\nd\nsecond");

    let start = terminal
        .grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
        .unwrap();
    let end = terminal
        .grid_ref(Point::Active(PointCoordinate { x: 4, y: 0 }))
        .unwrap();
    let selection = Selection::new(start, end, false);
    terminal.set_selection(Some(&selection)).unwrap();
    let mut formatter = Formatter::new(
        &terminal,
        FormatterOptions::new()
            .with_format(Format::Plain)
            .with_selection(&selection),
    )
    .unwrap();
    let bytes = formatter.format_alloc().unwrap();
    assert_eq!(std::str::from_utf8(&bytes).unwrap().trim_end(), "hello");
    let mut streamed = Vec::new();
    formatter.format_to(&mut streamed).unwrap();
    assert_eq!(streamed.as_slice(), &*bytes);
    assert_eq!(
        terminal
            .point_from_grid_ref(&selection.end(), PointSpace::Viewport)
            .unwrap(),
        Some(PointCoordinate { x: 4, y: 0 })
    );
}

#[test]
fn search_finds_matches_in_screen_and_scrollback() {
    let mut terminal = common::terminal(20, 2);
    for i in 0..50 {
        terminal.vt_write(format!("row {i} needle\r\n").as_bytes());
    }
    let mut search = Search::new(&terminal).unwrap();
    assert_eq!(search.status().unwrap(), Status::Complete);
    search.set_needle(&terminal, Some("NEEDLE")).unwrap();
    assert_eq!(search.needle().unwrap(), Some("NEEDLE"));
    search.run(&terminal).unwrap();
    assert_eq!(search.status().unwrap(), Status::Complete);
    assert_eq!(search.total_matches().unwrap(), 50);
    assert_eq!(search.matches(&terminal).unwrap().len(), 50);
    assert!(search.select_next(&terminal).unwrap());
    assert_eq!(search.selected_index().unwrap(), Some(0));
    let selected = search.selected_match(&terminal).unwrap().unwrap();
    let text = terminal
        .format_selection_alloc(FormatOptions::new().with_selection(&selected))
        .unwrap()
        .unwrap();
    assert_eq!(std::str::from_utf8(&text).unwrap(), "needle");
    assert!(!search.viewport_matches(&terminal).unwrap().is_empty());

    search.set_needle(&terminal, None).unwrap();
    assert_eq!(search.total_matches().unwrap(), 0);
    assert!(!search.select_next(&terminal).unwrap());
}

#[test]
fn search_survives_the_terminal_being_dropped_first() {
    let mut terminal = common::terminal(20, 2);
    terminal.vt_write(b"abc");
    let mut search = Search::new(&terminal).unwrap();
    search.set_needle(&terminal, Some("abc")).unwrap();
    search.run(&terminal).unwrap();
    drop(terminal);
    assert_eq!(search.total_matches().unwrap(), 1);
    assert!(search.tick().is_ok());
}

#[test]
fn snapshot_round_trips_through_a_mid_sequence_cut() {
    let mut terminal = common::terminal(20, 3);
    terminal.set_continuation_max_bytes(1024).unwrap();
    terminal.vt_write(b"one\r\ntwo\r\nthr");
    // Cut in the middle of an SGR sequence and a multi-byte codepoint.
    terminal.vt_write(b"\x1b[1;3");
    assert!(!terminal.is_vt_ground().unwrap());
    assert_eq!(terminal.continuation().unwrap(), b"\x1b[1;3");

    let bytes = snapshot::encode(&terminal).unwrap();
    let mut streamed = Vec::new();
    snapshot::encode_to(&terminal, &mut streamed).unwrap();
    assert_eq!(streamed.as_slice(), &*bytes);

    let mut decoder = snapshot::Decoder::from_slice(&bytes);
    decoder.set_retain_continuation(true).unwrap();
    let mut restored = decoder.decode().unwrap();
    assert_eq!(restored.continuation().unwrap(), b"\x1b[1;3");
    assert_eq!(common::rows(&restored), common::rows(&terminal));

    // Finish the sequence on both and compare again.
    terminal.vt_write(b"1mee\xe2\x82");
    restored.vt_write(b"1mee\xe2\x82");
    terminal.vt_write(b"\xac");
    restored.vt_write(b"\xac");
    assert_eq!(common::rows(&restored), common::rows(&terminal));
    assert_eq!(common::rows(&restored)[2], "three\u{20ac}");
}

#[test]
fn snapshot_restores_history_incrementally() {
    let mut terminal = Terminal::new(TerminalOptions {
        cols: 80,
        rows: 2,
        max_scrollback: 64 << 20,
    })
    .unwrap();
    for i in 0..20_000 {
        terminal.vt_write(format!("line {i}\r\n").as_bytes());
    }
    let total = terminal.total_rows().unwrap();
    assert!(total > 19_000);
    let bytes = snapshot::encode(&terminal).unwrap();

    let mut decoder = snapshot::Decoder::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    decoder.ready().unwrap();
    assert!(decoder.history_rows_primary().unwrap().is_some());
    let mut pages = 0;
    while decoder.next_page().unwrap().is_some() {
        pages += 1;
    }
    let restored = decoder.into_terminal().unwrap();
    assert!(pages >= 1);
    assert_eq!(restored.total_rows().unwrap(), total);
    assert_eq!(common::rows(&restored), common::rows(&terminal));
}

#[test]
fn fragmented_input_matches_whole_input_at_every_byte_boundary() {
    let input: &[u8] =
        b"a\x1b[1;32mb\x1b[0m\xe2\x82\xac\x1b]2;t\x1b\\\r\n\x1b[2Jc\xf0\x9f\x98\x80\x1b[?25l";
    let mut whole = common::terminal(10, 3);
    whole.vt_write(input);
    let expected = common::rows(&whole);
    let expected_events = events(&mut whole);

    for cut in 0..=input.len() {
        let mut split = common::terminal(10, 3);
        split.vt_write(&input[..cut]);
        split.vt_write(&input[cut..]);
        assert_eq!(common::rows(&split), expected, "cut at {cut}");
        assert_eq!(events(&mut split), expected_events, "cut at {cut}");
        assert!(!split.mode(Mode::CURSOR_VISIBLE).unwrap());
    }

    // One byte at a time as well.
    let mut bytewise = common::terminal(10, 3);
    for byte in input {
        bytewise.vt_write(std::slice::from_ref(byte));
    }
    assert_eq!(common::rows(&bytewise), expected);
}

#[test]
fn parsing_one_megabyte_without_a_render_state() {
    let mut terminal = Terminal::new(TerminalOptions {
        cols: 80,
        rows: 24,
        max_scrollback: 1 << 20,
    })
    .unwrap();
    let line = b"\x1b[38;5;123mlorem ipsum dolor sit amet \x1b[0mconsectetur adipiscing\r\n";
    let mut buffer = Vec::with_capacity(1 << 20);
    while buffer.len() < (1 << 20) {
        buffer.extend_from_slice(line);
    }
    let start = std::time::Instant::now();
    for chunk in buffer.chunks(64 * 1024) {
        terminal.vt_write(chunk);
    }
    let elapsed = start.elapsed();
    // Nothing read the render state; only the events queue was touched, and
    // this input produces none.
    assert!(!terminal.has_events());
    assert!(terminal.total_rows().unwrap() > 24);
    // A parse of 1 MB of styled text is well under a second even unoptimized.
    assert!(elapsed.as_secs() < 5, "parse took {elapsed:?}");
    terminal.scroll_viewport(ScrollViewport::Top);
    assert!(!terminal.is_viewport_active().unwrap());
}

#[test]
fn resize_reflows_and_reports_in_band_size() {
    let mut terminal = common::terminal(10, 2);
    terminal.vt_write(b"0123456789abc");
    assert_eq!(common::rows(&terminal), vec!["0123456789", "abc"]);
    terminal.set_mode(Mode::IN_BAND_RESIZE, true).unwrap();
    terminal.resize(20, 2, 8, 16).unwrap();
    assert_eq!(common::rows(&terminal)[0], "0123456789abc");
    let output = pty_output(&mut terminal);
    assert_eq!(output, b"\x1b[48;2;20;32;160t");
}

#[test]
fn terminfo_and_shell_integration_resources_are_written_once() {
    let temp = std::env::temp_dir().join(format!("ghostty-vt-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    // SAFETY: This test binary sets the variable before any other thread
    // reads it.
    unsafe { std::env::set_var("GHOSTTY_VT_CACHE_DIR", &temp) };

    let terminfo = ghostty_vt::terminfo::dir().unwrap();
    assert!(terminfo.join("78/xterm-ghostty").is_file());
    assert!(terminfo.join("x/xterm-ghostty").is_file());
    assert!(terminfo.join("g/ghostty").is_file());
    assert!(ghostty_vt::terminfo::SOURCE.starts_with("xterm-ghostty|ghostty|Ghostty,"));

    let resources = ghostty_vt::shell_integration::resources_dir().unwrap();
    for shell in ghostty_vt::shell_integration::SHELLS {
        assert!(
            resources.join("shell-integration").join(shell).is_dir(),
            "{shell}"
        );
    }
    assert!(
        resources
            .join("shell-integration/zsh/ghostty-integration")
            .is_file()
    );

    assert_eq!(ghostty_vt::terminfo::dir().unwrap(), terminfo);
    let _ = std::fs::remove_dir_all(&temp);
}
