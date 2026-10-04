//! Measures what a frame of each gallery page costs, offscreen.
//!
//! ```bash
//! cargo run --release -p gpui-cn-story --features snapshot --bin bench -- [frames]
//! ```
//!
//! For each page it reports the median and the slowest-in-20 cost of a
//! full frame (every view re-rendered, laid out, and painted, which is
//! what a resize or a theme change costs) and of a frame after nothing
//! changed (what an idle tick costs). Then the select: opening a menu of
//! three rows and one of ten thousand, a wheel step and a keyboard step
//! through the long one, a keystroke in its search, fifty opens and
//! closes, with the frames each asks for at rest, the CPU time, and the
//! resident memory. Runs on macOS, where GPUI has a headless Metal
//! renderer.

fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(not(target_os = "macos"))]
    println!("bench: skipped; GPUI has no headless renderer on this platform");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{sync::Arc, time::Instant};

    use gpui_cn::{ReduceMotion, Theme};
    use gpui_cn_story::Gallery;
    use gpui_kit::{
        AnyWindowHandle, AppContext as _, Entity, HeadlessAppContext, PlatformInput, ScrollDelta,
        ScrollWheelEvent, assets::Assets, point, px, test::TestWindowExt as _,
    };

    /// Median and 95th percentile, in milliseconds.
    fn summarize(mut samples: Vec<f64>) -> (f64, f64) {
        samples.sort_by(|a, b| a.total_cmp(b));
        let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
        (at(0.5), at(0.95))
    }

    fn measure(
        cx: &mut HeadlessAppContext,
        handle: AnyWindowHandle,
        frames: usize,
        full: bool,
    ) -> (f64, f64) {
        let mut samples = Vec::with_capacity(frames);
        for _ in 0..frames {
            cx.update_window(handle, |_, window, cx| {
                let start = Instant::now();
                if full {
                    window.render_frame(cx);
                } else {
                    window.draw(cx).clear(cx);
                }
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            })
            .expect("render");
        }
        summarize(samples)
    }

    /// This process' CPU time so far, user and system, in milliseconds.
    fn cpu_ms() -> f64 {
        // SAFETY: `getrusage` fills the struct it is given for this
        // process; a zeroed struct is a valid one to fill.
        let usage = unsafe {
            let mut usage: libc::rusage = std::mem::zeroed();
            assert_eq!(libc::getrusage(libc::RUSAGE_SELF, &mut usage), 0);
            usage
        };
        let seconds =
            |time: libc::timeval| time.tv_sec as f64 * 1000. + time.tv_usec as f64 / 1000.;
        seconds(usage.ru_utime) + seconds(usage.ru_stime)
    }

    /// This process' resident set, in megabytes.
    fn resident_mb() -> f64 {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .expect("ps");
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.)
            / 1024.
    }

    pub fn run() {
        let frames: usize = std::env::args()
            .nth(1)
            .and_then(|frames| frames.parse().ok())
            .unwrap_or(200);
        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(Assets),
            gpui_kit::platform::current_headless_renderer,
        );
        let start = Instant::now();
        cx.update(|cx| {
            gpui_kit::init(cx);
            let kit = start.elapsed();
            gpui_cn::init(cx);
            println!(
                "init: gpui_kit {:.1} ms, gpui_cn {:.1} ms",
                kit.as_secs_f64() * 1000.,
                (start.elapsed() - kit).as_secs_f64() * 1000.
            );
            // Terminal panes draw canned output, so the numbers do not
            // depend on the user's shell.
            #[cfg(feature = "terminal")]
            gpui_cn_story::stories::TerminalStory::use_fixtures(cx);
            Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
        });
        let mut gallery: Option<Entity<Gallery>> = None;
        let start = Instant::now();
        let handle = cx
            .open_window(gpui_cn_story::WINDOW_SIZE, |window, cx| {
                let opened = Instant::now();
                let view = cx.new(|cx| Gallery::new(window, cx));
                println!(
                    "Gallery::new: {:.1} ms",
                    opened.elapsed().as_secs_f64() * 1000.
                );
                gallery = Some(view.clone());
                cx.new(|cx| gpui_kit::base::Root::new(view, window, cx))
            })
            .expect("open the gallery window");
        let gallery = gallery.expect("the gallery view");
        let handle: AnyWindowHandle = handle.into();
        let opened = start.elapsed();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .expect("render");
        println!(
            "open window + gallery: {:.1} ms, first frame: {:.1} ms",
            opened.as_secs_f64() * 1000.,
            (start.elapsed() - opened).as_secs_f64() * 1000.
        );
        println!(
            "{:<14} {:>9} {:>9} {:>9} {:>9}",
            "page", "full p50", "full p95", "idle p50", "idle p95"
        );

        let mut pages: Vec<&str> = gpui_cn_story::stories()
            .into_iter()
            .map(|entry| entry.title())
            .collect();
        pages.push("Settings");
        for page in pages {
            cx.update_window(handle, |_, window, cx| {
                gallery.update(cx, |gallery, cx| {
                    if page == "Settings" {
                        gallery.open_settings(cx);
                    } else {
                        gallery.select_story(page, window, cx);
                    }
                });
                window.render_frame(cx);
                window.render_frame(cx);
            })
            .expect("select the page");
            let (full_p50, full_p95) = measure(&mut cx, handle, frames, true);
            let (idle_p50, idle_p95) = measure(&mut cx, handle, frames, false);
            println!(
                "{page:<14} {full_p50:>6.2} ms {full_p95:>6.2} ms {idle_p50:>6.2} ms {idle_p95:>6.2} ms"
            );
        }

        // Scrolling: a wheel step on the Typography page, then the frame it
        // costs, `frames` times.
        cx.update_window(handle, |_, window, cx| {
            gallery.update(cx, |gallery, cx| {
                gallery.select_story("Typography", window, cx);
            });
            window.render_frame(cx);
        })
        .expect("select the page");
        let mut samples = Vec::with_capacity(frames);
        for i in 0..frames {
            cx.update_window(handle, |_, window, cx| {
                let delta = if i % 40 < 20 { -12. } else { 12. };
                // The event goes straight to the window, as the platform
                // sends it, so the number is one wheel step and one frame.
                let position = window.find("page").bounds().center();
                let start = Instant::now();
                window.dispatch_event(
                    PlatformInput::ScrollWheel(ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
                        ..Default::default()
                    }),
                    cx,
                );
                window.draw(cx).clear(cx);
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            })
            .expect("scroll");
        }
        let (p50, p95) = summarize(samples);
        println!("scroll step + frame: {p50:.2} ms p50, {p95:.2} ms p95");

        // The sidebar's slide: frames rendered back to back through the
        // motion, with memory before and after.
        cx.update(|cx| Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::Off));
        let before = resident_mb();
        let mut samples = Vec::new();
        for _ in 0..4 {
            cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx)));
            let start = Instant::now();
            while start.elapsed().as_millis() < 400 {
                cx.update_window(handle, |_, window, cx| {
                    let frame = Instant::now();
                    window.draw(cx).clear(cx);
                    samples.push(frame.elapsed().as_secs_f64() * 1000.);
                })
                .expect("render");
            }
        }
        let after = resident_mb();
        let count = samples.len();
        let (p50, p95) = summarize(samples);
        println!(
            "sidebar slide: {count} frames in 4 x 400 ms, {p50:.2} ms p50, {p95:.2} ms p95; resident {before:.0} MB -> {after:.0} MB"
        );

        cx.update(|cx| Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On));
        select(&mut cx, handle, &gallery, frames);
        #[cfg(feature = "terminal")]
        grid(&mut cx, frames);
        #[cfg(feature = "terminal")]
        terminal();
        #[cfg(feature = "terminal")]
        parking();
    }

    /// The select: what a menu costs to open, to keep open, to scroll, to
    /// search, and to open and close many times.
    fn select(
        cx: &mut HeadlessAppContext,
        handle: AnyWindowHandle,
        gallery: &Entity<Gallery>,
        frames: usize,
    ) {
        use gpui_kit::ElementId;
        let child = |id: &'static str, name: &'static str| {
            ElementId::NamedChild(ElementId::Name(id.into()).into(), name.into())
        };
        cx.update_window(handle, |_, window, cx| {
            gallery.update(cx, |gallery, cx| gallery.select_story("Select", window, cx));
            window.render_frame(cx);
        })
        .expect("select the page");
        println!("select:");
        let cpu_start = cpu_ms();
        let wall_start = Instant::now();

        let open = |cx: &mut HeadlessAppContext, id: &'static str| -> (f64, usize, f64) {
            let before = resident_mb();
            cx.update_window(handle, |_, window, cx| {
                let start = Instant::now();
                window.click(child(id, "trigger"), cx);
                let mut settle = 0;
                while window.simulate_next_frame(cx) > 0 && settle < 10 {
                    settle += 1;
                }
                (
                    start.elapsed().as_secs_f64() * 1000.,
                    settle,
                    resident_mb() - before,
                )
            })
            .expect("open")
        };
        for (id, label) in [("shortcut", "3 rows"), ("numbers", "10,000 rows")] {
            cx.update_window(handle, |_, window, cx| {
                gpui_cn_story::reveal(child(id, "trigger"), window, cx);
            })
            .expect("scroll the page");
            let (ms, settle, grew) = open(cx, id);
            let (full_p50, _) = measure(cx, handle, frames, true);
            let (idle_p50, _) = measure(cx, handle, frames, false);
            let idle_frames = cx
                .update_window(handle, |_, window, cx| {
                    window.render_frame(cx);
                    window.simulate_next_frame(cx)
                })
                .expect("idle");
            println!(
                "  open {label:<12} {ms:>6.2} ms to open (+{settle} frames to settle), then {full_p50:.2} ms full, {idle_p50:.2} ms idle, asks for {idle_frames} frames at rest, resident +{grew:.1} MB"
            );
            if id == "numbers" {
                let mut samples = Vec::with_capacity(frames);
                for i in 0..frames {
                    cx.update_window(handle, |_, window, cx| {
                        let delta = if i % 40 < 20 { -28. } else { 28. };
                        let position = window.find(child(id, "rows")).bounds().center();
                        let start = Instant::now();
                        window.dispatch_event(
                            PlatformInput::ScrollWheel(ScrollWheelEvent {
                                position,
                                delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
                                ..Default::default()
                            }),
                            cx,
                        );
                        window.draw(cx).clear(cx);
                        samples.push(start.elapsed().as_secs_f64() * 1000.);
                    })
                    .expect("scroll");
                }
                let (p50, p95) = summarize(samples);
                println!("  wheel step + frame: {p50:.2} ms p50, {p95:.2} ms p95");
                let mut samples = Vec::with_capacity(frames);
                for i in 0..frames {
                    cx.update_window(handle, |_, window, cx| {
                        let start = Instant::now();
                        window.press(if i % 40 < 20 { "down" } else { "up" }, cx);
                        samples.push(start.elapsed().as_secs_f64() * 1000.);
                    })
                    .expect("step");
                }
                let (p50, p95) = summarize(samples);
                println!("  arrow key + frame: {p50:.2} ms p50, {p95:.2} ms p95");
                // A keystroke in the search: the filter over 10,000 rows,
                // the list reset, and the frame. The field's change lands
                // as an event after the keystroke's update, so the frame
                // that shows the narrowed rows is the next one.
                let mut samples = Vec::with_capacity(20);
                for i in 0..20 {
                    cx.update_window(handle, |_, window, cx| {
                        let start = Instant::now();
                        if i % 2 == 0 {
                            window.input("9", cx);
                        } else {
                            window.press("backspace", cx);
                        }
                        samples.push(start.elapsed().as_secs_f64() * 1000.);
                    })
                    .expect("type");
                    cx.update_window(handle, |_, window, cx| {
                        let start = Instant::now();
                        window.render_frame(cx);
                        let last = samples.len() - 1;
                        samples[last] += start.elapsed().as_secs_f64() * 1000.;
                    })
                    .expect("render");
                }
                let (p50, p95) = summarize(samples);
                println!("  search keystroke + filter + frame: {p50:.2} ms p50, {p95:.2} ms p95");
            }
            cx.update_window(handle, |_, window, cx| {
                window.press("escape", cx);
                window.render_frame(cx);
            })
            .expect("close");
        }

        let before = resident_mb();
        let mut samples = Vec::with_capacity(50);
        let mut after_ten = before;
        for i in 0..50 {
            cx.update_window(handle, |_, window, cx| {
                let start = Instant::now();
                window.click(child("numbers", "trigger"), cx);
                window.render_frame(cx);
                window.press("escape", cx);
                window.render_frame(cx);
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            })
            .expect("cycle");
            if i == 9 {
                after_ten = resident_mb();
            }
        }
        let after = resident_mb();
        let (p50, p95) = summarize(samples);
        println!(
            "  open + close 10,000 rows x50: {p50:.2} ms p50, {p95:.2} ms p95; resident {before:.0} MB -> {after_ten:.0} MB after 10 -> {after:.0} MB after 50"
        );
        println!(
            "  cpu {:.0} ms over {:.0} ms wall for this section",
            cpu_ms() - cpu_start,
            wall_start.elapsed().as_secs_f64() * 1000.
        );
    }

    /// The terminal engine with no window: a second of steady output (64
    /// KB every millisecond) through a stream, to a pane that is painted
    /// (a frame granted after every frame, more than a display grants), one
    /// that is hidden, and one that is never painted, as on a page that is
    /// not shown. Then what a quiet terminal costs.
    #[cfg(feature = "terminal")]
    fn terminal() {
        use std::time::Duration;

        use gpui_cn::terminal::{
            Engine, EngineOptions, FrameSink, FrameSource as _, StartOptions, StreamSource,
            Viewport,
        };

        println!("terminal engine:");
        let mut chunk = Vec::new();
        let mut line = 0;
        while chunk.len() < 64 << 10 {
            chunk.extend_from_slice(
                format!("line {line} the quick brown fox jumps over the lazy dog\r\n").as_bytes(),
            );
            line += 1;
        }
        for mode in ["painted", "hidden", "not painted"] {
            let (source, peer) = StreamSource::new();
            let (sink, _wake) = FrameSink::new();
            let viewport = Viewport::new(120, 40, 16, 32).expect("a grid");
            let handle = Box::new(Engine::new(source, EngineOptions::default()))
                .start(
                    sink.clone(),
                    StartOptions::default().with_viewport(viewport),
                )
                .expect("start");
            while sink.take().is_none() {
                std::thread::sleep(Duration::from_millis(1));
            }
            match mode {
                "painted" => handle.request_frame(),
                "hidden" => handle.set_visible(false),
                _ => {}
            }
            let cpu_start = cpu_ms();
            let start = Instant::now();
            let mut sent = 0usize;
            let mut frames = 0;
            while start.elapsed() < Duration::from_secs(1) {
                peer.output(&chunk);
                sent += chunk.len();
                if sink.take().is_some() {
                    frames += 1;
                    if mode == "painted" {
                        handle.request_frame();
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            peer.output(b"LAST");
            let cpu = cpu_ms() - cpu_start;
            // One frame proves the output was parsed, not dropped.
            match mode {
                "hidden" => handle.set_visible(true),
                _ => handle.request_frame(),
            }
            let parsed = loop {
                if let Some(snapshot) = sink.take() {
                    if snapshot.frame.text().contains("LAST") {
                        break true;
                    }
                    handle.request_frame();
                }
                if start.elapsed() > Duration::from_secs(10) {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(1));
            };
            println!(
                "  {mode:<11} {:>5.1} MB in 1 s: {frames:>4} frames, cpu {cpu:>4.0} ms, all parsed: {parsed}",
                sent as f64 / (1 << 20) as f64,
            );
            let idle_cpu = cpu_ms();
            let idle = Instant::now();
            let mut idle_frames = 0;
            while idle.elapsed() < Duration::from_millis(500) {
                if sink.take().is_some() {
                    idle_frames += 1;
                    handle.request_frame();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            println!(
                "  {:<11} then 500 ms quiet: {idle_frames} frames, cpu {:.1} ms",
                "",
                cpu_ms() - idle_cpu
            );
            handle.close();
        }
    }

    /// This process' physical footprint in megabytes, what Activity
    /// Monitor shows as Memory. Freed pages macOS keeps for reuse do not
    /// count, unlike the resident set.
    #[cfg(feature = "terminal")]
    fn footprint_mb() -> f64 {
        let out = std::process::Command::new("footprint")
            .args(["-p", &std::process::id().to_string()])
            .output()
            .expect("footprint");
        let text = String::from_utf8_lossy(&out.stdout);
        let mut words = text
            .lines()
            .find(|line| line.contains("Footprint:"))
            .unwrap_or_default()
            .split_whitespace()
            .skip_while(|word| *word != "Footprint:")
            .skip(1);
        let value: f64 = words.next().and_then(|v| v.parse().ok()).unwrap_or(0.);
        match words.next().unwrap_or("MB") {
            unit if unit.starts_with('K') => value / 1024.,
            unit if unit.starts_with('G') => value * 1024.,
            _ => value,
        }
    }

    /// Parking: ten terminals with full scrollback, live and then parked
    /// after their idle period, and how long the first output after it
    /// takes to show.
    #[cfg(feature = "terminal")]
    fn parking() {
        use std::time::Duration;

        use gpui_cn::terminal::park::{MemoryStore, ParkOptions};
        use gpui_cn::terminal::{
            Engine, EngineOptions, FrameSink, FrameSource as _, StartOptions, StreamSource,
            Viewport,
        };

        println!("terminal parking:");
        let idle = Duration::from_millis(500);
        let store = Arc::new(MemoryStore::new());
        let base = footprint_mb();
        let terminals: Vec<_> = (0..10)
            .map(|n| {
                let (source, peer) = StreamSource::new();
                let (sink, _wake) = FrameSink::new();
                let options = EngineOptions::default().with_park(
                    ParkOptions::default()
                        .with_idle(idle)
                        .with_shared_store(store.clone()),
                );
                let handle = Box::new(Engine::new(source, options))
                    .start(
                        sink.clone(),
                        StartOptions::default()
                            .with_viewport(Viewport::new(120, 40, 16, 32).expect("a grid")),
                    )
                    .expect("start");
                let mut chunk = Vec::new();
                for line in 0..200_000 {
                    chunk.extend_from_slice(
                        format!(
                            "line {line} of {n} the quick brown fox jumps over the lazy dog\r\n"
                        )
                        .as_bytes(),
                    );
                    if chunk.len() > 60 << 10 {
                        peer.output(&chunk);
                        chunk.clear();
                    }
                }
                peer.output(&chunk);
                (peer, sink, handle)
            })
            .collect();
        // Every terminal shows its last line, so it parsed everything;
        // then nothing paints them.
        for (n, (_, sink, handle)) in terminals.iter().enumerate() {
            let last = format!("line 199999 of {n}");
            handle.request_frame();
            loop {
                if let Some(snapshot) = sink.take() {
                    if snapshot.frame.text().contains(&last) {
                        break;
                    }
                    handle.request_frame();
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let live = footprint_mb() - base;
        std::thread::sleep(idle * 3);
        let parked = footprint_mb() - base;
        println!(
            "  10 terminals, 10 MB scrollback each: live {live:.0} MB, parked {parked:.0} MB (snapshots {:.1} MB)",
            store.len_bytes() as f64 / (1 << 20) as f64
        );
        let mut samples = Vec::new();
        for (peer, sink, handle) in &terminals {
            let _ = sink.take();
            let start = Instant::now();
            peer.output(b"\r\nback");
            handle.request_frame();
            loop {
                if let Some(snapshot) = sink.take() {
                    if snapshot.frame.text().contains("back") {
                        break;
                    }
                    handle.request_frame();
                }
                std::thread::sleep(Duration::from_micros(100));
            }
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        let (p50, p95) = summarize(samples);
        println!(
            "  first output after parking to its frame: {p50:.2} ms p50, {p95:.2} ms p95; restored {:.0} MB",
            footprint_mb() - base
        );
        for (_, _, handle) in &terminals {
            handle.close();
        }
    }

    /// A window that is one terminal with a full, colored 120-column
    /// screen: what an unchanged frame costs (a cursor blink, a hover), and
    /// a frame right after the font size changes, back and forth.
    #[cfg(feature = "terminal")]
    fn grid(cx: &mut HeadlessAppContext, frames: usize) {
        use std::time::Duration;

        use gpui_cn::terminal::{
            Engine, EngineOptions, StreamSource, Terminal, TerminalConfig, TerminalState,
        };
        use gpui_kit::{IntoElement, ParentElement as _, Render, Styled as _, Window, div};

        struct Grid(Entity<TerminalState>);
        impl Render for Grid {
            fn render(
                &mut self,
                _: &mut Window,
                _: &mut gpui_kit::Context<Self>,
            ) -> impl IntoElement {
                div().size_full().child(Terminal::new("grid", &self.0))
            }
        }

        println!("terminal grid (one window, 120 columns of colored text):");
        let (source, peer) = StreamSource::new();
        let mut text = String::new();
        for line in 0..400 {
            for word in 0..12 {
                text.push_str(&format!(
                    "\x1b[3{}m{line:04}-{word:02}\x1b[0m ",
                    (line + word) % 7 + 1
                ));
            }
            text.push_str("\r\n");
        }
        peer.output(text.as_bytes());
        let mut state = None;
        let handle = cx
            .open_window(gpui_kit::size(px(1100.), px(760.)), |window, cx| {
                let terminal = cx.new(|cx| {
                    let engine = Engine::new(source, EngineOptions::default());
                    TerminalState::new(engine, TerminalConfig::default(), window, cx)
                });
                state = Some(terminal.clone());
                let view = cx.new(|cx| {
                    cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
                    Grid(terminal)
                });
                cx.new(|cx| gpui_kit::base::Root::new(view, window, cx))
            })
            .expect("open the grid window");
        let handle: AnyWindowHandle = handle.into();
        let state = state.expect("the terminal");
        // Let the engine parse and the window draw the full screen.
        for _ in 0..50 {
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                .expect("render");
            std::thread::sleep(Duration::from_millis(5));
        }
        let (rows, columns, filled) = cx.update(|cx| {
            let frame = state.read(cx).frame().clone();
            let filled = frame
                .rows
                .iter()
                .filter(|row| !row.text.trim().is_empty())
                .count();
            (frame.viewport.rows(), frame.viewport.columns(), filled)
        });
        println!("  grid {columns}x{rows}, {filled} rows with text");
        let (full_p50, full_p95) = measure(cx, handle, frames, true);
        println!("  unchanged frame: {full_p50:.2} ms p50, {full_p95:.2} ms p95");
        let mut samples = Vec::with_capacity(frames);
        for i in 0..frames {
            let size = if i % 2 == 0 { px(15.) } else { px(13.) };
            cx.update(|cx| state.update(cx, |state, cx| state.set_font_size(size, cx)));
            cx.update_window(handle, |_, window, cx| {
                let start = Instant::now();
                window.render_frame(cx);
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            })
            .expect("render");
        }
        let (p50, p95) = summarize(samples);
        println!(
            "  frame after a font size change, 13 <-> 15 px: {p50:.2} ms p50, {p95:.2} ms p95"
        );
        cx.update(|cx| state.update(cx, |state, cx| state.close(cx)));
    }
}
