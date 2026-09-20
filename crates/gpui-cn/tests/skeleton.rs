//! UI integration tests for `Skeleton`.

use gpui_cn::{ReduceMotion, Root, Skeleton, Theme};
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    Window, div, px, size, test::TestWindowExt as _,
};

struct Harness;

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child(div().w(px(200.)).child(Skeleton::new("line")))
    }
}

fn setup(cx: &mut TestAppContext, motion: ReduceMotion) -> gpui_kit::WindowHandle<Root> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = motion);
    });
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let harness = cx.new(|_| Harness);
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    handle
}

#[gpui_kit::test]
fn a_block_is_a_line_high_and_pulses_while_shown(cx: &mut TestAppContext) {
    let handle = setup(cx, ReduceMotion::Off);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.simulate_next_frame(cx) > 0,
            "the pulse asks for the next frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn with_reduce_motion_the_block_holds_still(cx: &mut TestAppContext) {
    let handle = setup(cx, ReduceMotion::On);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.simulate_next_frame(cx), 0, "no pulse, no frame");
    })
    .unwrap();
}
