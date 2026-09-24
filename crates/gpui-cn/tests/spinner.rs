//! UI integration tests for `Spinner`.

use gpui_cn::{ReduceMotion, Root, Spinner, Theme};
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
            .flex()
            .gap_2()
            .child(Spinner::new("default"))
            .child(
                Spinner::new("large")
                    .size_8()
                    .accessibility_label("Syncing"),
            )
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
fn a_spinner_is_a_status_a_rem_square_and_turns(cx: &mut TestAppContext) {
    let handle = setup(cx, ReduceMotion::Off);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let spinner = window.find("default");
        assert_eq!(spinner.bounds().size.width, px(16.));
        assert_eq!(spinner.bounds().size.height, px(16.));
        assert_eq!(spinner.role(), Some(gpui_kit::Role::Status));
        assert_eq!(spinner.label(), Some("Loading"));
        let large = window.find("large");
        assert_eq!(large.bounds().size.width, px(32.));
        assert_eq!(large.label(), Some("Syncing"));
        assert!(
            window.simulate_next_frame(cx) > 0,
            "the turn asks for the next frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn with_reduce_motion_a_spinner_holds_still(cx: &mut TestAppContext) {
    let handle = setup(cx, ReduceMotion::On);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.simulate_next_frame(cx), 0, "no turn, no frame");
    })
    .unwrap();
}
