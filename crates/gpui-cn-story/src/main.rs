//! The gpui-cn component gallery.

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
            ]);
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.set_menus(vec![Menu {
                name: "gpui-cn".into(),
                items: vec![MenuItem::action("Quit", Quit)],
                disabled: false,
            }]);
            cx.activate(true);
            let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
            cx.spawn(async move |cx| {
                let options = WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("gpui-cn".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                };
                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| gpui_cn_story::Gallery::new(window, cx));
                    cx.new(|cx| gpui_cn::Root::new(view, window, cx))
                })
                .expect("failed to open window");
            })
            .detach();
        });
}
