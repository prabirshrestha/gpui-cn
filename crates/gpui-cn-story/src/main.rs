//! The gpui-cn component gallery.

use gpui_cn::TitleBar;
use gpui_cn_story::{
    Gallery, NavigateBack, NavigateForward, OpenSettings, ToggleSidebar, WINDOW_SIZE,
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
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            // The host initializes base; gpui-cn adds its theme on top.
            gpui_kit::init(cx);
            gpui_cn::init(cx);
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
            cx.spawn(async move |cx| {
                cx.open_window(options, |window, cx| {
                    let gallery = cx.new(|cx| Gallery::new(window, cx));
                    // The menu and its shortcuts reach the gallery through
                    // application-level handlers, so they work wherever
                    // focus is.
                    cx.on_action({
                        let gallery = gallery.clone();
                        move |_: &OpenSettings, cx| {
                            gallery.update(cx, |gallery, cx| gallery.open_settings(cx))
                        }
                    });
                    cx.on_action({
                        let gallery = gallery.clone();
                        move |_: &ToggleSidebar, cx| {
                            gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx))
                        }
                    });
                    cx.on_action({
                        let gallery = gallery.clone();
                        move |_: &NavigateBack, cx| {
                            gallery.update(cx, |gallery, cx| gallery.go_back(cx))
                        }
                    });
                    cx.on_action({
                        let gallery = gallery.clone();
                        move |_: &NavigateForward, cx| {
                            gallery.update(cx, |gallery, cx| gallery.go_forward(cx))
                        }
                    });
                    cx.new(|cx| gpui_cn::Root::new(gallery, window, cx))
                })
                .expect("failed to open window");
            })
            .detach();
        });
}
