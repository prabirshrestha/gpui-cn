//! UI integration tests for `ProgressRing`.

use gpui_cn::{ProgressRing, ReduceMotion, Theme};
use gpui_kit::base::Root;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, div, px, size, test::TestWindowExt as _,
};

struct Harness {
    value: f32,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            ProgressRing::new("meter")
                .value(self.value)
                .accessibility_label("Context"),
        )
    }
}

fn setup(
    cx: &mut TestAppContext,
    motion: ReduceMotion,
) -> (gpui_kit::WindowHandle<Root>, Entity<Harness>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = motion);
    });
    let mut view = None;
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let harness = cx.new(|_| Harness { value: 57. });
        view = Some(harness.clone());
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, view.unwrap())
}

#[gpui_kit::test]
fn the_ring_is_sixteen_pixels_and_is_idle(cx: &mut TestAppContext) {
    let (handle, _) = setup(cx, ReduceMotion::On);
    cx.update_window(handle.into(), |_, window, cx| {
        let ring = window.find("meter");
        assert_eq!(ring.bounds().size, size(px(16.), px(16.)));
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "an idle ring asks for no frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_new_value_moves_the_arc_over_frames_unless_motion_is_reduced(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, ReduceMotion::Off);
    view.update(cx, |harness, cx| {
        harness.value = 90.;
        cx.notify();
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.simulate_next_frame(cx) > 0,
            "the arc is still moving"
        );
    })
    .unwrap();

    let (handle, view) = setup(cx, ReduceMotion::On);
    view.update(cx, |harness, cx| {
        harness.value = 90.;
        cx.notify();
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.simulate_next_frame(cx), 0, "the arc lands at once");
    })
    .unwrap();
}
