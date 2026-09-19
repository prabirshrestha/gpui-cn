use gpui_cn::{
    Button, ButtonSize, ButtonVariant, gpui_kit::assets::IconName, gpui_kit::base::Placement,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Disableable as _, base::Selectable as _, div, prelude::FluentBuilder as _,
};

use crate::{Story, page, section};

/// Every variant, size, and state of `Button`.
pub struct ButtonStory {
    clicks: usize,
    selected: bool,
}

impl Story for ButtonStory {
    fn title() -> &'static str {
        "Button"
    }

    fn icon() -> IconName {
        IconName::SquareTerminal
    }

    fn description() -> &'static str {
        "Displays a button or a component that looks like a button."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self {
            clicks: 0,
            selected: false,
        })
        .into()
    }
}

const VARIANTS: [(&str, ButtonVariant); 6] = [
    ("Default", ButtonVariant::Default),
    ("Primary", ButtonVariant::Primary),
    ("Destructive", ButtonVariant::Destructive),
    ("Outline", ButtonVariant::Outline),
    ("Ghost", ButtonVariant::Ghost),
    ("Link", ButtonVariant::Link),
];

impl Render for ButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected;
        page([
            section(
                "Variants",
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(VARIANTS.iter().enumerate().map(|(ix, (label, variant))| {
                        Button::new(("variant", ix))
                            .variant(*variant)
                            .label(*label)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clicks += 1;
                                cx.notify();
                            }))
                    })),
            )
            .into_any_element(),
            section(
                "Sizes",
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(Button::new("xs").size(ButtonSize::Xs).label("Extra small"))
                    .child(Button::new("sm").size(ButtonSize::Sm).label("Small"))
                    .child(Button::new("default").label("Default"))
                    .child(Button::new("lg").size(ButtonSize::Lg).label("Large")),
            )
            .into_any_element(),
            section(
                "With icon",
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("new-branch")
                            .icon(IconName::Plus)
                            .label("New item"),
                    )
                    .child(
                        Button::new("open")
                            .label("Open")
                            .trailing_icon(IconName::ExternalLink),
                    )
                    .child(
                        Button::new("next")
                            .primary()
                            .size(ButtonSize::Sm)
                            .label("Next")
                            .trailing_icon(IconName::ArrowRight),
                    ),
            )
            .into_any_element(),
            section(
                "Icon",
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("icon-xs")
                            .size(ButtonSize::Xs)
                            .icon(IconName::Ellipsis)
                            .accessibility_label("More")
                            .tooltip("More"),
                    )
                    .child(
                        Button::new("icon-sm")
                            .size(ButtonSize::Sm)
                            .icon(IconName::Ellipsis)
                            .accessibility_label("More")
                            .tooltip("More"),
                    )
                    .child(
                        Button::new("icon")
                            .icon(IconName::Ellipsis)
                            .accessibility_label("More")
                            .tooltip("More"),
                    )
                    .child(
                        Button::new("icon-lg")
                            .size(ButtonSize::Lg)
                            .icon(IconName::Ellipsis)
                            .accessibility_label("More")
                            .tooltip_at(Placement::Bottom, "More, below"),
                    )
                    .child(
                        Button::new("icon-ghost")
                            .ghost()
                            .icon(IconName::Settings)
                            .accessibility_label("Settings")
                            .tooltip("Settings"),
                    )
                    .child(
                        Button::new("icon-outline")
                            .outline()
                            .icon(IconName::Plus)
                            .accessibility_label("Add")
                            .tooltip("Add"),
                    ),
            )
            .into_any_element(),
            section(
                "Disabled",
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(VARIANTS.iter().enumerate().map(|(ix, (label, variant))| {
                        Button::new(("disabled", ix))
                            .variant(*variant)
                            .disabled(true)
                            .label(*label)
                    }))
                    .child(
                        Button::new("disabled-reason")
                            .primary()
                            .disabled(true)
                            .label("Upload")
                            .tooltip("Select a file first"),
                    ),
            )
            .into_any_element(),
            section(
                "Selected",
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("toggle-selected")
                            .label("Bold")
                            .toggled(selected)
                            .selected(selected)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.selected = !this.selected;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("selected-default")
                            .label("Selected")
                            .selected(true),
                    )
                    .child(
                        Button::new("selected-ghost")
                            .ghost()
                            .label("Selected ghost")
                            .selected(true),
                    ),
            )
            .into_any_element(),
            section(
                "Rounded",
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("pill")
                            .primary()
                            .rounded_full()
                            .label("Continue")
                            .trailing_icon(IconName::ArrowUp),
                    )
                    .child(
                        Button::new("round-icon")
                            .rounded_full()
                            .icon(IconName::Plus)
                            .accessibility_label("Add")
                            .tooltip("Add"),
                    ),
            )
            .into_any_element(),
            section(
                "Truncation (hover for the full label)",
                div().w_40().child(
                    Button::new("long")
                        .w_full()
                        .label("A label that is much longer than the button"),
                ),
            )
            .into_any_element(),
            section(
                "Clicks",
                div()
                    .text_sm()
                    .child(format!("Variant buttons clicked {} times", self.clicks))
                    .when(self.clicks == 0, |this| this.child(" (none yet)")),
            )
            .into_any_element(),
        ])
    }
}
