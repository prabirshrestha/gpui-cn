use gpui_cn::{Avatar, Badge, Button, Icon, Tag, gpui_kit::assets::IconName};

use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};

use crate::{Story, page, section};

/// Counts, dots, and icons on buttons, icons, tags, and avatars.
pub struct BadgeStory;

impl Story for BadgeStory {
    fn title() -> &'static str {
        "Badge"
    }

    fn icon() -> IconName {
        IconName::Bell
    }

    fn description() -> &'static str {
        "A count, a dot, or an icon at the corner of another element."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

impl Render for BadgeStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = || div().flex().flex_wrap().items_center().gap_6();
        let avatar = |id: &str| Avatar::new(format!("avatar-{id}")).name("Ada Lovelace");
        page([
            section(
                "On any element",
                row()
                    .child(
                        Badge::new("bell-count")
                            .count(5)
                            .child(Button::new("bell").ghost().icon(IconName::Bell)),
                    )
                    .child(
                        Badge::new("inbox-count").count(12).child(
                            Button::new("inbox")
                                .outline()
                                .icon(IconName::Inbox)
                                .label("Inbox"),
                        ),
                    )
                    .child(
                        Badge::new("icon-dot")
                            .dot()
                            .child(Icon::from(IconName::Bell).size_5()),
                    )
                    .child(
                        Badge::new("tag-dot")
                            .dot()
                            .child(Tag::new("beta").label("Beta").secondary()),
                    ),
            )
            .into_any_element(),
            section(
                "Count",
                row()
                    .child(Badge::new("count-one").count(3).child(avatar("one")))
                    .child(Badge::new("count-two").count(42).child(avatar("two")))
                    .child(
                        Badge::new("count-max")
                            .count(120)
                            .danger()
                            .child(avatar("max")),
                    )
                    .child(Badge::new("count-zero").count(0).child(avatar("zero"))),
            )
            .into_any_element(),
            section(
                "Dot and icon",
                row()
                    .child(Badge::new("dot").dot().child(avatar("dot")))
                    .child(
                        Badge::new("dot-alert")
                            .dot()
                            .danger()
                            .child(avatar("dot-alert")),
                    )
                    .child(
                        Badge::new("dot-success")
                            .dot()
                            .success()
                            .child(avatar("dot-success")),
                    )
                    .child(
                        Badge::new("icon")
                            .icon(IconName::Check)
                            .success()
                            .child(avatar("icon")),
                    ),
            )
            .into_any_element(),
        ])
    }
}
