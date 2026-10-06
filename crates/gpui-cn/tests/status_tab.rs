//! UI integration tests for `ComposerStatusTab` and `ContextMeter`.

use gpui_cn::{ComposerStatusTab, ReduceMotion, Theme};
use gpui_kit::base::Root;
use gpui_kit::{
    AppContext as _, Context, ElementId, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, div, px, size, test::TestWindowExt as _,
};

struct Harness {
    percent: f32,
    bare: bool,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(500.)).child(if self.bare {
            ComposerStatusTab::new("status")
        } else {
            ComposerStatusTab::new("status")
                .branch("Main")
                .folder("project-sea")
                .context(self.percent)
        })
    }
}

fn setup(cx: &mut TestAppContext, bare: bool) -> gpui_kit::WindowHandle<Root> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let handle = cx.open_window(size(px(600.), px(300.)), |window, cx| {
        let harness = cx.new(|_| Harness { percent: 57., bare });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    handle
}

fn meter() -> ElementId {
    ElementId::NamedChild(ElementId::from("status").into(), "context".into())
}

#[gpui_kit::test]
fn the_tab_is_thirty_four_high_and_inset_from_both_sides(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    cx.update_window(handle.into(), |_, window, cx| {
        let tab = window.find("status").bounds();
        assert_eq!(tab.size.height, px(34.));
        assert_eq!(tab.origin.x, px(28.), "inset from the left");
        assert_eq!(tab.size.width, px(500. - 56.), "inset from both sides");
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "an idle tab asks for no frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_meter_sits_at_the_far_right_and_holds_a_sixteen_pixel_ring(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    cx.update_window(handle.into(), |_, window, _| {
        let tab = window.find("status").bounds();
        let bounds = window.find(meter()).bounds();
        assert_eq!(
            bounds.right(),
            tab.right() - px(12.),
            "the tab's side padding"
        );
        let ring = window
            .find(ElementId::NamedChild(meter().into(), "ring".into()))
            .bounds();
        assert_eq!(ring.size, size(px(16.), px(16.)));
        assert!(ring.left() >= bounds.left());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_tab_without_items_is_still_a_strip(cx: &mut TestAppContext) {
    let handle = setup(cx, true);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("status").bounds().size.height, px(34.));
        assert!(window.try_find(meter()).is_none(), "no meter was given");
    })
    .unwrap();
}
