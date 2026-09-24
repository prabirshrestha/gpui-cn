use gpui_cn::{ActiveTheme as _, Button, ButtonSize, Spinner, Switch, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};

use crate::{Story, note, page, section};

/// Spinners on their own, beside text, and in buttons that are busy.
pub struct SpinnerStory {
    saving: bool,
}

impl Story for SpinnerStory {
    fn title() -> &'static str {
        "Spinner"
    }

    fn icon() -> IconName {
        IconName::LoaderCircle
    }

    fn description() -> &'static str {
        "An indicator that can be used to show a loading state."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self { saving: true }).into()
    }
}

impl Render for SpinnerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let control = cx.theme().text_control;
        let muted = cx.theme().muted_foreground();
        let saving = self.saving;
        page([
            section(
                "Sizes and colors",
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(Spinner::new("default"))
                    .child(Spinner::new("large").size_8())
                    .child(Spinner::new("muted").text_color(muted)),
            )
            .into_any_element(),
            section(
                "Beside text",
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(control.size)
                    .line_height(control.line_height)
                    .text_color(muted)
                    .child(Spinner::new("inline"))
                    .child("Loading..."),
            )
            .into_any_element(),
            section(
                "In a button",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "A busy button shows a spinner in place of its icon and ignores \
                         presses until the work ends. It keeps its colors and its focus.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("busy-primary")
                                    .primary()
                                    .loading(true)
                                    .label("Submit"),
                            )
                            .child(
                                Button::new("busy-default")
                                    .loading(true)
                                    .label("Please wait"),
                            )
                            .child(
                                Button::new("busy-outline")
                                    .outline()
                                    .loading(true)
                                    .label("Loading"),
                            )
                            .child(
                                Button::new("busy-ghost")
                                    .ghost()
                                    .size(ButtonSize::Sm)
                                    .loading(true)
                                    .label("Refreshing"),
                            )
                            .child(
                                Button::new("busy-icon")
                                    .outline()
                                    .icon(IconName::Copy)
                                    .loading(true)
                                    .accessibility_label("Copy")
                                    .tooltip("Copy"),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Toggle",
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        Button::new("save")
                            .primary()
                            .icon(IconName::Check)
                            .loading(saving)
                            .label(if saving { "Saving" } else { "Save" }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Switch::new("saving")
                                    .checked(saving)
                                    .accessibility_label("Busy")
                                    .on_change(cx.listener(|this, checked, _, cx| {
                                        this.saving = *checked;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(control.size)
                                    .line_height(control.line_height)
                                    .child("Busy"),
                            ),
                    ),
            )
            .into_any_element(),
        ])
    }
}
