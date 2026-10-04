//! A terminal driven by the application's own byte stream, as a remote
//! session or an application's own pty would drive it. The engine runs on
//! its own thread, so the test reads its frames from the sink, as the
//! pty test does.

#![cfg(feature = "ghostty")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_cn::terminal::input::KeyInput;
use gpui_cn::terminal::{
    Engine, EngineOptions, ExitStatus, FrameHandle, FrameSink, FrameSource, StartOptions,
    StreamEvent, StreamPeer, StreamSource, TerminalInput, TerminalSnapshot, TerminalStatus,
    Viewport,
};

/// Waits until a snapshot satisfies `done`, granting a frame after each.
fn wait_for(
    sink: &FrameSink,
    handle: &dyn FrameHandle,
    done: impl Fn(&TerminalSnapshot) -> bool,
) -> Arc<TerminalSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = None;
    while Instant::now() < deadline {
        if let Some(snapshot) = sink.take() {
            handle.request_frame();
            if done(&snapshot) {
                return snapshot;
            }
            last = Some(snapshot);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "timed out; last frame:\n{}",
        last.map_or_else(String::new, |s| s.frame.text())
    );
}

/// Waits until the peer has seen `event`.
fn wait_event(peer: &StreamPeer, event: &StreamEvent) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        while let Ok(seen) = peer.events().try_recv() {
            if &seen == event {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {event:?}");
}

#[test]
fn a_custom_stream_renders_output_gets_input_and_sizes_and_reports_exit() {
    let (source, peer) = StreamSource::new();
    // Output written before the terminal starts is kept.
    peer.output(b"remote$ ");
    let (sink, _wake) = FrameSink::new();
    let start = Viewport::new(60, 10, 8, 16).unwrap();
    let handle = Box::new(Engine::new(source, EngineOptions::default()))
        .start(sink.clone(), StartOptions::default().with_viewport(start))
        .unwrap();
    wait_event(&peer, &StreamEvent::Resize(start));

    peer.output(b"ls\r\nnotes.txt\r\n");
    let shown = wait_for(&sink, handle.as_ref(), |s| {
        s.frame.text().contains("notes.txt")
    });
    assert!(shown.frame.text().starts_with("remote$ ls\nnotes.txt"));
    assert_eq!(shown.status, TerminalStatus::Live);

    // Keys arrive encoded for the program.
    handle
        .input(TerminalInput::Key(KeyInput::new("a").with_text("a")))
        .unwrap();
    wait_event(&peer, &StreamEvent::Input(b"a".to_vec()));

    let bigger = Viewport::new(100, 30, 8, 16).unwrap();
    handle.resize(bigger);
    wait_event(&peer, &StreamEvent::Resize(bigger));
    let resized = wait_for(&sink, handle.as_ref(), |s| s.frame.viewport == bigger);
    assert_eq!(resized.frame.rows.len(), 30);

    peer.exit(ExitStatus::new(Some(3), None::<String>));
    let exited = wait_for(&sink, handle.as_ref(), |s| {
        matches!(s.status, TerminalStatus::Exited(_))
    });
    let TerminalStatus::Exited(status) = &exited.status else {
        unreachable!()
    };
    assert_eq!(status.code(), Some(3));

    handle.close();
    wait_event(&peer, &StreamEvent::Closed);
}
