use gpui_cn::{ActiveTheme as _, Skeleton, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};

use crate::{Story, note, page, section};

/// Skeletons in the shapes content takes while it loads.
pub struct SkeletonStory;

impl Story for SkeletonStory {
    fn title() -> &'static str {
        "Skeleton"
    }

    fn icon() -> IconName {
        IconName::Frame
    }

    fn description() -> &'static str {
        "Use to show a placeholder while content is loading."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

impl Render for SkeletonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let card_radius = theme.radius_lg();
        let border = theme.border();
        page([
            section(
                "Lines",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(320.))
                    .child(note(
                        "A block a line of text high, pulsing between full and half strength \
                         over two seconds. Size it with the same helpers as any element.",
                        cx,
                    ))
                    .child(Skeleton::new("line-1"))
                    .child(Skeleton::new("line-2").w_3_4())
                    .child(Skeleton::new("line-3").w_1_2()),
            )
            .into_any_element(),
            section(
                "A row with an avatar",
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .w(px(320.))
                    .child(Skeleton::new("avatar").size_10().rounded_full())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .flex_1()
                            .child(Skeleton::new("name").w_2_3())
                            .child(Skeleton::new("email").w_1_3()),
                    ),
            )
            .into_any_element(),
            section(
                "A card",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(320.))
                    .p_4()
                    .rounded(card_radius)
                    .border_1()
                    .border_color(border)
                    .child(Skeleton::new("image").h(px(120.)).rounded_lg())
                    .child(Skeleton::new("title").w_3_4())
                    .child(Skeleton::new("body")),
            )
            .into_any_element(),
        ])
    }
}
