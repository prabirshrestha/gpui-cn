//! UI integration tests for `Progress`.

use gpui_cn::{Progress, ReduceMotion, Root, Theme};
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    Window, div, px, size, test::TestWindowExt as _,
};

/// A 200px column of bars, one per value.
struct Harness {
    values: Vec<(&'static str, Option<f32>)>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            div().w(px(200.)).flex().flex_col().gap_4().children(
                self.values
                    .iter()
                    .map(|(id, value)| Progress::new(*id).value(*value)),
            ),
        )
    }
}

fn setup(
    cx: &mut TestAppContext,
    motion: ReduceMotion,
    values: Vec<(&'static str, Option<f32>)>,
) -> gpui_kit::WindowHandle<Root> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = motion);
    });
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let harness = cx.new(|_| Harness { values });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    handle
}

fn indicator(id: &'static str) -> gpui_kit::ElementId {
    gpui_kit::ElementId::NamedChild(gpui_kit::ElementId::from(id).into(), "indicator".into())
}

#[gpui_kit::test]
fn the_indicator_covers_the_value_of_the_track(cx: &mut TestAppContext) {
    let handle = setup(
        cx,
        ReduceMotion::On,
        vec![
            ("forty", Some(40.)),
            ("over", Some(150.)),
            ("zero", Some(0.)),
        ],
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let track = window.find("forty").bounds();
        assert_eq!(track.size.width, px(200.));
        assert_eq!(track.size.height, px(8.), "h-2 at the default font size");
        let forty = window.find(indicator("forty")).bounds();
        assert_eq!(forty.size.width, px(80.));
        assert_eq!(forty.size.height, px(8.));
        assert_eq!(forty.origin.x, track.origin.x);
        assert_eq!(window.find(indicator("over")).bounds().size.width, px(200.));
        assert_eq!(window.find(indicator("zero")).bounds().size.width, px(0.));
    })
    .unwrap();
}

/// One bar whose value and height the test changes.
struct Moving {
    value: Option<f32>,
}

impl Render for Moving {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            div()
                .w(px(200.))
                .child(Progress::new("bar").value(self.value).h(px(12.))),
        )
    }
}

#[gpui_kit::test]
fn a_new_value_slides_the_indicator_and_the_height_follows_the_root(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let mut view = None;
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let moving = cx.new(|_| Moving { value: Some(0.) });
        view = Some(moving.clone());
        Root::new(moving, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("bar").bounds().size.height, px(12.));
        let indicator = window.find(indicator("bar")).bounds();
        assert_eq!(indicator.size.width, px(0.));
        assert_eq!(indicator.size.height, px(12.));
    })
    .unwrap();
    view.update(cx, |view, cx| {
        view.value = Some(50.);
        cx.notify();
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.simulate_next_frame(cx) > 0,
            "the indicator is on its way"
        );
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find(indicator("bar")).bounds().size.width, px(100.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn an_indeterminate_bar_sweeps(cx: &mut TestAppContext) {
    let handle = setup(cx, ReduceMotion::Off, vec![("busy", None)]);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.simulate_next_frame(cx) > 0,
            "the sweep asks for the next frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn with_reduce_motion_an_indeterminate_bar_holds_still_in_the_middle(cx: &mut TestAppContext) {
    let handle = setup(cx, ReduceMotion::On, vec![("busy", None)]);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.simulate_next_frame(cx), 0, "no sweep, no frame");
        let track = window.find("busy").bounds();
        let segment = window.find(indicator("busy")).bounds();
        assert_eq!(segment.size.width, px(70.));
        assert_eq!(segment.origin.x - track.origin.x, px(65.));
    })
    .unwrap();
}
