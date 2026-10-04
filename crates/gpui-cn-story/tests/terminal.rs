//! The Terminal story's tabs, splits and leader keys, with fixture panes.

#![cfg(feature = "terminal")]

use std::sync::Arc;

use gpui_cn::terminal::{TerminalColors, frame::Rgb};
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
