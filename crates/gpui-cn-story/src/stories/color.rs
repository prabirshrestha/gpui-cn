use std::time::Duration;

use gpui_cn::{
    ActiveTheme as _, Tag,
    gpui_kit::assets::IconName,
    theme::{ThemeTokens, contrast_ratio, to_hex},
};
use gpui_kit::{
    AnyView, App, AppContext as _, ClipboardItem, Context, ElementId, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Task, Window, base::TestSupportExt as _, div,
    prelude::FluentBuilder as _, px,
};

use crate::{Story, note, page, section};

/// The roles the swatches group by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorGroup {
    /// The surfaces things sit on.
    Surfaces,
    /// Text and icons.
    Text,
    /// Hairlines and focus rings.
    Borders,
    /// The accent and what sits on it.
    Accent,
    /// Rows in their states.
    Rows,
    /// Status colors, as badges and tags use them.
    Status,
}

impl ColorGroup {
    /// Every group, in the order the page lists them.
    pub const ALL: [ColorGroup; 6] = [
        ColorGroup::Surfaces,
        ColorGroup::Text,
        ColorGroup::Borders,
        ColorGroup::Accent,
        ColorGroup::Rows,
        ColorGroup::Status,
    ];

    fn title(self) -> &'static str {
        match self {
            ColorGroup::Surfaces => "Surfaces",
            ColorGroup::Text => "Text",
            ColorGroup::Borders => "Borders and rings",
            ColorGroup::Accent => "Accent",
            ColorGroup::Rows => "Rows and states",
            ColorGroup::Status => "Status",
        }
    }

    /// How the theme derives the group, in one sentence.
    fn rule(self) -> &'static str {
        match self {
            ColorGroup::Surfaces => {
                "Each surface is the window surface moved a fixed step toward the ink, so a \
                 new surface is one more step, never a literal color."
            }
            ColorGroup::Text => {
                "Text is the ink, or the ink mixed back toward its surface for muted and \
                 placeholder text, and stepped until it keeps its contrast."
            }
            ColorGroup::Borders => {
                "A hairline is the surface moved a small step toward the ink. Focus colors \
                 come from the accent."
            }
            ColorGroup::Accent => {
                "The accent comes from the theme's config. The primary button is the ink a \
                 step short of the surface, with its label a step short of the surface."
            }
            ColorGroup::Rows => {
                "A hovered row is one step from its surface and a chosen row a second step. \
                 A menu's highlight and a hovered picker row sit between them."
            }
            ColorGroup::Status => {
                "A tint is the surface moved toward the status hue, and a solid is the hue \
                 darkened until a light label reads on it at 4.5:1."
            }
        }
    }
}

/// One token of the registry: its name, the group it shows in, and how to
/// read it from the theme. Adding a token to the page is adding a row.
#[derive(Clone, Copy)]
pub struct ColorToken {
    /// The name, as the code reads it.
    pub name: &'static str,
    /// The group it shows in.
    pub group: ColorGroup,
    /// Reads the color from the theme.
    pub get: fn(&ThemeTokens) -> Hsla,
}

macro_rules! tokens {
    ($($group:ident: [$($name:literal => $get:expr),* $(,)?]),* $(,)?) => {
        &[$($(ColorToken { name: $name, group: ColorGroup::$group, get: $get },)*)*]
    };
}

/// Every color the page shows.
pub const REGISTRY: &[ColorToken] = tokens!(
    Surfaces: [
        "background" => |t| t.background(),
        "sidebar" => |t| t.sidebar,
        "field" => |t| t.field,
        "popover" => |t| t.popover,
        "card" => |t| t.base.colors.surface,
        "composer_card" => |t| t.composer_card,
        "status_tab" => |t| t.status_tab,
        "select_trigger" => |t| t.select_trigger,
    ],
    Text: [
        "foreground" => |t| t.foreground(),
        "muted_foreground" => |t| t.muted_foreground(),
        "composer_placeholder" => |t| t.composer_placeholder,
        "composer_muted" => |t| t.composer_muted,
        "popover_muted_foreground" => |t| t.popover_muted_foreground,
        "sidebar_muted_foreground" => |t| t.sidebar_muted_foreground,
        "warning_text" => |t| t.warning_text,
    ],
    Borders: [
        "border" => |t| t.border(),
        "input" => |t| t.input(),
        "field_border" => |t| t.field_border,
        "field_focus_border" => |t| t.field_focus_border,
        "select_trigger_border" => |t| t.select_trigger_border,
        "popover_border" => |t| t.popover_border,
        "popover_separator" => |t| t.popover_separator,
        "sidebar_border" => |t| t.sidebar_border,
        "composer_border" => |t| t.composer_border,
        "ring" => |t| t.ring(),
        "focus_ring" => |t| t.focus_ring(),
    ],
    Accent: [
        "control_accent" => |t| t.control_accent,
        "primary" => |t| t.primary(),
        "primary_foreground" => |t| t.primary_foreground(),
        "accent" => |t| t.accent(),
        "link" => |t| t.link,
        "selection" => |t| t.selection(),
    ],
    Rows: [
        "selected" => |t| t.selected,
        "sidebar_accent" => |t| t.sidebar_accent,
        "sidebar_selected" => |t| t.sidebar_selected,
        "popover_hover" => |t| t.popover_hover,
        "popover_accent" => |t| t.popover_accent,
    ],
    Status: [
        "destructive" => |t| t.destructive(),
        "success" => |t| t.success,
        "warning" => |t| t.warning,
        "info" => |t| t.info,
        "destructive_tint" => |t| t.destructive_tint,
        "success_tint" => |t| t.success_tint,
        "warning_tint" => |t| t.warning_tint,
        "info_tint" => |t| t.info_tint,
        "destructive_solid" => |t| t.destructive_solid,
        "success_solid" => |t| t.success_solid,
        "warning_solid" => |t| t.warning_solid,
        "info_solid" => |t| t.info_solid,
    ],
);

/// The text-on-surface pairs the contrast table measures, as
/// (label, text, surface).
type Getter = fn(&ThemeTokens) -> Hsla;

const PAIRS: &[(&str, Getter, Getter)] = &[
    (
        "foreground on background",
        |t| t.foreground(),
        |t| t.background(),
    ),
    (
        "muted_foreground on background",
        |t| t.muted_foreground(),
        |t| t.background(),
    ),
    (
        "popover_foreground on popover",
        |t| t.popover_foreground,
        |t| t.popover,
    ),
    (
        "popover_muted_foreground on popover",
        |t| t.popover_muted_foreground,
        |t| t.popover,
    ),
    (
        "sidebar_muted_foreground on sidebar",
        |t| t.sidebar_muted_foreground,
        |t| t.sidebar,
    ),
    (
        "composer_muted on composer_card",
        |t| t.composer_muted,
        |t| t.composer_card,
    ),
    (
        "primary_foreground on primary",
        |t| t.primary_foreground(),
        |t| t.primary(),
    ),
    ("link on background", |t| t.link, |t| t.background()),
];

/// How a ratio reads against WCAG: AAA from 7, AA from 4.5, and below
/// that the large-text threshold of 3.
pub fn grade(ratio: f32) -> &'static str {
    if ratio >= 7. {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3. {
        "AA large"
    } else {
        "Low"
    }
}

/// How long "Copied" stays on a swatch.
const COPIED_FOR: Duration = Duration::from_millis(1500);

/// The theme's colors, read live, by role.
pub struct ColorStory {
    copied: Option<&'static str>,
    reset: Option<Task<()>>,
}

impl ColorStory {
    /// The name of the swatch that was just copied.
    pub fn copied(&self) -> Option<&'static str> {
        self.copied
    }

    /// Copies the hex of the token `name` and shows "Copied" on its swatch
    /// for a moment.
    pub fn copy(&mut self, name: &'static str, cx: &mut Context<Self>) {
        let Some(token) = REGISTRY.iter().find(|token| token.name == name) else {
            return;
        };
        let hex = to_hex((token.get)(cx.theme()));
        cx.write_to_clipboard(ClipboardItem::new_string(hex));
        self.copied = Some(name);
        self.reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |this, cx| {
                this.copied = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

impl Story for ColorStory {
    fn title() -> &'static str {
        "Color"
    }

    fn icon() -> IconName {
        IconName::Palette
    }

    fn description() -> &'static str {
        "The theme's colors by role, read from the live theme, with the contrast of the main \
         text pairs. Click a swatch to copy its hex."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self {
            copied: None,
            reset: None,
        })
        .into()
    }
}

/// The width of one swatch card.
const SWATCH_WIDTH: f32 = 168.;

impl Render for ColorStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mono = theme.mono_font_family().clone();
        let (border, muted, radius) = (theme.border(), theme.muted_foreground(), theme.radius_md());
        let copied = self.copied;

        let swatch = |token: &ColorToken, cx: &mut Context<Self>| {
            let color = (token.get)(&theme);
            let hex = to_hex(color);
            let name = token.name;
            div()
                .id(ElementId::NamedChild(
                    ElementId::from("swatch").into(),
                    name.into(),
                ))
                .flex()
                .flex_col()
                .gap_2()
                .w(px(SWATCH_WIDTH))
                .p_2()
                .rounded(radius)
                .border_1()
                .border_color(border)
                .test_support()
                .cursor_pointer()
                .tab_index(0)
                .aria_label(format!("{name} {hex}"))
                .on_click(cx.listener(move |this, _, _, cx| this.copy(name, cx)))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter" {
                        this.copy(name, cx);
                    }
                }))
                .child(
                    div()
                        .h(px(40.))
                        .w_full()
                        .rounded(theme.radius_sm())
                        .border_1()
                        .border_color(border)
                        .bg(color),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(mono.clone())
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(name),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .text_color(muted)
                        .font_family(mono.clone())
                        .child(hex)
                        .when(copied == Some(name), |this| {
                            this.child(
                                Tag::new(ElementId::NamedChild(
                                    ElementId::from("copied").into(),
                                    name.into(),
                                ))
                                .label("Copied")
                                .success(),
                            )
                        }),
                )
        };

        let mut sections = Vec::new();
        for group in ColorGroup::ALL {
            let swatches: Vec<_> = REGISTRY
                .iter()
                .filter(|token| token.group == group)
                .map(|token| swatch(token, cx))
                .collect();
            sections.push(
                section(
                    group.title(),
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .w_full()
                        .child(note(group.rule(), cx))
                        .child(div().flex().flex_wrap().gap_3().children(swatches)),
                )
                .into_any_element(),
            );
        }
        let rows: Vec<_> = PAIRS
            .iter()
            .map(|(label, text, surface)| {
                let ratio = contrast_ratio(text(&theme), surface(&theme));
                let grade = grade(ratio);
                div()
                    .id(ElementId::NamedChild(
                        ElementId::from("contrast").into(),
                        (*label).into(),
                    ))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(320.))
                            .text_xs()
                            .font_family(mono.clone())
                            .child(*label),
                    )
                    .child(div().w(px(64.)).child(format!("{ratio:.1}:1")))
                    .child({
                        let tag = Tag::new(ElementId::NamedChild(
                            ElementId::from("grade").into(),
                            (*label).into(),
                        ))
                        .label(grade);
                        match grade {
                            "AAA" => tag.success(),
                            "AA" => tag.secondary(),
                            _ => tag.outline(),
                        }
                    })
            })
            .collect();
        sections.push(
            section(
                "Contrast",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The ratio of each main text color over the surface it sits on, from \
                         the live theme: AAA from 7:1 and AA from 4.5:1.",
                        cx,
                    ))
                    .children(rows),
            )
            .into_any_element(),
        );
        page(sections)
    }
}
