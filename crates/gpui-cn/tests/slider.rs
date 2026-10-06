//! UI integration tests for `Slider`: real input through a headless
//! window.

use std::{cell::RefCell, rc::Rc};

use gpui_cn::{Slider, SliderEvent, SliderState};
use gpui_kit::base::Root;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, base::Disableable as _, div, point, px, size, test::TestWindowExt as _,
};

struct Harness {
    state: Entity<SliderState>,
    disabled: bool,
    stops: Option<usize>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let slider = Slider::new(&self.state)
            .id("effort")
            .accessibility_label("Effort")
            .disabled(self.disabled);
        let slider = match self.stops {
            Some(count) => slider.stops(count),
            None => slider,
        };
        div()
            .size_full()
            .p_4()
            .child(div().w(px(200.)).child(slider))
    }
}

struct Setup {
    handle: gpui_kit::WindowHandle<Root>,
    state: Entity<SliderState>,
    events: Rc<RefCell<Vec<String>>>,
}

fn setup(cx: &mut TestAppContext, disabled: bool, stops: Option<usize>) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let slot = Rc::new(RefCell::new(None));
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let state = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(5.)
                .step(1.)
                .default_value(2.)
        });
        let log = events.clone();
        cx.subscribe(&state, move |_, _, event: &SliderEvent, _| {
            log.borrow_mut().push(match event {
                SliderEvent::Change(value) => format!("change {value}"),
                SliderEvent::Release(value) => format!("release {value}"),
            });
        })
        .detach();
        *slot.borrow_mut() = Some(state.clone());
        let harness = cx.new(|_| Harness {
            state,
            disabled,
            stops,
        });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    let state = slot.borrow_mut().take().unwrap();
    Setup {
        handle,
        state,
        events,
    }
}

fn value(setup: &Setup, cx: &mut TestAppContext) -> f32 {
    setup.state.read_with(cx, |state, _| state.value().end())
}

#[gpui_kit::test]
fn the_control_is_a_thumb_high_and_as_wide_as_its_parent(cx: &mut TestAppContext) {
    let setup = setup(cx, false, None);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let bounds = window.find("effort").bounds();
        assert_eq!(bounds.size.width, px(200.));
        assert_eq!(bounds.size.height, px(16.), "the thumb diameter");
        assert_eq!(window.find("effort").label(), Some("Effort"));
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "an idle slider asks for no frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_press_on_the_track_moves_the_thumb_to_the_nearest_step(cx: &mut TestAppContext) {
    let setup = setup(cx, false, Some(6));
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click_at("effort", point(px(190.), px(8.)), cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 5.);
    assert_eq!(
        setup.events.borrow().last().map(String::as_str),
        Some("release 5")
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click_at("effort", point(px(2.), px(8.)), cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 0.);
}

#[gpui_kit::test]
fn dragging_the_thumb_follows_the_pointer(cx: &mut TestAppContext) {
    let setup = setup(cx, false, None);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let bounds = window.find("effort").bounds();
        let from = point(bounds.left() + bounds.size.width * 0.4, bounds.center().y);
        let to = point(bounds.left() + bounds.size.width * 0.8, bounds.center().y);
        window.drag(from, to, cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 4.);
}

#[gpui_kit::test]
fn tab_reaches_the_slider_and_the_arrows_home_and_end_move_it(cx: &mut TestAppContext) {
    let setup = setup(cx, false, None);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("tab", cx);
        window.render_frame(cx);
        assert_eq!(window.find("effort").focused(), Some(true));
        window.press("right", cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 3.);
    assert_eq!(
        setup.events.borrow().as_slice(),
        ["change 3", "release 3"],
        "a key press reports like a click"
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("left", cx);
        window.press("left", cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 1.);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("end", cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 5.);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("right", cx);
        window.press("home", cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 0.);
}

#[gpui_kit::test]
fn a_disabled_slider_ignores_pointer_and_keys(cx: &mut TestAppContext) {
    let setup = setup(cx, true, None);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click_at("effort", point(px(190.), px(8.)), cx);
        window.press("tab", cx);
        window.render_frame(cx);
        assert_ne!(window.find("effort").focused(), Some(true));
        window.press("right", cx);
    })
    .unwrap();
    assert_eq!(value(&setup, cx), 2.);
    assert!(setup.events.borrow().is_empty());
}
