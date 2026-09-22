//! UI integration tests for `Textarea`: real input through a headless
//! window.

use std::{cell::Cell, rc::Rc};

use gpui_cn::{Root, Textarea, TextareaState};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Role, Styled as _,
    TestAppContext, Window, base::Disableable as _, div, px, size, test::TestWindowExt as _,
};

struct Harness {
    state: Entity<TextareaState>,
    disabled: bool,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            Textarea::new(&self.state)
                .id("notes")
                .disabled(self.disabled)
                .accessibility_label("Notes"),
        )
    }
}

struct Setup {
    handle: gpui_kit::WindowHandle<Root>,
    state: Entity<TextareaState>,
}

fn setup(cx: &mut TestAppContext, rows: usize, disabled: bool) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let state = Rc::new(Cell::new(None));
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let textarea = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Notes")
                .auto_grow(rows, rows)
        });
        state.set(Some(textarea.clone()));
        let harness = cx.new(|_| Harness {
            state: textarea,
            disabled,
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
    }
}

#[gpui_kit::test]
fn the_rows_sit_between_the_reference_padding(cx: &mut TestAppContext) {
    let one = setup(cx, 1, false);
    cx.update_window(one.handle.into(), |_, window, cx| {
        let frame = window.find("notes");
        assert_eq!(frame.label(), Some("Notes"));
        assert_eq!(frame.role(), Some(Role::MultilineTextInput));
        assert_eq!(
            frame.bounds().size.height,
            px(37.),
            "one row: 18.5px rounded up to 19 by the layout, 8px padding and the hairline above and below"
        );
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "an idle field asks for no frame"
        );
    })
    .unwrap();
    let three = setup(cx, 3, false);
    cx.update_window(three.handle.into(), |_, window, _| {
        assert_eq!(
            window.find("notes").bounds().size.height,
            px(75.),
            "three rows: 3 x 18.5px rounded up to 57 by the layout, 8px padding and the hairline above and below"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn enter_starts_a_new_line(cx: &mut TestAppContext) {
    let setup = setup(cx, 3, false);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("notes", cx);
        window.render_frame(cx);
        assert_eq!(window.find("notes").focused(), Some(true));
        window.input("a", cx);
        window.press("enter", cx);
        window.input("b", cx);
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "a\nb");
}

#[gpui_kit::test]
fn a_disabled_field_ignores_input(cx: &mut TestAppContext) {
    let setup = setup(cx, 3, true);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("notes", cx);
        window.render_frame(cx);
        assert_ne!(
            window.find("notes").focused(),
            Some(true),
            "a click does not focus a disabled field"
        );
        let mut key = gpui_kit::Keystroke::parse("x").unwrap();
        key.key_char = Some("x".into());
        window.dispatch_keystroke(key, cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(setup.state.read_with(cx, |state, _| state.value()), "");
}
