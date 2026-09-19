//! Renders the gallery offscreen and writes PNG files, one per theme mode.
//!
//! ```bash
//! cargo run -p gpui-cn-story --features snapshot --bin snapshot -- out-dir
//! ```
//!
//! Needs GPUI's Metal headless renderer, so it runs on macOS only. On other
//! platforms it prints that it skipped.

fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(not(target_os = "macos"))]
    println!("snapshot: skipped; GPUI has no headless renderer on this platform");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{path::PathBuf, sync::Arc};

    use gpui_cn::{ReduceMotion, Theme, ThemeMode};
    use gpui_kit::{
        AppContext as _, HeadlessAppContext, assets::Assets, px, size, test::TestWindowExt as _,
    };

    pub fn run() {
        let out = PathBuf::from(
            std::env::args()
                .nth(1)
                .unwrap_or_else(|| "snapshots".into()),
        );
        std::fs::create_dir_all(&out).expect("create the output directory");

        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(Assets),
            gpui_kit::platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::init(cx);
            gpui_cn::init(cx);
            // Snapshots want final states, not a frame mid-fade.
            Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
        });

        let handle = cx
            .open_window(size(px(1100.), px(1500.)), |window, cx| {
                let view = cx.new(|cx| gpui_cn_story::Gallery::new(window, cx));
                cx.new(|cx| gpui_cn::Root::new(view, window, cx))
            })
            .expect("open the gallery window");

        for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            cx.update(|cx| Theme::change(mode, cx));
            cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
                .expect("render");
            let image = cx
                .capture_screenshot(handle.into())
                .expect("Metal rendering must be available");
            let path = out.join(format!("gallery-{name}.png"));
            image.save(&path).expect("write the PNG");
            println!("{}", path.display());
        }
    }
}
