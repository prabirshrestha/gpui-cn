//! Renders the gallery offscreen and writes PNG files: the components page
//! with the sidebar open and closed, and the settings page, per theme
//! mode.
//!
//! ```bash
//! cargo run -p gpui-cn-story --features snapshot --bin snapshot -- out-dir [height]
//! ```
//!
//! The window is 1100px wide and 760px tall unless a height is given; a
//! taller window shows a story that scrolls.
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
    use gpui_cn_story::Gallery;
    use gpui_kit::{
        AppContext as _, Entity, HeadlessAppContext, assets::Assets, px, size,
        test::TestWindowExt as _,
    };

    pub fn run() {
        let out = PathBuf::from(
            std::env::args()
                .nth(1)
                .unwrap_or_else(|| "snapshots".into()),
        );
        std::fs::create_dir_all(&out).expect("create the output directory");

        let height: f32 = std::env::args()
            .nth(2)
            .and_then(|height| height.parse().ok())
            .unwrap_or_else(|| f32::from(gpui_cn_story::WINDOW_SIZE.height));
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

        let mut gallery: Option<Entity<Gallery>> = None;
        let handle = cx
            .open_window(
                size(gpui_cn_story::WINDOW_SIZE.width, px(height)),
                |window, cx| {
                    let view = cx.new(|cx| Gallery::new(window, cx));
                    gallery = Some(view.clone());
                    cx.new(|cx| gpui_cn::Root::new(view, window, cx))
                },
            )
            .expect("open the gallery window");
        let gallery = gallery.expect("the gallery view");

        let capture = |cx: &mut HeadlessAppContext, name: &str| {
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.render_frame(cx);
            })
            .expect("render");
            let image = cx
                .capture_screenshot(handle.into())
                .expect("Metal rendering must be available");
            let path = out.join(format!("{name}.png"));
            image.save(&path).expect("write the PNG");
            println!("{}", path.display());
        };

        for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            cx.update(|cx| Theme::change(mode, cx));
            capture(&mut cx, &format!("gallery-{name}"));
            // Closed: off the canvas, the shell's default, then the rail.
            cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx)));
            capture(&mut cx, &format!("gallery-{name}-collapsed"));
            cx.update(|cx| {
                gallery.update(cx, |gallery, cx| {
                    gallery.sidebar().update(cx, |state, cx| {
                        state.set_collapsible(gpui_cn::SidebarCollapsible::Icon, cx)
                    })
                })
            });
            capture(&mut cx, &format!("gallery-{name}-rail"));
            cx.update(|cx| {
                gallery.update(cx, |gallery, cx| {
                    gallery.sidebar().update(cx, |state, cx| {
                        state.set_collapsible(gpui_cn::SidebarCollapsible::Offcanvas, cx)
                    });
                    gallery.toggle_sidebar(cx);
                })
            });
            cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.open_settings(cx)));
            capture(&mut cx, &format!("settings-{name}"));
            cx.update(|cx| {
                gallery.update(cx, |gallery, cx| {
                    gallery.settings().update(cx, |settings, cx| {
                        settings.show(gpui_cn_story::SettingsSection::Appearance, cx)
                    })
                })
            });
            capture(&mut cx, &format!("settings-{name}-appearance"));
            cx.update(|cx| {
                gallery.update(cx, |gallery, cx| {
                    gallery.settings().update(cx, |settings, cx| {
                        settings.show(gpui_cn_story::SettingsSection::General, cx)
                    })
                })
            });
            cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.go_back(cx)));
            for story in [
                "Typography",
                "Spacing",
                "Switch",
                "Select",
                "Skeleton",
                "Theme mode picker",
                "Sidebar",
                "Nav stack",
                "Scroll area",
                "Title bar",
            ] {
                cx.update_window(handle.into(), |_, window, cx| {
                    gallery.update(cx, |gallery, cx| gallery.select_story(story, window, cx));
                })
                .expect("select the story");
                if story == "Typography" {
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.scroll(
                            "page",
                            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-120.))),
                            cx,
                        );
                        window.render_frame(cx);
                    })
                    .expect("scroll the page");
                }
                if story == "Sidebar" {
                    // Hover the row under the selected one, so the two
                    // fills can be compared.
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.hover("starred", cx);
                    })
                    .expect("hover a row");
                }
                let slug = story.to_lowercase().replace(' ', "-");
                capture(&mut cx, &format!("story-{slug}-{name}"));
                if story == "Select" {
                    let trigger = |id: &'static str| {
                        gpui_kit::ElementId::NamedChild(
                            gpui_kit::ElementId::Name(id.into()).into(),
                            "trigger".into(),
                        )
                    };
                    for (id, query) in gpui_cn_story::stories::SelectStory::MENUS {
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.render_frame(cx);
                            gpui_cn_story::reveal(trigger(id), window, cx);
                            window.click(trigger(id), cx);
                            window.render_frame(cx);
                            window.render_frame(cx);
                            if !query.is_empty() {
                                window.input(query, cx);
                            }
                        })
                        .expect("open a menu");
                        if *id == "cities" {
                            cx.advance_clock(std::time::Duration::from_millis(400));
                            cx.run_until_parked();
                        }
                        let suffix = if query.is_empty() { "" } else { "-search" };
                        capture(&mut cx, &format!("story-{slug}-{id}{suffix}-{name}"));
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.press("escape", cx);
                        })
                        .expect("close the menu");
                    }
                }
            }
            cx.update_window(handle.into(), |_, window, cx| {
                gallery.update(cx, |gallery, cx| gallery.select_story("Button", window, cx));
            })
            .expect("select the story");
        }
    }
}
