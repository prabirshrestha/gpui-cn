use gpui_cn::{ActiveTheme as _, Theme, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, FontWeight, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, base::StyledExt as _, div, px, rems,
};

use crate::{Story, note, page, section};

/// The type scale: the theme's text tokens, the rem helpers on top of
/// them, the sizes the reference application uses, and shadcn's prose
/// styles.
pub struct TypographyStory;

impl Story for TypographyStory {
    fn title() -> &'static str {
        "Typography"
    }

    fn icon() -> IconName {
        IconName::ALargeSmall
    }

    fn description() -> &'static str {
        "The text tokens and the prose styles built from them."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

const SAMPLE: &str = "The quick brown fox jumps over the lazy dog";

impl Render for TypographyStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground();
        let border = theme.border();
        let soft = theme.secondary();
        let radius = theme.radius_sm();
        let mono = theme.mono_font_family().clone();
        let typography = theme.base.typography.clone();
        let ui_font_size = Theme::global(cx).ui_font_size;

        let tokens = [
            ("xs", typography.xs),
            ("sm", typography.sm),
            ("md", typography.md),
            ("lg", typography.lg),
            ("xl", typography.xl),
        ];
        let scale_note = format!(
            "Sizes come from the UI font size ({}px, the window rem) and scale with it; \
             change it in Settings. Line heights scale with them.",
            f32::from(ui_font_size)
        );

        page([
            section(
                "Token scale",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(scale_note, cx))
                    .children(tokens.into_iter().map(|(name, token)| {
                        div()
                            .flex()
                            .items_baseline()
                            .gap_6()
                            .pb_3()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .w(px(120.))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_family(mono.clone())
                                    .text_color(muted)
                                    .child(SharedString::from(format!(
                                        "{name}  {}/{}",
                                        f32::from(token.size),
                                        f32::from(token.line_height)
                                    ))),
                            )
                            .child(
                                div()
                                    .text_size(token.size)
                                    .line_height(token.line_height)
                                    .child(SAMPLE),
                            )
                    }))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_6()
                            .child(
                                div()
                                    .w(px(120.))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_family(mono.clone())
                                    .text_color(muted)
                                    .child(SharedString::from(format!(
                                        "mono  {}/{}",
                                        f32::from(typography.mono_md.size),
                                        f32::from(typography.mono_md.line_height)
                                    ))),
                            )
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(typography.mono_md.size)
                                    .line_height(typography.mono_md.line_height)
                                    .child("let quick = fox.jump(over: lazy_dog);"),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Weights",
                div()
                    .flex()
                    .flex_wrap()
                    .gap_6()
                    .child(weight("Normal", FontWeight::NORMAL))
                    .child(weight("Medium", FontWeight::MEDIUM))
                    .child(weight("Semibold", FontWeight::SEMIBOLD))
                    .child(weight("Bold", FontWeight::BOLD)),
            )
            .into_any_element(),
            section(
                "Reference sizes",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "The sizes gpui-cn components use, sampled from the reference \
                         application at the 16px rem. Controls and rows read at 13px, \
                         descriptions at 12px, page titles at 24px.",
                        cx,
                    ))
                    .child(reference_row(
                        "Page title",
                        "24px semibold, rems(1.5)",
                        div().text_size(rems(1.5)).font_semibold().child("General"),
                        muted,
                        &mono,
                    ))
                    .child(reference_row(
                        "Section heading",
                        "15px medium, rems(0.9375)",
                        div().text_size(rems(0.9375)).font_medium().child("Permissions"),
                        muted,
                        &mono,
                    ))
                    .child(reference_row(
                        "Sidebar title",
                        "15px semibold, rems(0.9375)",
                        div().text_size(rems(0.9375)).font_semibold().child("gpui-cn"),
                        muted,
                        &mono,
                    ))
                    .child(reference_row(
                        "Control and row",
                        "13px, rems(0.8125)",
                        div().text_size(rems(0.8125)).child("Default permissions"),
                        muted,
                        &mono,
                    ))
                    .child(reference_row(
                        "Description",
                        "12px muted, text_xs",
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("By default, the app can read and edit files in its workspace."),
                        muted,
                        &mono,
                    ))
                    .child(reference_row(
                        "Group label",
                        "13px sidebar muted",
                        div()
                            .text_size(rems(0.8125))
                            .text_color(theme.sidebar_muted_foreground)
                            .child("Projects"),
                        muted,
                        &mono,
                    )),
            )
            .into_any_element(),
            section(
                "Prose",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .max_w(px(640.))
                    .child(note(
                        "shadcn's typography examples on the rem helpers: h1 to h4, lead, \
                         paragraph, blockquote, list, inline code, large, small, and muted.",
                        cx,
                    ))
                    .child(
                        div()
                            .text_size(rems(2.25))
                            .font_bold()
                            .line_height(rems(2.5))
                            .child("Taxing Laughter: The Joke Tax Chronicles"),
                    )
                    .child(
                        div()
                            .text_xl()
                            .text_color(muted)
                            .child("A modest proposal for funding the royal treasury one punchline at a time."),
                    )
                    .child(
                        div()
                            .text_3xl()
                            .font_semibold()
                            .pb_2()
                            .border_b_1()
                            .border_color(border)
                            .child("The People of the Kingdom"),
                    )
                    .child(div().text_base().line_height(rems(1.75)).child(
                        "The king, seeing how much happier his subjects were, realized the error \
                         of his ways and repealed the joke tax. Jokester was declared a hero, and \
                         the kingdom lived happily ever after.",
                    ))
                    .child(div().text_2xl().font_semibold().child("The Joke Tax"))
                    .child(
                        div()
                            .pl_6()
                            .border_l_2()
                            .border_color(border)
                            .text_base()
                            .italic()
                            .line_height(rems(1.75))
                            .child(
                                "\"After all,\" he said, \"everyone enjoys a good joke, so it's only \
                                 fair that they should pay for the privilege.\"",
                            ),
                    )
                    .child(div().text_xl().font_semibold().child("The King's Plan"))
                    .child(table(
                        ["King's Treasury", "People's happiness"],
                        [
                            ["Empty", "Overflowing"],
                            ["Modest", "Satisfied"],
                            ["Full", "Ecstatic"],
                        ],
                        border,
                        soft,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pl_6()
                            .text_base()
                            .child("1st level of puns: 5 gold coins")
                            .child("2nd level of jokes: 10 gold coins")
                            .child("3rd level of one-liners: 20 gold coins"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_baseline()
                            .gap_1()
                            .text_base()
                            .child("Inline code such as")
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded(radius)
                                    .bg(soft)
                                    .font_family(mono.clone())
                                    .text_sm()
                                    .child("Button::new(\"save\")"),
                            )
                            .child("sits on the soft fill in the mono family."),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child("Are you absolutely sure?"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .line_height(rems(1.))
                            .child("Email address"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("Enter your email address."),
                    ),
            )
            .into_any_element(),
        ])
    }
}

/// shadcn's table example: a header row on the soft fill, hairlines
/// between rows, cells padded on the rem scale.
fn table<const C: usize, const R: usize>(
    header: [&'static str; C],
    rows: [[&'static str; C]; R],
    border: gpui_kit::Hsla,
    soft: gpui_kit::Hsla,
) -> impl IntoElement {
    let cell = |text: &'static str| div().flex_1().px_4().py_2().text_sm().child(text);
    div()
        .flex()
        .flex_col()
        .w_full()
        .border_1()
        .border_color(border)
        .rounded_md()
        .overflow_hidden()
        .child(
            div()
                .flex()
                .bg(soft)
                .font_bold()
                .children(header.into_iter().map(cell)),
        )
        .children(rows.into_iter().map(move |row| {
            div()
                .flex()
                .border_t_1()
                .border_color(border)
                .children(row.into_iter().map(cell))
        }))
}

fn weight(name: &'static str, weight: FontWeight) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_lg().font_weight(weight).child(SAMPLE))
        .child(div().text_xs().child(name))
}

fn reference_row(
    name: &'static str,
    spec: &'static str,
    sample: impl IntoElement,
    muted: gpui_kit::Hsla,
    mono: &SharedString,
) -> impl IntoElement {
    div()
        .flex()
        .items_baseline()
        .gap_6()
        .child(
            div()
                .w(px(180.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .child(div().text_sm().child(name))
                .child(
                    div()
                        .text_xs()
                        .font_family(mono.clone())
                        .text_color(muted)
                        .child(spec),
                ),
        )
        .child(sample)
}
