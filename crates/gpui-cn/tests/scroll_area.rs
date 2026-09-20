//! UI integration tests for `ScrollArea`.

use gpui_cn::{Root, ScrollArea};
use gpui_kit::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    PlatformInput, Render, ScrollDelta, ScrollWheelEvent, Styled as _, TestAppContext, TouchPhase,
    Window, base::TestSupportExt as _, div, point, px, size, test::TestWindowExt as _,
};

/// A 200px viewport over forty 30px rows.
struct Harness;

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h(px(200.))
            .child(
                ScrollArea::new("rows")
                    .size_full()
                    .children((0..40usize).map(|n| {
                        div()
                            .id(("row", n))
                            .test_support()
                            .h(px(30.))
                            .child(format!("Row {n}"))
                    })),
            )
    }
}

#[gpui_kit::test]
fn the_region_scrolls_its_rows_and_keeps_its_position_across_frames(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let handle = cx.open_window(size(px(400.), px(400.)), |window, cx| {
        let harness = cx.new(|_| Harness);
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("rows").bounds().size.height, px(200.));
        assert_eq!(window.find(("row", 0usize)).bounds().origin.y, px(0.));
        window.scroll("rows", ScrollDelta::Pixels(point(px(0.), px(-90.))), cx);
        window.render_frame(cx);
        assert_eq!(window.find(("row", 0usize)).bounds().origin.y, px(-90.));
        // The position lives in window state, not in the element, so a
        // fresh render of the view keeps it.
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find(("row", 0usize)).bounds().origin.y, px(-90.));
    })
    .unwrap();
}

/// A page that scrolls, holding a region that scrolls (forty rows in a
/// 120px box) and a region whose content fits (three rows in a 200px box).
struct Nested;

impl Render for Nested {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h(px(300.)).child(
            ScrollArea::new("page")
                .size_full()
                .child(div().id("top").test_support().h(px(20.)))
                .child(
                    div()
                        .h(px(120.))
                        .child(
                            ScrollArea::new("inner")
                                .size_full()
                                .children((0..40usize).map(|n| {
                                    div()
                                        .id(("inner-row", n))
                                        .test_support()
                                        .h(px(30.))
                                        .child(format!("Inner {n}"))
                                })),
                        ),
                )
                .child(
                    div()
                        .h(px(200.))
                        .child(
                            ScrollArea::new("fits")
                                .size_full()
                                .children((0..3usize).map(|n| {
                                    div()
                                        .id(("fits-row", n))
                                        .test_support()
                                        .h(px(30.))
                                        .child(format!("Fits {n}"))
                                })),
                        ),
                )
                .child(div().h(px(600.))),
        )
    }
}

#[gpui_kit::test]
fn a_region_with_room_to_scroll_keeps_the_wheel_and_one_that_fits_passes_it_on(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let handle = cx.open_window(size(px(400.), px(400.)), |window, cx| {
        let harness = cx.new(|_| Nested);
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        let top = window.find("top").bounds().origin.y;
        // A wheel step over the inner region scrolls it and not the page.
        window.scroll(
            ("inner-row", 1usize),
            ScrollDelta::Pixels(point(px(0.), px(-40.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(
            window.find(("inner-row", 0usize)).bounds().origin.y,
            px(-40.) + px(20.)
        );
        assert_eq!(window.find("top").bounds().origin.y, top, "the page stayed");
        // A wheel step over the region whose content fits scrolls the page.
        window.scroll(
            ("fits-row", 1usize),
            ScrollDelta::Pixels(point(px(0.), px(-40.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(window.find("top").bounds().origin.y, top - px(40.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_step_past_the_top_stretches_the_region(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let handle = cx.open_window(size(px(400.), px(400.)), |window, cx| {
        let harness = cx.new(|_| Harness);
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find(("row", 0usize)).bounds().origin.y, px(0.));
        // A finger pulling down at the top: the rows follow it past the
        // edge instead of staying put.
        window.dispatch_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position: window.find("rows").bounds().center(),
                delta: ScrollDelta::Pixels(point(px(0.), px(60.))),
                touch_phase: TouchPhase::Started,
                ..Default::default()
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position: window.find("rows").bounds().center(),
                delta: ScrollDelta::Pixels(point(px(0.), px(60.))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            }),
            cx,
        );
        window.render_frame(cx);
        let stretched = window.find(("row", 0usize)).bounds().origin.y;
        assert!(stretched > px(0.), "stretched past the top: {stretched:?}");
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_scrollbar_shows_after_a_wheel_step_and_drags_the_content(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};

    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let handle = cx.open_window(size(px(400.), px(400.)), |window, cx| {
        let harness = cx.new(|_| Harness);
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        window.scroll("rows", ScrollDelta::Pixels(point(px(0.), px(-90.))), cx);
        window.render_frame(cx);
        assert_eq!(window.find(("row", 0usize)).bounds().origin.y, px(-90.));
        let viewport = window.find("rows").bounds();
        let thumb = point(viewport.right() - px(4.), px(30.));
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: thumb,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: thumb + point(px(0.), px(50.)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::default(),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: thumb + point(px(0.), px(50.)),
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
            cx,
        );
        window.render_frame(cx);
        let row = window.find(("row", 0usize)).bounds().origin.y;
        assert!(row < px(-200.), "the thumb dragged the rows to {row:?}");
    })
    .unwrap();
}
