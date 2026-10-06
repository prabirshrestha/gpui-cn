//! The gpui-cn component gallery.
//!
//! `--exercise` drives the gallery by itself, forever: it scrolls the
//! page, slides the sidebar, and walks the stories, so a profiler can
//! watch it without a hand on the mouse.

use gpui_cn::TitleBar;
use gpui_cn_story::{
    Gallery, NavigateBack, NavigateForward, OpenSettings, TogglePerformanceHud, ToggleSidebar,
    WINDOW_SIZE,
};
use gpui_kit::*;

actions!(
    story,
    [
        /// Quits the gallery.
        Quit
    ]
);

fn main() {
    let t_main = std::time::Instant::now();
    let trace = std::env::var_os("GPUI_CN_STARTUP_TRACE").is_some();
    let mark = move |what: &str| {
        if trace {
            eprintln!(
                "startup: {what} at {:.1} ms",
                t_main.elapsed().as_secs_f64() * 1000.
            );
        }
    };
    let app = gpui_kit::application().with_assets(gpui_cn::ComposerAssets);
    mark("application built");
    app.run(move |cx| {
        mark("run callback");
        // The host initializes base; gpui-cn adds its theme on top.
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        mark("init done");
        cx.bind_keys([
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-q", Quit, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("alt-f4", Quit, None),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-,", OpenSettings, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("ctrl-,", OpenSettings, None),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("ctrl-b", ToggleSidebar, None),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-[", NavigateBack, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("alt-left", NavigateBack, None),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-]", NavigateForward, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("alt-right", NavigateForward, None),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-shift-p", TogglePerformanceHud, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("ctrl-shift-p", TogglePerformanceHud, None),
        ]);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.set_menus(vec![
            Menu {
                name: "gpui-cn".into(),
                items: vec![
                    MenuItem::action("Settings...", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Quit", Quit),
                ],
                disabled: false,
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Toggle Sidebar", ToggleSidebar),
                    MenuItem::separator(),
                    MenuItem::action("Back", NavigateBack),
                    MenuItem::action("Forward", NavigateForward),
                    #[cfg(feature = "fps")]
                    MenuItem::separator(),
                    #[cfg(feature = "fps")]
                    MenuItem::action("Performance HUD", TogglePerformanceHud),
                ],
                disabled: false,
            },
        ]);
        cx.activate(true);
        let bounds = Bounds::centered(None, WINDOW_SIZE, cx);
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("gpui-cn".into()),
                ..TitleBar::title_bar_options(cx)
            }),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..TitleBar::window_options(cx)
        };
        // The window opens here, not in a spawned task: a task waits
        // for the next turn of the run loop, and the Dock icon is
        // bouncing meanwhile.
        let (handle, gallery) = gpui_kit::open_window(options, cx, |window, cx| {
            mark("window created");
            let view = cx.new(|cx| Gallery::new(window, cx));
            let story = std::env::args().skip_while(|arg| arg != "--story").nth(1);
            if let Some(title) = story {
                view.update(cx, |gallery, cx| gallery.select_story(&title, window, cx));
            }
            window.on_next_frame(move |_, _| mark("first frame drawn"));
            view
        })
        .expect("failed to open window");
        mark("open_window returned");
        if std::env::args().any(|arg| arg == "--exercise") {
            cx.spawn(async move |cx| {
                gpui_cn_story::exercise(gallery, handle, cx).await;
            })
            .detach();
        }
    });
}
