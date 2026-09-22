use gpui_cn::{Textarea, TextareaState, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window, base::Disableable as _, div,
};

use crate::{Story, note, page, section};

/// The multi-line text field in every state.
pub struct TextareaStory {
    notes: Entity<TextareaState>,
    growing: Entity<TextareaState>,
    disabled: Entity<TextareaState>,
}

impl Story for TextareaStory {
    fn title() -> &'static str {
        "Textarea"
    }

    fn icon() -> IconName {
        IconName::FileText
    }

    fn description() -> &'static str {
        "Displays a form textarea or a component that looks like a textarea."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| Self {
            notes: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("Type your message here.")
                    .auto_grow(3, 3)
            }),
            growing: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("Grows from two to six rows.")
                    .auto_grow(2, 6)
            }),
            disabled: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("Disabled")
                    .default_value("Cannot edit")
                    .auto_grow(3, 3)
            }),
        })
        .into()
    }
}

/// The width a story field takes, so a field does not run the page.
const FIELD_WIDTH: gpui_kit::Pixels = gpui_kit::px(320.);

impl Render for TextareaStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field = |textarea: Textarea| div().w(FIELD_WIDTH).child(textarea);
        page([
            section("Default", field(Textarea::new(&self.notes).id("notes"))).into_any_element(),
            section(
                "Auto grow",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The field grows with its lines between two and six rows, then scrolls.",
                        cx,
                    ))
                    .child(field(Textarea::new(&self.growing))),
            )
            .into_any_element(),
            section(
                "Disabled",
                field(Textarea::new(&self.disabled).disabled(true)),
            )
            .into_any_element(),
        ])
    }
}
