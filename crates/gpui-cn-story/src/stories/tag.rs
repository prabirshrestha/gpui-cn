use gpui_cn::{Tag, TagVariant, gpui_kit::assets::IconName};

use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};

use crate::{Story, page, section};

/// Tags in each color, filled and outlined, with and without an icon.
pub struct TagStory;

impl Story for TagStory {
    fn title() -> &'static str {
        "Tag"
    }

    fn icon() -> IconName {
        IconName::CircleCheck
    }

    fn description() -> &'static str {
        "A small labeled pill for a status or a category."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

/// The colors, with a label and an id suffix for each.
const VARIANTS: [(TagVariant, &str, &str); 6] = [
    (TagVariant::Primary, "Primary", "primary"),
    (TagVariant::Secondary, "Secondary", "secondary"),
    (TagVariant::Danger, "Danger", "danger"),
    (TagVariant::Success, "Success", "success"),
    (TagVariant::Warning, "Warning", "warning"),
    (TagVariant::Info, "Info", "info"),
];

impl Render for TagStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = || div().flex().flex_wrap().items_center().gap_2();
        page([
            section(
                "Variants",
                row().children(VARIANTS.map(|(variant, label, id)| {
                    Tag::new(format!("filled-{id}"))
                        .label(label)
                        .variant(variant)
                })),
            )
            .into_any_element(),
            section(
                "Outline",
                row().children(VARIANTS.map(|(variant, label, id)| {
                    Tag::new(format!("outline-{id}"))
                        .label(label)
                        .variant(variant)
                        .outline()
                })),
            )
            .into_any_element(),
            section(
                "With icon",
                row()
                    .child(
                        Tag::new("verified")
                            .icon(IconName::CircleCheck)
                            .label("Verified")
                            .success(),
                    )
                    .child(
                        Tag::new("done")
                            .icon(IconName::Check)
                            .label("Done")
                            .outline(),
                    ),
            )
            .into_any_element(),
        ])
    }
}
