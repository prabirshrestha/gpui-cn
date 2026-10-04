//! The Terminal story's tabs, splits and leader keys, with fixture panes.

#![cfg(feature = "terminal")]

use std::sync::Arc;

use gpui_cn::terminal::{TerminalColors, TerminalSnapshot, frame::Rgb};
use gpui_cn::{ActiveTheme as _, ReduceMotion, Theme};
use gpui_cn_story::{Gallery, stories::TerminalStory};
use gpui_kit::{
    AppContext as _, ElementId, Entity, TestAppContext, WindowHandle, base::Root, px, size,
    test::TestWindowExt as _,
};

fn setup(cx: &mut TestAppContext) -> WindowHandle<Root> {
    setup_gallery(cx).0
}

fn setup_gallery(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<Gallery>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
        TerminalStory::use_fixtures(cx);
    });
    let mut opened = None;
    let handle = cx.open_window(size(px(1100.), px(1000.)), |window, cx| {
        let gallery = cx.new(|cx| Gallery::new(window, cx));
        opened = Some(gallery.clone());
        gallery.update(cx, |gallery, cx| {
            gallery.select_story("Terminal", window, cx);
        });
        Root::new(gallery, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    (handle, opened.expect("the gallery"))
}

fn pane(n: u64) -> ElementId {
    ElementId::NamedInteger("terminal-pane".into(), n)
}

fn tab(id: &str) -> ElementId {
    ElementId::NamedChild(Arc::new("terminal-tabs".into()), id.to_owned().into())
}

/// Presses `keys` in the focused pane, one keystroke per entry.
fn press(handle: WindowHandle<Root>, cx: &mut TestAppContext, keys: &[&str]) {
    cx.update_window(handle.into(), |_, window, cx| {
        for key in keys {
            window.press(key, cx);
        }
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn shows(handle: WindowHandle<Root>, cx: &mut TestAppContext, id: ElementId) -> bool {
    cx.update_window(handle.into(), |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

#[gpui_kit::test]
fn leader_keys_split_panes_open_tabs_and_close_them(cx: &mut TestAppContext) {
    let handle = setup(cx);
    assert!(shows(handle, cx, pane(0)));
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();

    press(handle, cx, &["ctrl-a", "%"]);
    assert!(shows(handle, cx, pane(1)), "a split right opens a pane");
    let (left, right) = cx
        .update_window(handle.into(), |_, window, _| {
            (window.find(pane(0)).bounds(), window.find(pane(1)).bounds())
        })
        .unwrap();
    assert!(right.left() >= left.right(), "the new pane is on the right");

    press(handle, cx, &["ctrl-a", "c"]);
    assert!(shows(handle, cx, tab("shell-2")), "a new tab");
    assert!(shows(handle, cx, pane(2)));
    assert!(
        !shows(handle, cx, pane(0)),
        "the first tab's panes are hidden"
    );

    press(handle, cx, &["ctrl-a", "1"]);
    assert!(shows(handle, cx, pane(0)), "back on the first tab");

    press(handle, cx, &["ctrl-a", "x"]);
    assert!(
        !shows(handle, cx, pane(1)),
        "the focused pane, the new one, closes"
    );
    assert!(shows(handle, cx, pane(0)));
}

#[gpui_kit::test]
fn a_theme_change_reaches_every_pane_and_the_tab_strip(cx: &mut TestAppContext) {
    let (handle, gallery) = setup_gallery(cx);
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "%"]);
    let story = gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<TerminalStory>(cx))
        .expect("the Terminal story");

    // Ghostty's own defaults until a theme is picked.
    let (strip, window_background) = cx.update(|cx| {
        (
            Theme::global(cx)
                .scope(TerminalStory::SCOPE)
                .map(|t| t.background()),
            cx.theme().background(),
        )
    });
    assert_eq!(strip, Some(gpui_kit::rgb(0x282c34).into()));
    assert_ne!(strip, Some(window_background));

    let light = TerminalColors::new(Rgb(0x07, 0x36, 0x42), Rgb(0xfd, 0xf6, 0xe3));
    cx.update(|cx| story.update(cx, |story, cx| story.set_colors(light.clone(), cx)));
    // A pane opened after the change takes the new colors too.
    press(handle, cx, &["ctrl-a", "-"]);
    let terminals = story.read_with(cx, |story, _| story.terminals());
    assert_eq!(terminals.len(), 3);
    for terminal in terminals {
        terminal.read_with(cx, |terminal, _| assert_eq!(*terminal.colors(), light));
    }
    let strip = cx.update(|cx| {
        let tokens = Theme::global(cx).scope(TerminalStory::SCOPE).unwrap();
        (tokens.background(), tokens.foreground(), tokens.is_dark())
    });
    assert_eq!(strip.0, gpui_kit::rgb(0xfdf6e3).into());
    assert_eq!(strip.1, gpui_kit::rgb(0x073642).into());
    assert!(!strip.2, "a light terminal gives a light strip");
}

fn story(gallery: &Entity<Gallery>, cx: &mut TestAppContext) -> Entity<TerminalStory> {
    gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<TerminalStory>(cx))
        .expect("the Terminal story")
}

fn named(parent: ElementId, child: &str) -> ElementId {
    ElementId::NamedChild(Arc::new(parent), child.to_owned().into())
}

fn label(story: &Entity<TerminalStory>, id: &str, cx: &mut TestAppContext) -> String {
    story.read_with(cx, |story, cx| {
        story
            .tabs()
            .read(cx)
            .tabs()
            .iter()
            .find(|tab| tab.id() == id)
            .map(|tab| tab.label().to_string())
            .unwrap_or_default()
    })
}

#[gpui_kit::test]
fn typing_in_the_theme_picker_finds_a_theme_and_picking_it_applies_it(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("gpui-cn-themes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, background) in [
        ("Dracula", "#282a36"),
        ("Nord", "#2e3440"),
        ("Solarized Light", "#fdf6e3"),
    ] {
        std::fs::write(dir.join(name), format!("background = {background}\n")).unwrap();
    }
    cx.update(|cx| TerminalStory::use_theme_dirs([dir.clone()], cx));
    let (handle, gallery) = setup_gallery(cx);
    let story = story(&gallery, cx);
    let trigger = named(ElementId::Name("terminal-theme".into()), "trigger");
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(trigger, cx);
        window.render_frame(cx);
        window.input("drac", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let highlighted = story.read_with(cx, |story, cx| {
        story
            .themes()
            .read(cx)
            .highlighted()
            .map(|item| item.value().to_string())
    });
    assert_eq!(highlighted.as_deref(), Some("Dracula"), "the fuzzy match");
    press(handle, cx, &["enter"]);
    let background = story.read_with(cx, |story, _| story.colors().background());
    assert_eq!(background, Rgb(0x28, 0x2a, 0x36));
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn the_pane_menu_copies_the_selection_and_splits_the_pane(cx: &mut TestAppContext) {
    let (handle, _) = setup_gallery(cx);
    let menu = named(pane(0), "menu");
    let bounds = cx
        .update_window(handle.into(), |_, window, _| window.find(pane(0)).bounds())
        .unwrap();
    // The first row starts with the prompt "~/gpui-cn".
    let row = bounds.origin.y + px(10.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(pane(0), cx);
        window.drag(
            gpui_kit::point(bounds.origin.x + px(1.), row),
            gpui_kit::point(bounds.origin.x + px(200.), row),
            cx,
        );
        window.right_click(pane(0), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        window.click(named(menu.clone(), "copy"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert!(
        copied
            .as_deref()
            .is_some_and(|text| text.starts_with("~/gpui-cn")),
        "copied {copied:?}"
    );

    cx.update_window(handle.into(), |_, window, cx| {
        window.right_click(pane(0), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        window.click(named(menu, "split-left"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    let (left, right) = cx
        .update_window(handle.into(), |_, window, _| {
            (window.find(pane(1)).bounds(), window.find(pane(0)).bounds())
        })
        .unwrap();
    assert!(left.right() <= right.left(), "the new pane is on the left");
}

#[gpui_kit::test]
fn the_tab_menu_closes_other_tabs_and_renames(cx: &mut TestAppContext) {
    let (handle, gallery) = setup_gallery(cx);
    let story = story(&gallery, cx);
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "c"]);
    press(handle, cx, &["ctrl-a", "c"]);
    assert!(shows(handle, cx, tab("shell-3")));
    let menu = named(ElementId::Name("terminal-tabs".into()), "menu");
    cx.update_window(handle.into(), |_, window, cx| {
        window.right_click(tab("shell-2"), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        window.click(named(menu.clone(), "close-tabs-right"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        !shows(handle, cx, tab("shell-3")),
        "the tab to the right closes"
    );
    assert!(shows(handle, cx, tab("shell-1")));

    cx.update_window(handle.into(), |_, window, cx| {
        window.right_click(tab("shell-2"), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        window.click(named(menu, "close-other-tabs"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!shows(handle, cx, tab("shell-1")), "the others close");
    assert_eq!(label(&story, "shell-2", cx), "Shell 2");

    // A double click renames the tab in place.
    cx.update_window(handle.into(), |_, window, cx| {
        window.double_click(tab("shell-2"), cx);
        window.render_frame(cx);
        window.input("Build", cx);
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(label(&story, "shell-2", cx), "Build");
}

#[gpui_kit::test]
fn a_tab_follows_its_pane_title_until_the_user_names_it(cx: &mut TestAppContext) {
    let (handle, gallery) = setup_gallery(cx);
    let story = story(&gallery, cx);
    let set_title = |title: &str, cx: &mut TestAppContext| {
        let title = title.to_owned();
        let terminal = story.read_with(cx, |story, _| story.terminals()[0].clone());
        cx.update(|cx| {
            terminal.update(cx, |terminal, cx| {
                let mut snapshot = TerminalSnapshot::for_frame(terminal.frame().clone());
                snapshot.title = title;
                terminal.apply(&snapshot, cx);
            })
        });
        cx.run_until_parked();
    };
    set_title("vim Cargo.lock", cx);
    assert_eq!(label(&story, "shell-1", cx), "vim Cargo.lock");
    cx.update(|cx| {
        story.update(cx, |story, cx| {
            story
                .tabs()
                .update(cx, |tabs, cx| tabs.rename("shell-1", "Work", cx))
        })
    });
    set_title("htop", cx);
    assert_eq!(
        label(&story, "shell-1", cx),
        "Work",
        "the user's name stays"
    );
    let _ = handle;
}

#[gpui_kit::test]
fn the_leader_and_the_menu_zoom_a_pane_and_a_split_unzooms_it(cx: &mut TestAppContext) {
    let (handle, gallery) = setup_gallery(cx);
    let story = story(&gallery, cx);
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "%"]);
    assert!(shows(handle, cx, pane(0)) && shows(handle, cx, pane(1)));

    // The leader zooms the focused pane, the new one, over the whole tab.
    press(handle, cx, &["ctrl-a", "z"]);
    assert!(shows(handle, cx, pane(1)));
    assert!(!shows(handle, cx, pane(0)), "the other pane is hidden");
    let hidden = story.read_with(cx, |story, cx| {
        story
            .terminals()
            .iter()
            .filter(|terminal| !terminal.read(cx).is_visible())
            .count()
    });
    assert_eq!(hidden, 1, "the hidden pane stops painting");
    press(handle, cx, &["ctrl-a", "z"]);
    assert!(
        shows(handle, cx, pane(0)) && shows(handle, cx, pane(1)),
        "unzoomed"
    );

    // The pane menu zooms and unzooms too.
    let menu = named(pane(1), "menu");
    for _ in 0..2 {
        cx.update_window(handle.into(), |_, window, cx| {
            window.right_click(pane(1), cx);
            window.render_frame(cx);
            window.render_frame(cx);
            window.click(named(menu.clone(), "zoom-pane"), cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    assert!(shows(handle, cx, pane(0)), "zoomed and back");

    // A split while zoomed unzooms first, as tmux does.
    press(handle, cx, &["ctrl-a", "z"]);
    assert!(!shows(handle, cx, pane(0)));
    press(handle, cx, &["ctrl-a", "-"]);
    assert!(
        shows(handle, cx, pane(0)) && shows(handle, cx, pane(1)) && shows(handle, cx, pane(2)),
        "the split put the layout back"
    );
}

/// Reports `name` as the foreground process of every pane, as a local pty
/// would once a program took over, or the shell with `shell`.
fn run_in_panes(story: &Entity<TerminalStory>, name: &str, shell: bool, cx: &mut TestAppContext) {
    let terminals = story.read_with(cx, |story, _| story.terminals());
    cx.update(|cx| {
        for terminal in terminals {
            terminal.update(cx, |state, cx| {
                let mut snapshot = TerminalSnapshot::for_frame(state.frame().clone());
                snapshot.foreground =
                    Some(gpui_cn::terminal::ForegroundProcess::new(42, name).with_shell(shell));
                state.apply(&snapshot, cx);
            });
        }
    });
}

fn click(handle: WindowHandle<Root>, cx: &mut TestAppContext, id: &str) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(ElementId::Name(id.to_owned().into()), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn asking(handle: WindowHandle<Root>, cx: &mut TestAppContext) -> bool {
    shows(handle, cx, ElementId::Name("terminal-close-confirm".into()))
}

#[gpui_kit::test]
fn closing_a_pane_that_runs_a_program_asks_first(cx: &mut TestAppContext) {
    let (handle, gallery) = setup_gallery(cx);
    let story = story(&gallery, cx);
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "%"]);
    assert!(shows(handle, cx, pane(1)));

    // The shell at its prompt closes at once.
    run_in_panes(&story, "zsh", true, cx);
    press(handle, cx, &["ctrl-a", "x"]);
    assert!(!asking(handle, cx));
    assert!(!shows(handle, cx, pane(1)), "closed without asking");

    // A program in the foreground asks, and Cancel keeps the pane.
    run_in_panes(&story, "vim", false, cx);
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "x"]);
    assert!(asking(handle, cx), "asks before ending vim");
    click(handle, cx, "terminal-close-cancel");
    assert!(!asking(handle, cx));
    assert!(shows(handle, cx, pane(0)), "kept");

    // Close ends it.
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "x"]);
    click(handle, cx, "terminal-close-confirm");
    assert!(!asking(handle, cx));
    assert!(!shows(handle, cx, pane(0)), "closed");
}

#[gpui_kit::test]
fn closing_a_tab_that_runs_a_program_asks_first(cx: &mut TestAppContext) {
    let (handle, gallery) = setup_gallery(cx);
    let story = story(&gallery, cx);
    cx.update_window(handle.into(), |_, window, cx| window.click(pane(0), cx))
        .unwrap();
    press(handle, cx, &["ctrl-a", "c"]);
    assert!(shows(handle, cx, tab("shell-2")));
    run_in_panes(&story, "cargo", false, cx);

    let close = named(tab("shell-2"), "close");
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(close, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(asking(handle, cx), "asks before ending cargo");
    assert!(
        shows(handle, cx, tab("shell-2")),
        "the tab stays while asking"
    );
    click(handle, cx, "terminal-close-confirm");
    assert!(!shows(handle, cx, tab("shell-2")), "closed");
    assert!(shows(handle, cx, tab("shell-1")));
}
