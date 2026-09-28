use gpui_cn::{Button, Popover, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Align, div, px,
};

use crate::{Story, note, page, section};

/// Panels of any content under a trigger.
pub struct PopoverStory;

impl Story for PopoverStory {
    fn title() -> &'static str {
        "Popover"
    }

    fn icon() -> IconName {
        IconName::Info
    }

    fn description() -> &'static str {
        "A panel of any content that opens under a trigger."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

impl Render for PopoverStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = |title: &'static str, text: &'static str| {
            div()
                .w(px(260.))
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(title))
                .child(text)
        };
        page([section(
            "Alignment",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(
                    "A press or Enter opens the panel; Escape or a press outside closes it.",
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            Popover::new("popover-start")
                                .trigger(
                                    Button::new("popover-start-trigger")
                                        .outline()
                                        .label("Start"),
                                )
                                .content(move |_, _| {
                                    body("Dimensions", "Lined up with the trigger's leading edge.")
                                }),
                        )
                        .child(
                            Popover::new("popover-end")
                                .align(Align::End)
                                .trigger(Button::new("popover-end-trigger").outline().label("End"))
                                .content(move |_, _| {
                                    body("Dimensions", "Lined up with the trigger's trailing edge.")
                                }),
                        ),
                ),
        )
        .into_any_element()])
    }
}
