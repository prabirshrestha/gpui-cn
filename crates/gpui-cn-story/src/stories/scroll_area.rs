use gpui_cn::{ActiveTheme as _, ScrollArea, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};

use crate::{Story, frame, note, page, section};

/// A scroll region that bounces at its ends.
pub struct ScrollAreaStory;

impl Story for ScrollAreaStory {
    fn title() -> &'static str {
        "Scroll area"
    }

    fn icon() -> IconName {
        IconName::ArrowDown
    }

    fn description() -> &'static str {
        "A vertical scroll region that stretches past its ends and springs back."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

impl Render for ScrollAreaStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, muted) = {
            let theme = cx.theme();
            (theme.border(), theme.muted_foreground())
        };
        page([
            section(
                "Bounce",
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Scroll past the top or the bottom with a trackpad or a finger. \
                         The rows stretch away from the edge and spring back when you \
                         let go. Reduced motion turns the bounce off.",
                        cx,
                    ))
                    .child(
                        frame(px(240.), cx).child(
                            ScrollArea::new("rows")
                                .size_full()
                                .p_2()
                                .children((1..=40).map(|n| {
                                    div()
                                        .h(px(30.))
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(border)
                                        .child(div().text_sm().child(format!("Row {n}")))
                                        .child(div().flex_1())
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(format!("{}", n * 30)),
                                        )
                                })),
                        ),
                    ),
            ),
            section(
                "Where it is used",
                div().w_full().child(note(
                    "The sidebar's rows, each story page, and the settings page scroll \
                     in a ScrollArea, so the whole gallery has the same end of the page.",
                    cx,
                )),
            ),
        ])
    }
}
