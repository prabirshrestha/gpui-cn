use gpui_cn::{ActiveTheme as _, Button, ButtonSize, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, DefiniteLength, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Window, div, prelude::FluentBuilder as _, px, rems,
};

use crate::{Story, note, page, section};

/// The spacing scale: the theme's spacing tokens, the rem helpers, the
/// radius scale, and the gaps the reference application keeps.
pub struct SpacingStory;

impl Story for SpacingStory {
    fn title() -> &'static str {
        "Spacing"
    }

    fn icon() -> IconName {
        IconName::Frame
    }

    fn description() -> &'static str {
        "The spacing and radius scales and how components keep them."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

/// The rem helpers a layout uses, with their value at the 16px rem.
const REM_STEPS: [(&str, f32); 9] = [
    ("0p5", 0.125),
    ("1", 0.25),
    ("1p5", 0.375),
    ("2", 0.5),
    ("3", 0.75),
    ("4", 1.),
    ("6", 1.5),
    ("8", 2.),
    ("12", 3.),
];

impl Render for SpacingStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground();
        let border = theme.border();
        let soft = theme.secondary();
        let accent = theme.ring();
        let card = theme.base.colors.surface;
        let mono = theme.mono_font_family().clone();
        let spacing = theme.base.spacing;
        let radius = theme.base.radius;

        let tokens = [
            ("xxs", spacing.xxs),
            ("xs", spacing.xs),
            ("sm", spacing.sm),
            ("md", spacing.md),
            ("lg", spacing.lg),
            ("xl", spacing.xl),
            ("xxl", spacing.xxl),
        ];
        let radii = [
            ("sm", radius.sm),
            ("md", radius.md),
            ("lg", radius.lg),
            ("xl", radius.xl),
            ("full", radius.full),
        ];

        page([
            section(
                "Spacing tokens",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w_full()
                    .child(note(
                        "Base's spacing tokens, in pixels. They do not move with the UI font \
                         size; they are for layout that stays put, such as window chrome.",
                        cx,
                    ))
                    .children(tokens.into_iter().map(|(name, value)| {
                        bar(
                            SharedString::from(format!("{name}  {}px", f32::from(value))),
                            value.into(),
                            accent,
                            muted,
                            &mono,
                        )
                    })),
            )
            .into_any_element(),
            section(
                "Rem helpers",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w_full()
                    .child(note(
                        "GPUI's p_, m_, gap_, w_, and h_ helpers are on the rem scale: one \
                         step is a quarter rem, 4px at the 16px rem. Components use these, so \
                         their padding and gaps grow with the text. Change the text size in \
                         Settings to see the bars move.",
                        cx,
                    ))
                    .children(REM_STEPS.into_iter().map(|(name, value)| {
                        bar(
                            SharedString::from(format!("_{name}  {}rem", value)),
                            rems(value).into(),
                            accent,
                            muted,
                            &mono,
                        )
                    })),
            )
            .into_any_element(),
            section(
                "Radius scale",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "One base radius (10px) and the scale derived from it: sm for badges \
                         and menu items, md for buttons and inputs, lg for cards and popovers, \
                         xl for sheets and panels, full for pills.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_4()
                            .children(radii.into_iter().map(|(name, value)| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .size(px(64.))
                                            .rounded(value)
                                            .bg(soft)
                                            .border_1()
                                            .border_color(border),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(mono.clone())
                                            .text_color(muted)
                                            .child(SharedString::from(if value > px(1000.) {
                                                name.to_string()
                                            } else {
                                                format!("{name}  {}px", f32::from(value))
                                            })),
                                    )
                            })),
                    ),
            )
            .into_any_element(),
            section(
                "In components",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "The gaps the reference application keeps: 8px between a row's icon \
                         and label, 6px between an icon and a button label, 4px between \
                         buttons in a group, 12px between a control and its neighbor, 16px \
                         of card padding, 24px of page padding.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .max_w(px(560.))
                            .rounded(radius.xl)
                            .border_1()
                            .border_color(border)
                            .bg(card)
                            .px_4()
                            .child(measured_row(
                                "Card padding",
                                "px_4 (16px), py_3p5 (14px)",
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(pill("Bottom"))
                                    .child(pill("Right")),
                                border,
                                muted,
                                &mono,
                                false,
                            ))
                            .child(measured_row(
                                "Buttons in a group",
                                "gap_1 (4px)",
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(Button::new("sp-a").size(ButtonSize::Sm).label("Cancel"))
                                    .child(
                                        Button::new("sp-b")
                                            .size(ButtonSize::Sm)
                                            .primary()
                                            .label("Save"),
                                    ),
                                border,
                                muted,
                                &mono,
                                true,
                            ))
                            .child(measured_row(
                                "Control and neighbor",
                                "gap_3 (12px)",
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(div().text_size(rems(0.8125)).child("Label"))
                                    .child(
                                        Button::new("sp-c").size(ButtonSize::Sm).label("Change"),
                                    ),
                                border,
                                muted,
                                &mono,
                                true,
                            )),
                    ),
            )
            .into_any_element(),
        ])
    }
}

/// A labeled bar whose width is the value.
fn bar(
    label: SharedString,
    width: DefiniteLength,
    fill: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
    mono: &SharedString,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_4()
        .child(
            div()
                .w(px(120.))
                .flex_shrink_0()
                .text_xs()
                .font_family(mono.clone())
                .text_color(muted)
                .child(label),
        )
        .child(div().h_3().w(width).rounded_sm().bg(fill))
}

fn pill(label: &'static str) -> impl IntoElement {
    Button::new(label)
        .ghost()
        .size(ButtonSize::Sm)
        .rounded_full()
        .label(label)
}

#[allow(clippy::too_many_arguments)]
fn measured_row(
    title: &'static str,
    spec: &'static str,
    control: impl IntoElement,
    border: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
    mono: &SharedString,
    separated: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .py_3p5()
        .when(separated, |this| this.border_t_1().border_color(border))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .min_w_0()
                .child(div().text_size(rems(0.8125)).child(title))
                .child(
                    div()
                        .text_xs()
                        .font_family(mono.clone())
                        .text_color(muted)
                        .child(spec),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}
