//! The command palettes of the gallery: the shell's Cmd+K palette that
//! lists every story, and the Command story's own dialog.

use gpui_cn::{ReduceMotion, Theme};
use gpui_cn_story::{
    Gallery, OpenCommandPalette,
    stories::{CommandStory, TypographyStory},
};
use gpui_kit::{
    AppContext as _, Entity, KeyBinding, TestAppContext, WindowHandle, px, size,
    test::TestWindowExt as _,
};

const OPEN: &str = if cfg!(target_os = "macos") {
    "cmd-k"
} else {
    "ctrl-k"
};

fn setup(cx: &mut TestAppContext) -> (WindowHandle<gpui_kit::base::Root>, Entity<Gallery>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
        cx.bind_keys([KeyBinding::new(OPEN, OpenCommandPalette, None)]);
    });
    let mut gallery = None;
    let handle = cx.open_window(size(px(1100.), px(760.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    let gallery = gallery.unwrap();
    settle(&handle, cx);
    (handle, gallery)
}

fn settle(handle: &WindowHandle<gpui_kit::base::Root>, cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.update_window((*handle).into(), |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_shell_shortcut_opens_the_story_palette_and_a_pick_navigates(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    assert!(!gallery.read_with(cx, |gallery, _| gallery.palette_open()));
    cx.update_window(handle.into(), |_, window, cx| window.press(OPEN, cx))
        .unwrap();
    settle(&handle, cx);
    assert!(gallery.read_with(cx, |gallery, _| gallery.palette_open()));
    let count = gallery.read_with(cx, |gallery, cx| gallery.palette().read(cx).matched_count());
    assert_eq!(
        count,
        gpui_cn_story::stories().len(),
        "every story is listed"
    );
    cx.update_window(handle.into(), |_, window, cx| window.input("typogr", cx))
        .unwrap();
    settle(&handle, cx);
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    settle(&handle, cx);
    settle(&handle, cx);
    assert!(!gallery.read_with(cx, |gallery, _| gallery.palette_open()));
    assert!(
        gallery
            .read_with(cx, |gallery, cx| gallery
                .current_story::<TypographyStory>(cx))
            .is_some(),
        "the chosen story is showing"
    );
}

#[gpui_kit::test]
fn the_command_story_opens_its_dialog_with_its_own_shortcut(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        gallery.update(cx, |gallery, cx| {
            gallery.select_story("Command", window, cx)
        });
    })
    .unwrap();
    settle(&handle, cx);
    let story = gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<CommandStory>(cx))
        .expect("the command story is showing");
    cx.update_window(handle.into(), |_, window, cx| {
        let focus = story.read(cx).focus().clone();
        focus.focus(window, cx);
    })
    .unwrap();
    settle(&handle, cx);
    cx.update_window(handle.into(), |_, window, cx| window.press(OPEN, cx))
        .unwrap();
    settle(&handle, cx);
    assert!(story.read_with(cx, |story, _| story.dialog_open()));
    assert!(
        !gallery.read_with(cx, |gallery, _| gallery.palette_open()),
        "the story's binding wins in its context"
    );
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    settle(&handle, cx);
    assert!(!story.read_with(cx, |story, _| story.dialog_open()));
}
