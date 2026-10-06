//! The File picker story page: the trigger opens the dialog, a file can
//! be picked, and the page reports it.

use gpui_cn::{ReduceMotion, Theme};
use gpui_cn_story::{Gallery, stories::FilePickerStory};
use gpui_kit::{
    AppContext as _, ElementId, Entity, TestAppContext, WindowHandle, px, size,
    test::TestWindowExt as _,
};

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

fn entry(name: &str) -> ElementId {
    ElementId::NamedChild(
        ElementId::NamedChild(
            ElementId::Name("file-picker-one".into()).into(),
            "entry".into(),
        )
        .into(),
        name.to_string().into(),
    )
}

#[gpui_kit::test]
fn the_story_opens_the_dialog_and_reports_a_chosen_file(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery: Option<Entity<Gallery>> = None;
    let handle = cx.open_window(size(px(1300.), px(1400.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    let gallery = gallery.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        gallery.update(cx, |gallery, cx| {
            gallery.select_story("File picker", window, cx)
        });
    })
    .unwrap();
    settle(&handle, cx);
    let story = gallery
        .read_with(cx, |gallery, cx| {
            gallery.current_story::<FilePickerStory>(cx)
        })
        .expect("the story is showing");
    assert_eq!(
        story.read_with(cx, |s, _| s.chosen().to_string()),
        "No file chosen."
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(FilePickerStory::TRIGGER_ONE, cx);
    })
    .unwrap();
    settle(&handle, cx);
    assert!(story.read_with(cx, |s, _| s.one_open()));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(entry("main.rs"), cx);
        window.click(
            ElementId::NamedChild(
                ElementId::Name("file-picker-one".into()).into(),
                "open".into(),
            ),
            cx,
        );
    })
    .unwrap();
    settle(&handle, cx);
    assert!(
        !story.read_with(cx, |s, _| s.one_open()),
        "choosing closes it"
    );
    let chosen = story.read_with(cx, |s, _| s.chosen().to_string());
    assert!(
        chosen.starts_with("Chose ") && chosen.ends_with("main.rs."),
        "{chosen}"
    );
}
