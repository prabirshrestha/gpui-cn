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

/// Starts an engine on a stream, takes its first frame and grants credit
/// for the next, as a painted terminal does.
fn started(viewport: Viewport) -> (StreamPeer, FrameSink, Box<dyn FrameHandle>) {
    let (source, peer) = StreamSource::new();
    let (sink, _wake) = FrameSink::new();
    let handle = Box::new(Engine::new(source, EngineOptions::default()))
        .start(
            sink.clone(),
            StartOptions::default().with_viewport(viewport),
        )
        .unwrap();
    wait_for(&sink, handle.as_ref(), |_| true);
    (peer, sink, handle)
}

/// Fails if the engine publishes a frame within `window`.
fn assert_no_frame(sink: &FrameSink, window: Duration, why: &str) {
    let deadline = Instant::now() + window;
    while Instant::now() < deadline {
        assert!(sink.take().is_none(), "{why}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Program output: `lines` numbered lines, the last one marked.
fn flood(lines: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in 0..lines {
        bytes.extend_from_slice(format!("line {line} the quick brown fox\r\n").as_bytes());
    }
    bytes.extend_from_slice(b"LAST");
    bytes
}

#[test]
fn an_idle_terminal_builds_no_frames() {
    let (_peer, sink, handle) = started(Viewport::new(80, 24, 8, 16).unwrap());
    assert_no_frame(&sink, Duration::from_millis(400), "nothing changed");
    handle.close();
}

#[test]
fn a_hidden_terminal_drains_output_without_frames_and_shows_it_when_revealed() {
    let (peer, sink, handle) = started(Viewport::new(80, 24, 8, 16).unwrap());
    handle.set_visible(false);
    peer.output(&flood(50_000));
    assert_no_frame(
        &sink,
        Duration::from_millis(400),
        "a hidden terminal draws nothing",
    );
    handle.set_visible(true);
    let shown = wait_for(&sink, handle.as_ref(), |s| s.frame.text().contains("LAST"));
    assert!(
        shown.frame.text().contains("line 49999"),
        "the output was parsed"
    );
    handle.close();
}

#[test]
fn a_terminal_that_is_not_painted_builds_no_more_frames() {
    // The first frame is taken but never painted, so no credit comes back:
    // what a terminal on a page that is not shown, or in a window that is
    // minimized, sees.
    let (source, peer) = StreamSource::new();
    let (sink, _wake) = FrameSink::new();
    let handle = Box::new(Engine::new(source, EngineOptions::default()))
        .start(
            sink.clone(),
            StartOptions::default().with_viewport(Viewport::new(80, 24, 8, 16).unwrap()),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while sink.take().is_none() {
        assert!(Instant::now() < deadline, "no first frame");
        std::thread::sleep(Duration::from_millis(5));
    }
    peer.output(&flood(20_000));
    assert_no_frame(&sink, Duration::from_millis(400), "no credit, no frame");
    // A paint grants one frame, and it is current.
    handle.request_frame();
    let shown = wait_for(&sink, handle.as_ref(), |s| s.frame.text().contains("LAST"));
    assert!(shown.frame.text().contains("line 19999"));
    handle.close();
}

/// A 2x2 Kitty graphics animation of two frames, 50 ms each, that loops:
/// frame 1 is red, frame 2 is black.
fn animation() -> Vec<u8> {
    let mut bytes = b"\x1b_Ga=T,q=2,i=3,f=32,s=1,v=1;/wAA/w==\x1b\\".to_vec();
    bytes.extend_from_slice(b"\x1b_Ga=f,q=2,i=3,f=32,s=1,v=1,z=50;AAAA/w==\x1b\\");
    bytes.extend_from_slice(b"\x1b_Ga=a,q=2,i=3,r=1,z=50\x1b\\");
    bytes.extend_from_slice(b"\x1b_Ga=a,q=2,i=3,s=3,v=1\x1b\\");
    bytes
}

/// The first pixel of the first image of a snapshot.
fn first_pixel(snapshot: &TerminalSnapshot) -> Option<[u8; 4]> {
    let image = &snapshot.frame.images.first()?.image;
    image.rgba()[..4].try_into().ok()
}

#[test]
fn a_kitty_animation_plays_while_shown_and_stops_while_hidden() {
    let (peer, sink, handle) = started(Viewport::new(20, 4, 8, 16).unwrap());
    peer.output(&animation());
    // Both frames show without more output.
    wait_for(&sink, handle.as_ref(), |s| {
        first_pixel(s) == Some([255, 0, 0, 255])
    });
    wait_for(&sink, handle.as_ref(), |s| {
        first_pixel(s) == Some([0, 0, 0, 255])
    });
    wait_for(&sink, handle.as_ref(), |s| {
        first_pixel(s) == Some([255, 0, 0, 255])
    });
    handle.set_visible(false);
    // A frame already on its way may still arrive.
    std::thread::sleep(Duration::from_millis(100));
    let _ = sink.take();
    handle.request_frame();
    assert_no_frame(
        &sink,
        Duration::from_millis(300),
        "a hidden terminal does not animate",
    );
    handle.close();
}
