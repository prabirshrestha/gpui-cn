//! The Progress page: the live example fills, stops by itself, and
//! restarts on Reset.

use std::time::Duration;

use gpui_cn::{ReduceMotion, Theme};
use gpui_cn_story::{Gallery, stories::ProgressStory};
use gpui_kit::{AppContext as _, Entity, TestAppContext, px, size, test::TestWindowExt as _};

#[gpui_kit::test]
fn the_live_example_fills_stops_and_restarts(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery = None;
    let handle = cx.open_window(size(px(1300.), px(2200.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    let gallery: Entity<Gallery> = gallery.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        gallery.update(cx, |gallery, cx| {
            gallery.select_story("Progress", window, cx)
        });
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    let story = gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<ProgressStory>(cx))
        .expect("the progress story is showing");
    assert!(story.read_with(cx, |story, _| story.is_live()));
    for _ in 0..80 {
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
    }
    assert_eq!(story.read_with(cx, |story, _| story.live()), 100.);
    assert!(
        !story.read_with(cx, |story, _| story.is_live()),
        "it stopped"
    );

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("live-reset", cx);
    })
    .unwrap();
    assert!(
        story.read_with(cx, |story, _| story.is_live()),
        "Reset restarts it"
    );
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    let live = story.read_with(cx, |story, _| story.live());
    assert!(live > 0. && live < 100., "{live}");
}
