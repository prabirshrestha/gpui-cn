//! UI integration tests for `Input`: real input through a headless window.

use std::{cell::Cell, rc::Rc};

use gpui_cn::{Icon, Input, InputEvent, InputState, Root, Theme};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, TestAppContext, TestSupportExt as _, Window,
    base::Disableable as _, div, point, prelude::FluentBuilder as _, px, size,
    test::TestWindowExt as _,
};

struct Harness {
    state: Entity<InputState>,
    options: Options,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let options = &self.options;
        div().size_full().p_4().child(
            Input::new(&self.state)
                .id("name")
                .disabled(options.disabled)
                .readonly(options.readonly)
                .when(options.label, |this| this.accessibility_label("Name"))
                .when(options.affixes, |this| {
                    this.prefix(Icon::new("icons/search.svg"))
                        .suffix(div().id("suffix").test_support().size_4())
                })
                .cleanable(options.cleanable)
                .mask_toggle(options.masked),
        )
    }
}

struct Setup {
    handle: gpui_kit::WindowHandle<Root>,
    state: Entity<InputState>,
    enters: Rc<Cell<usize>>,
}

struct Options {
    disabled: bool,
    readonly: bool,
    affixes: bool,
    /// Whether the field gets an accessibility label of its own.
    label: bool,
    cleanable: bool,
    /// The state is masked and the field shows the toggle.
    masked: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            disabled: false,
            readonly: false,
            affixes: false,
            label: true,
            cleanable: false,
            masked: false,
        }
    }
}

fn setup(cx: &mut TestAppContext, options: Options) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let enters = Rc::new(Cell::new(0));
    let state = Rc::new(Cell::new(None));
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Your name")
                .masked(options.masked)
        });
        state.set(Some(input.clone()));
        let harness = cx.new(|cx| {
            let enters = enters.clone();
            cx.subscribe(&input, move |_, _, event: &InputEvent, _| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    enters.set(enters.get() + 1);
                }
            })
            .detach();
            Harness {
                state: input,
                options,
            }
        });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    Setup {
        handle,
        state: state.take().unwrap(),
        enters,
    }
}

#[gpui_kit::test]
fn the_frame_is_the_reference_field_height_and_carries_the_name(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let input = window.find("name");
        assert_eq!(input.label(), Some("Name"));
        assert_eq!(input.bounds().size.height, px(32.));
        assert_eq!(
            input.bounds().size.width,
            px(400. - 32.),
            "the frame fills its parent"
        );
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "an idle field asks for no frame"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_placeholder_names_the_field_when_no_label_is_given(cx: &mut TestAppContext) {
    let setup = setup(
        cx,
        Options {
            label: false,
            ..Options::default()
        },
    );
    cx.update_window(setup.handle.into(), |_, window, _| {
        assert_eq!(window.find("name").label(), Some("Your name"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_click_on_the_padding_puts_the_caret_in_the_text(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let padding = point(px(3.), px(16.));
        window.click_at("name", padding, cx);
        window.input("hi", cx);
        window.click_at("name", padding, cx);
        window.render_frame(cx);
        assert!(
            setup
                .state
                .read(cx)
                .presentation()
                .focus_handle()
                .is_focused(window),
            "a second click on the padding keeps the caret inside"
        );
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "hi");
}

#[gpui_kit::test]
fn a_click_focuses_the_field_and_typing_changes_the_value(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("name").focused(),
            Some(true),
            "the frame reports focus while the caret is inside"
        );
        window.input("hello", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("name").value(),
            Some("hello"),
            "assistive technology reads the value from the frame"
        );
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "hello");
}

#[gpui_kit::test]
fn select_all_takes_the_whole_value(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.input("Ada", cx);
        window.press("cmd-a", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        setup.state.read_with(cx, |state, _| state.selected_range()),
        0..3
    );
}

#[gpui_kit::test]
fn tab_reaches_the_field(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("tab", cx);
        window.render_frame(cx);
        assert_eq!(window.find("name").focused(), Some(true));
        window.input("a", cx);
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "a");
}

#[gpui_kit::test]
fn enter_reports_a_submit(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.input("go", cx);
        window.press("enter", cx);
    })
    .unwrap();
    assert_eq!(setup.enters.get(), 1);
    assert_eq!(
        setup.state.read_with(cx, |state, _| state.value()),
        "go",
        "a single line keeps Enter out of the text"
    );
}

#[gpui_kit::test]
fn a_disabled_field_ignores_input(cx: &mut TestAppContext) {
    let setup = setup(
        cx,
        Options {
            disabled: true,
            ..Options::default()
        },
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.render_frame(cx);
        assert_ne!(
            window.find("name").focused(),
            Some(true),
            "a click does not focus a disabled field"
        );
        assert!(!setup.state.read(cx).focus_handle(cx).is_focused(window));
        let mut key = gpui_kit::Keystroke::parse("x").unwrap();
        key.key_char = Some("x".into());
        window.dispatch_keystroke(key, cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "");
}

#[gpui_kit::test]
fn a_read_only_field_focuses_but_keeps_its_value(cx: &mut TestAppContext) {
    let setup = setup(
        cx,
        Options {
            readonly: true,
            ..Options::default()
        },
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.render_frame(cx);
        assert_eq!(window.find("name").focused(), Some(true));
        window.input("x", cx);
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "");
}

#[gpui_kit::test]
fn on_touch_the_frame_is_a_hit_target_tall(cx: &mut TestAppContext) {
    let setup = setup(cx, Options::default());
    cx.update(|cx| Theme::update(cx, |theme| theme.touch = true));
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("name").bounds().size.height,
            px(44.),
            "the HIG hit target"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_prefix_and_suffix_sit_inside_the_frame(cx: &mut TestAppContext) {
    let setup = setup(
        cx,
        Options {
            affixes: true,
            ..Options::default()
        },
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let frame = window.find("name").bounds();
        let suffix = window.find("suffix").bounds();
        assert_eq!(frame.size.height, px(32.), "affixes do not grow the frame");
        assert!(frame.contains(&suffix.origin));
        assert!(frame.contains(&suffix.bottom_right()));
        assert!(
            suffix.right() < frame.right() - px(11.),
            "the suffix sits inside the horizontal padding"
        );
        window.click("name", cx);
        window.input("hi", cx);
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "hi");
}

#[gpui_kit::test]
fn the_clear_button_empties_the_value_and_keeps_focus(cx: &mut TestAppContext) {
    let setup = setup(
        cx,
        Options {
            cleanable: true,
            ..Options::default()
        },
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert!(
            window.try_find("clear").is_none(),
            "no button while the value is empty"
        );
        window.click("name", cx);
        window.input("hi", cx);
        window.click("clear", cx);
        window.render_frame(cx);
        assert!(
            setup
                .state
                .read(cx)
                .presentation()
                .focus_handle()
                .is_focused(window),
            "the caret stays in the field"
        );
        assert!(window.try_find("clear").is_none());
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "");
}

#[gpui_kit::test]
fn the_mask_toggle_shows_and_hides_the_value(cx: &mut TestAppContext) {
    let setup = setup(
        cx,
        Options {
            masked: true,
            ..Options::default()
        },
    );
    let masked = |cx: &mut TestAppContext| {
        setup
            .state
            .read_with(cx, |state, _| state.presentation().is_masked())
    };
    assert!(masked(cx));
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("mask-toggle", cx);
    })
    .unwrap();
    assert!(!masked(cx));
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("mask-toggle", cx);
    })
    .unwrap();
    assert!(masked(cx));
}
