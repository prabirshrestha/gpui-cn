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

    use gpui_cn::{ComposerAssets, ReduceMotion, Theme, ThemeMode};
    use gpui_cn_story::Gallery;
    use gpui_kit::{
        AppContext as _, Entity, HeadlessAppContext, px, size, test::TestWindowExt as _,
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
            Arc::new(ComposerAssets),
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
                    cx.new(|cx| gpui_kit::base::Root::new(view, window, cx))
                },
            )
            .expect("open the gallery window");
        let gallery = gallery.expect("the gallery view");

        let capture = |cx: &mut HeadlessAppContext, name: &str| {
            cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
                .expect("render");
            // Pictures decode off the main thread; wait for them.
            cx.run_until_parked();
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

        let snap_picker =
            |cx: &mut HeadlessAppContext, parent: gpui_kit::ElementId, prefix: &str, name: &str| {
                let part = |name: &'static str| {
                    gpui_kit::ElementId::NamedChild(parent.clone().into(), name.into())
                };
                cx.update_window(handle.into(), |_, window, cx| {
                    window.scroll(
                        "page",
                        gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                            gpui_kit::px(0.),
                            gpui_kit::px(100000.),
                        )),
                        cx,
                    );
                    window.render_frame(cx);
                    window.render_frame(cx);
                    gpui_cn_story::reveal(part("trigger"), window, cx);
                    window.click(part("trigger"), cx);
                    window.render_frame(cx);
                    window.render_frame(cx);
                })
                .expect("open the model picker");
                capture(cx, &format!("{prefix}-models-{name}"));
                for (suffix, click, query) in [
                    ("favorites", Some("favorites"), None),
                    ("legacy", Some("provider-codex"), None),
                    ("search", None, Some("opus")),
                    ("empty", None, Some("zzzz")),
                ] {
                    cx.update_window(handle.into(), |_, window, cx| {
                        match (click, query) {
                            (Some("favorites"), _) => window.click(part("favorites"), cx),
                            (Some(_), _) => {
                                window.click(
                                    gpui_kit::ElementId::NamedChild(
                                        part("provider").into(),
                                        "codex".into(),
                                    ),
                                    cx,
                                );
                                window.render_frame(cx);
                                window.click(part("legacy"), cx);
                            }
                            (None, Some(query)) => {
                                window.press("cmd-a", cx);
                                window.input(query, cx);
                            }
                            _ => {}
                        }
                        window.render_frame(cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                    })
                    .expect("show a model picker view");
                    capture(cx, &format!("{prefix}-models-{suffix}-{name}"));
                }
                cx.update_window(handle.into(), |_, window, cx| {
                    window.press("escape", cx);
                    window.render_frame(cx);
                })
                .expect("close the model picker");
            };

        for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            cx.update(|cx| Theme::change(mode, cx));
            capture(&mut cx, &format!("gallery-{name}"));
            cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.open_palette(cx)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.render_frame(cx);
            })
            .expect("draw the palette");
            capture(&mut cx, &format!("gallery-{name}-palette"));
            cx.update_window(handle.into(), |_, window, cx| {
                window.press("escape", cx);
                window.render_frame(cx);
            })
            .expect("close the palette");
            cx.update_window(handle.into(), |_, window, cx| {
                window.scroll(
                    "sidebar-content",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                        gpui_kit::px(0.),
                        gpui_kit::px(-300.),
                    )),
                    cx,
                );
                window.render_frame(cx);
                window.render_frame(cx);
            })
            .expect("scroll the sidebar");
            capture(&mut cx, &format!("gallery-{name}-scrolled"));
            cx.update_window(handle.into(), |_, window, cx| {
                window.scroll(
                    "sidebar-content",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                        gpui_kit::px(0.),
                        gpui_kit::px(1000.),
                    )),
                    cx,
                );
                window.render_frame(cx);
            })
            .expect("scroll the sidebar back");
            for (query, label) in [("sel", "filtered"), ("zzzz", "unmatched")] {
                cx.update_window(handle.into(), |_, window, cx| {
                    window.render_frame(cx);
                    window.click("story-filter", cx);
                    window.press("cmd-a", cx);
                    window.input(query, cx);
                    window.render_frame(cx);
                })
                .expect("filter the story list");
                capture(&mut cx, &format!("gallery-{name}-{label}"));
            }
            cx.update_window(handle.into(), |_, window, cx| {
                window.press("escape", cx);
                window.render_frame(cx);
            })
            .expect("clear the filter");
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
                "Color",
                "Spacing",
                "Switch",
                "Slider",
                "Radio",
                "Input",
                "Textarea",
                "Select",
                "Menu",
                "Command",
                "Popover",
                "Dialog",
                "File picker",
                "Folder picker",
                "Composer",
                "Model picker",
                "Avatar",
                "Badge",
                "Tag",
                "Skeleton",
                "Progress",
                "Theme",
                "Sidebar",
                "Nav stack",
                "Scroll area",
                "Title bar",
                "Tabs",
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
                // The caret goes into the first field, so the focused
                // hairline and the text sit beside the resting ones; the
                // search field gets text so its clear button shows.
                let fields: &[&str] = match story {
                    "Input" => &["search", "name"],
                    "Textarea" => &["notes"],
                    _ => &[],
                };
                for &field in fields {
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.click(field, cx);
                        // The window is reused per appearance, so the
                        // text of the last pass is replaced, not appended.
                        window.press("cmd-a", cx);
                        window.input("Ada Lovelace", cx);
                        window.render_frame(cx);
                    })
                    .expect("type into the field");
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
                if story == "Composer" {
                    let named = |parent: &'static str, child: &'static str| {
                        gpui_kit::ElementId::NamedChild(
                            gpui_kit::ElementId::Name(parent.into()).into(),
                            child.into(),
                        )
                    };
                    let status_branch = gpui_kit::ElementId::NamedChild(
                        named("composer-agent-status", "branch").into(),
                        "trigger".into(),
                    );
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        gpui_cn_story::reveal(status_branch.clone(), window, cx);
                        window.click(status_branch.clone(), cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                    })
                    .expect("open a status dropdown");
                    capture(&mut cx, &format!("story-{slug}-status-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.render_frame(cx);
                    })
                    .expect("close the status dropdown");
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.click(named("composer-basic", "text"), cx);
                        window.press("cmd-a", cx);
                        window.input("Summarize the attached forecast", cx);
                        window.press("shift-enter", cx);
                        window.input("and list the three biggest risks.", cx);
                        window.render_frame(cx);
                    })
                    .expect("type a prompt");
                    capture(&mut cx, &format!("story-{slug}-typed-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        gpui_cn_story::reveal(named("permission", "trigger"), window, cx);
                        window.click(named("permission", "trigger"), cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                        window.hover(named("permission", "manual"), cx);
                        window.render_frame(cx);
                    })
                    .expect("open the permission menu");
                    capture(&mut cx, &format!("story-{slug}-permission-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.render_frame(cx);
                    })
                    .expect("close the permission menu");
                    snap_picker(
                        &mut cx,
                        named("composer-agent", "models"),
                        &format!("story-{slug}"),
                        name,
                    );
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        let trigger = gpui_kit::ElementId::NamedChild(
                            named("composer-agent", "effort").into(),
                            "trigger".into(),
                        );
                        window.scroll(
                            "page",
                            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                                gpui_kit::px(0.),
                                gpui_kit::px(100000.),
                            )),
                            cx,
                        );
                        window.render_frame(cx);
                        window.render_frame(cx);
                        window.click(trigger, cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                    })
                    .expect("open the effort menu");
                    capture(&mut cx, &format!("story-{slug}-effort-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.render_frame(cx);
                    })
                    .expect("close the effort menu");
                }
                if story == "Color" {
                    cx.update_window(handle.into(), |_, window, cx| {
                        let search = gallery
                            .read(cx)
                            .current_story::<gpui_cn_story::stories::ColorStory>(cx)
                            .expect("the color story")
                            .read(cx)
                            .search()
                            .clone();
                        search.update(cx, |input, cx| input.set_value("focus", window, cx));
                        window.render_frame(cx);
                        window.render_frame(cx);
                    })
                    .expect("search the colors");
                    capture(&mut cx, &format!("story-{slug}-search-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        let search = gallery
                            .read(cx)
                            .current_story::<gpui_cn_story::stories::ColorStory>(cx)
                            .expect("the color story")
                            .read(cx)
                            .search()
                            .clone();
                        search.update(cx, |input, cx| input.set_value("", window, cx));
                        window.render_frame(cx);
                    })
                    .expect("clear the search");
                }
                if story == "Model picker" {
                    snap_picker(
                        &mut cx,
                        gpui_kit::ElementId::Name("models".into()),
                        &format!("story-{slug}"),
                        name,
                    );
                }
                if story == "Menu" {
                    use gpui_cn_story::stories::MenuStory;
                    let named = |parent: &'static str, child: &'static str| {
                        gpui_kit::ElementId::NamedChild(
                            gpui_kit::ElementId::Name(parent.into()).into(),
                            child.into(),
                        )
                    };
                    // The document holds the caret, so Edit acts on it and
                    // the menus show the story's shortcuts.
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.click(MenuStory::DOCUMENT, cx);
                    })
                    .expect("focus the document");
                    for title in ["File", "View"] {
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.render_frame(cx);
                            window.click(named(MenuStory::MENU_BAR, title), cx);
                            window.render_frame(cx);
                            window.render_frame(cx);
                        })
                        .expect("open a menu of the bar");
                        let slug_title = title.to_lowercase();
                        capture(&mut cx, &format!("story-{slug}-bar-{slug_title}-{name}"));
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.press("escape", cx);
                        })
                        .expect("close the menu");
                    }
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.click(MenuStory::OPTIONS, cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                        window.hover(named("options", "share"), cx);
                        window.render_frame(cx);
                    })
                    .expect("open the dropdown");
                    capture(&mut cx, &format!("story-{slug}-dropdown-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.press("escape", cx);
                        window.render_frame(cx);
                        gpui_cn_story::reveal(MenuStory::REGION, window, cx);
                        window.right_click(MenuStory::REGION, cx);
                        window.render_frame(cx);
                    })
                    .expect("open the context menu");
                    capture(&mut cx, &format!("story-{slug}-context-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.render_frame(cx);
                        gpui_cn_story::reveal("dated", window, cx);
                        window.click("dated", cx);
                        window.press("cmd-a", cx);
                        window.input("Remember the milk, the eggs, and the bread", cx);
                        window.press("cmd-a", cx);
                        window.right_click("dated", cx);
                    })
                    .expect("right-click the field");
                    capture(&mut cx, &format!("story-{slug}-field-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.scroll(
                            "page",
                            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(2000.))),
                            cx,
                        );
                    })
                    .expect("close the field's menu");
                }
                if story == "Command" {
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        gpui_cn_story::reveal("command-dialog-trigger", window, cx);
                        window.click("command-dialog-trigger", cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                    })
                    .expect("open the palette");
                    capture(&mut cx, &format!("story-{slug}-dialog-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                    })
                    .expect("close the palette");
                }
                if story == "Dialog" {
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.click(gpui_cn_story::stories::DialogStory::TRIGGER, cx);
                        window.render_frame(cx);
                        window.render_frame(cx);
                    })
                    .expect("open the dialog");
                    capture(&mut cx, &format!("story-{slug}-open-{name}"));
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.render_frame(cx);
                    })
                    .expect("close the dialog");
                }
                if story == "File picker" {
                    for (trigger, suffix, steps) in [
                        (
                            gpui_cn_story::stories::FilePickerStory::TRIGGER_ONE,
                            "one",
                            &[("", None), ("-selected", Some("main.rs"))][..],
                        ),
                        (
                            gpui_cn_story::stories::FilePickerStory::TRIGGER_MANY,
                            "many",
                            &[("", None), ("-selected", Some("notes.txt"))][..],
                        ),
                    ] {
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.render_frame(cx);
                            window.click(trigger, cx);
                        })
                        .expect("open the picker");
                        for (extra, click) in steps {
                            cx.run_until_parked();
                            cx.update_window(handle.into(), |_, window, cx| {
                                window.render_frame(cx);
                                window.render_frame(cx);
                                if let Some(file) = click {
                                    let id = gpui_kit::ElementId::NamedChild(
                                        gpui_kit::ElementId::NamedChild(
                                            gpui_kit::ElementId::Name(
                                                format!("file-picker-{suffix}").into(),
                                            )
                                            .into(),
                                            "entry".into(),
                                        )
                                        .into(),
                                        (*file).into(),
                                    );
                                    window.click(id, cx);
                                    window.render_frame(cx);
                                }
                            })
                            .expect("draw the picker");
                            cx.run_until_parked();
                            cx.update_window(handle.into(), |_, window, cx| {
                                window.render_frame(cx);
                                window.render_frame(cx);
                            })
                            .expect("draw the picker");
                            capture(&mut cx, &format!("story-{slug}-{suffix}{extra}-{name}"));
                        }
                        if suffix == "one" {
                            cx.update(|cx| {
                                let story = gallery
                                    .read(cx)
                                    .current_story::<gpui_cn_story::stories::FilePickerStory>(cx)
                                    .expect("the file picker story");
                                let one = story.read(cx).one().clone();
                                one.update(cx, |state, cx| state.set_active_filter(1, cx));
                            });
                            cx.run_until_parked();
                            cx.update_window(handle.into(), |_, window, cx| {
                                window.render_frame(cx);
                                window.render_frame(cx);
                            })
                            .expect("draw the filtered picker");
                            capture(&mut cx, &format!("story-{slug}-one-filtered-{name}"));
                            cx.update(|cx| {
                                let story = gallery
                                    .read(cx)
                                    .current_story::<gpui_cn_story::stories::FilePickerStory>(cx)
                                    .expect("the file picker story");
                                let one = story.read(cx).one().clone();
                                one.update(cx, |state, cx| state.set_active_filter(0, cx));
                            });
                        }
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.press("escape", cx);
                            window.render_frame(cx);
                        })
                        .expect("close the picker");
                    }
                }
                if story == "Folder picker" {
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        window.click(gpui_cn_story::stories::FolderPickerStory::TRIGGER, cx);
                    })
                    .expect("open the picker");
                    for (suffix, text) in [
                        // The gallery is reused per appearance, so the path is set again.
                        ("home", Some("~/")),
                        ("tilde", Some("~/co")),
                        ("open", Some("/home/prabirshrestha/")),
                        ("filtered", Some("/home/prabirshrestha/co")),
                        ("error", Some("/home/prabirshrestha/code/psl/")),
                        ("empty", Some("/home/prabirshrestha/.zed_server/")),
                        ("more", Some("/home/prabirshrestha/code/")),
                    ] {
                        if let Some(text) = text {
                            cx.update_window(handle.into(), |_, window, cx| {
                                window.press("cmd-a", cx);
                                window.input(text, cx);
                            })
                            .expect("type a path");
                        }
                        cx.run_until_parked();
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.render_frame(cx);
                            window.render_frame(cx);
                        })
                        .expect("draw the picker");
                        capture(&mut cx, &format!("story-{slug}-{suffix}-{name}"));
                    }
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.press("escape", cx);
                        window.render_frame(cx);
                    })
                    .expect("close the picker");
                }
                if story == "Tabs" {
                    let add = gpui_kit::ElementId::NamedChild(
                        gpui_kit::ElementId::Name("tabs-menu".into()).into(),
                        "add".into(),
                    );
                    for query in ["", "er"] {
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.render_frame(cx);
                            gpui_cn_story::reveal(add.clone(), window, cx);
                            window.click(add.clone(), cx);
                            window.render_frame(cx);
                            window.render_frame(cx);
                            if !query.is_empty() {
                                window.input(query, cx);
                            }
                        })
                        .expect("open the new-tab menu");
                        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
                            .expect("draw the rows that match");
                        let suffix = if query.is_empty() { "" } else { "-search" };
                        capture(&mut cx, &format!("story-{slug}-add-menu{suffix}-{name}"));
                        cx.update_window(handle.into(), |_, window, cx| {
                            // Escape clears a query first, then closes.
                            window.press("escape", cx);
                            window.press("escape", cx);
                        })
                        .expect("close the menu");
                    }
                }
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
