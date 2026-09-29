//! Tests the root's public styling contract through painted window output.

use gpui_cn::{ActiveTheme as _, Input, InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, base::Root, div, px, size, test::TestWindowExt as _, transparent_black,
};

struct Content {
    input: Entity<InputState>,
}

impl Render for Content {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(Input::new(&self.input))
    }
}

#[gpui_kit::test]
fn root_background_override_preserves_control_theme(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let handle = cx.open_window(size(px(400.), px(200.)), |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx));
        let content = cx.new(|_| Content { input });
        Root::new(content, window, cx).bg(transparent_black())
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let quads = window.painted_quads();
        assert!(
            !quads.iter().any(|quad| {
                quad.bounds.size.width == px(400.).scale(window.scale_factor())
                    && quad.bounds.size.height == px(200.).scale(window.scale_factor())
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.)
            }),
            "the transparent root must not paint a full-window fill"
        );
        let field = cx.theme().field;
        assert!(
            quads
                .iter()
                .any(|quad| quad.background.as_solid() == Some(field)),
            "the input must retain its default field fill"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn root_uses_the_theme_background_by_default(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let handle = cx.open_window(size(px(400.), px(200.)), |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx));
        let content = cx.new(|_| Content { input });
        Root::new(content, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let background = cx.theme().background();
        assert!(window.painted_quads().iter().any(|quad| {
            quad.bounds.size.width == px(400.).scale(window.scale_factor())
                && quad.bounds.size.height == px(200.).scale(window.scale_factor())
                && quad.background.as_solid() == Some(background)
        }));
    })
    .unwrap();
}
