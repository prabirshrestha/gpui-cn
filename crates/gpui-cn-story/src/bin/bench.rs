//! Measures what a frame of each gallery page costs, offscreen.
//!
//! ```bash
//! cargo run --release -p gpui-cn-story --features snapshot --bin bench -- [frames]
//! ```
//!
//! For each page it reports the median and the slowest-in-20 cost of a
//! full frame (every view re-rendered, laid out, and painted, which is
//! what a resize or a theme change costs) and of a frame after nothing
//! changed (what an idle tick costs). Runs on macOS, where GPUI has a
//! headless Metal renderer.

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
                cx.new(|cx| gpui_cn::Root::new(view, window, cx))
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
    }
}
