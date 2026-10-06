//! UI integration tests for `ModelPicker`: real input through a headless
//! window.

use std::{cell::RefCell, rc::Rc};

use gpui_cn::{
    ModelEntry, ModelPicker, ModelPickerEvent, ModelPickerState, ModelProvider, ReduceMotion, Theme,
};
use gpui_kit::base::Root;
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TestAppContext, Window, assets::IconName, div, point, px, size,
    test::TestWindowExt as _,
};

struct Harness {
    state: Entity<ModelPickerState>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .flex()
            .items_start()
            .justify_end()
            .child(ModelPicker::new("model", &self.state))
    }
}

fn catalog() -> Vec<ModelProvider> {
    vec![
        ModelProvider::new("acme", "Acme")
            .icon(IconName::Bot)
            .models([
                ModelEntry::new("acme-fast", "Alpha Fast"),
                ModelEntry::new("acme-deep", "Alpha Deep").effort(true),
            ]),
        ModelProvider::new("zed", "Zed Labs")
            .icon(IconName::Cpu)
            .models([
                ModelEntry::new("zed-one", "Beta One"),
                ModelEntry::new("zed-alpha", "Alpha Z"),
            ]),
        ModelProvider::new("moon", "Moon")
            .icon(IconName::Moon)
            .models([ModelEntry::new("moon-1", "Gamma")]),
    ]
}

struct Setup {
    handle: gpui_kit::WindowHandle<Root>,
    state: Entity<ModelPickerState>,
    events: Rc<RefCell<Vec<ModelPickerEvent>>>,
}

fn setup(cx: &mut TestAppContext) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut slot = None;
    let handle = cx.open_window(size(px(800.), px(700.)), |window, cx| {
        let state = cx.new(|cx| ModelPickerState::new(catalog(), window, cx));
        let recorded = events.clone();
        cx.subscribe(&state, move |_, _, event: &ModelPickerEvent, _| {
            recorded.borrow_mut().push(event.clone());
        })
        .detach();
        slot = Some(state.clone());
        let harness = cx.new(|cx| {
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            Harness { state }
        });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    Setup {
        handle,
        state: slot.unwrap(),
        events,
    }
}

fn named(parent: ElementId, name: impl Into<SharedString>) -> ElementId {
    ElementId::NamedChild(parent.into(), name.into())
}

fn id() -> ElementId {
    ElementId::from("model")
}

fn model(name: &'static str) -> ElementId {
    named(id(), name)
}

fn provider(name: &'static str) -> ElementId {
    named(named(id(), "provider"), name)
}

fn open(setup: &Setup, cx: &mut TestAppContext) {
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(named(id(), "trigger"), cx);
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(setup.handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn frames(setup: &Setup, cx: &mut TestAppContext) {
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
}

fn rows(setup: &Setup, cx: &mut TestAppContext) -> Vec<String> {
    setup.state.read_with(cx, |state, cx| {
        state
            .visible(cx)
            .into_iter()
            .map(|(_, model)| model.id().to_string())
            .collect()
    })
}

#[gpui_kit::test]
fn the_panel_is_the_spec_width_and_lists_the_first_providers_models(cx: &mut TestAppContext) {
    let setup = setup(cx);
    cx.update_window(setup.handle.into(), |_, window, _| {
        assert!(window.try_find(named(id(), "panel")).is_none(), "closed");
    })
    .unwrap();
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, _| {
        let panel = window.find(named(id(), "panel")).bounds();
        assert_eq!(panel.size.width, px(341.));
        assert!(window.try_find(model("acme-fast")).is_some());
        assert!(
            window.try_find(model("zed-one")).is_none(),
            "another provider"
        );
        assert_eq!(
            window.find(model("acme-fast")).bounds().size.height,
            px(40.)
        );
        for name in ["acme", "zed", "moon"] {
            assert!(window.try_find(provider(name)).is_some(), "{name} mark");
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_provider_mark_swaps_the_models(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(provider("zed"), cx);
        window.render_frame(cx);
        assert!(window.try_find(model("zed-one")).is_some());
        assert!(window.try_find(model("acme-fast")).is_none());
    })
    .unwrap();
    assert_eq!(
        setup.events.borrow().as_slice(),
        [
            ModelPickerEvent::OpenChanged(true),
            ModelPickerEvent::ProviderChanged("zed".into())
        ]
    );
}

#[gpui_kit::test]
fn choosing_a_model_selects_it_and_closes_the_panel(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(model("acme-fast"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    frames(&setup, cx);
    assert_eq!(
        setup
            .state
            .read_with(cx, |state, _| state.selected().cloned()),
        Some("acme-fast".into())
    );
    assert!(
        setup
            .events
            .borrow()
            .contains(&ModelPickerEvent::Selected("acme-fast".into()))
    );
    cx.update_window(setup.handle.into(), |_, window, _| {
        assert!(window.try_find(named(id(), "panel")).is_none(), "closed");
    })
    .unwrap();
    assert!(!setup.state.read_with(cx, |state, _| state.is_open()));
}

#[gpui_kit::test]
fn quick_search_filters_across_every_provider(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert_ne!(
            window.find(named(id(), "search")).focused(),
            Some(true),
            "the search field is not boxed until it is focused"
        );
        window.press("/", cx);
        window.render_frame(cx);
        window.input("alpha", cx);
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find(model("acme-fast")).is_some());
        assert!(window.try_find(model("acme-deep")).is_some());
        assert!(
            window.try_find(model("zed-alpha")).is_some(),
            "from another provider"
        );
        assert!(window.try_find(model("zed-one")).is_none());
        assert!(window.try_find(model("moon-1")).is_none());
    })
    .unwrap();
    assert_eq!(rows(&setup, cx), ["acme-fast", "acme-deep", "zed-alpha"]);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("cmd-a", cx);
        window.input("nothing like it", cx);
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find(model("acme-fast")).is_none());
    })
    .unwrap();
    assert!(rows(&setup, cx).is_empty(), "an empty state shows");
}

#[gpui_kit::test]
fn a_slash_moves_focus_to_the_search_field(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert_ne!(window.find(named(id(), "search")).focused(), Some(true));
        window.press("/", cx);
        window.render_frame(cx);
        assert_eq!(window.find(named(id(), "search")).focused(), Some(true));
    })
    .unwrap();
    assert_eq!(
        setup.state.read_with(cx, |state, cx| state.query(cx)),
        "",
        "the slash is not typed"
    );
}

#[gpui_kit::test]
fn the_chosen_model_shows_an_effort_chip_that_opens_a_slider(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(model("acme-deep"), cx);
    })
    .unwrap();
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert!(window.try_find(named(id(), "effort")).is_some(), "the chip");
        assert!(window.try_find(model("acme-fast")).is_some_and(|_| {
            window
                .try_find(named(named(id(), "acme-fast"), "effort"))
                .is_none()
        }));
        window.click(named(id(), "effort"), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find(named(id(), "effort-card")).is_some());
        assert!(
            window.find(named(id(), "effort-card")).bounds().top()
                >= window.find(model("acme-deep")).bounds().bottom(),
            "the card opens under the chosen row"
        );
        let slider = window.find(named(id(), "effort-slider")).bounds();
        window.click_at(
            named(id(), "effort-slider"),
            point(px(1.), slider.size.height / 2.),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        setup.state.read_with(cx, |state, _| state.effort()),
        Some(0)
    );
    assert_eq!(
        setup.state.read_with(cx, |state, _| state.effort_label()),
        Some("Low")
    );
    assert!(
        setup
            .events
            .borrow()
            .contains(&ModelPickerEvent::EffortChanged(0))
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let slider = window.find(named(id(), "effort-slider")).bounds();
        window.click_at(
            named(id(), "effort-slider"),
            point(slider.size.width - px(1.), slider.size.height / 2.),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        setup.state.read_with(cx, |state, _| state.effort()),
        Some(5)
    );
}

#[gpui_kit::test]
fn escape_closes_the_effort_card_first_and_the_panel_second(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(model("acme-deep"), cx);
    })
    .unwrap();
    open(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(named(id(), "effort"), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(window.try_find(named(id(), "effort-card")).is_none());
        assert!(
            window.try_find(named(id(), "panel")).is_some(),
            "the panel stays"
        );
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(window.try_find(named(id(), "panel")).is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_model_without_effort_reports_none(cx: &mut TestAppContext) {
    let setup = setup(cx);
    setup.state.update(cx, |state, cx| {
        state.select("acme-fast", cx);
    });
    assert_eq!(setup.state.read_with(cx, |state, _| state.effort()), None);
    setup.state.update(cx, |state, cx| {
        state.select("acme-deep", cx);
    });
    assert_eq!(
        setup.state.read_with(cx, |state, _| state.effort()),
        Some(1)
    );
}

fn panel_height(setup: &Setup, cx: &mut TestAppContext) -> gpui_kit::Pixels {
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.find(named(id(), "panel")).bounds().size.height
    })
    .unwrap()
}

#[gpui_kit::test]
fn the_panel_keeps_one_height_in_every_state(cx: &mut TestAppContext) {
    let setup = setup(cx);
    open(&setup, cx);
    let height = panel_height(&setup, cx);
    assert_eq!(
        height,
        px(34. + 240. + 8. + 2.),
        "header, six rows, padding, hairline"
    );
    let same = |setup: &Setup, cx: &mut TestAppContext, state: &str| {
        assert_eq!(panel_height(setup, cx), height, "{state}");
    };
    same(&setup, cx, "two models of the first provider");

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(provider("moon"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    same(&setup, cx, "one model of another provider");

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("/", cx);
        window.input("alpha", cx);
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    same(&setup, cx, "three matches");

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("cmd-a", cx);
        window.input("gamma", cx);
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    same(&setup, cx, "one match");

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("cmd-a", cx);
        window.input("nothing like it", cx);
        window.render_frame(cx);
        window.render_frame(cx);
        let list = window.find(named(id(), "list")).bounds();
        let empty = window.find(named(id(), "empty")).bounds();
        assert_eq!(empty, list, "the message fills the list box");
    })
    .unwrap();
    same(&setup, cx, "no match");

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        window.render_frame(cx);
        window.click(provider("acme"), cx);
        window.render_frame(cx);
        window.click(model("acme-deep"), cx);
    })
    .unwrap();
    open(&setup, cx);
    same(&setup, cx, "a chosen model with an effort chip");
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(named(id(), "effort"), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find(named(id(), "effort-card")).is_some());
    })
    .unwrap();
    same(&setup, cx, "the effort card open");
}
