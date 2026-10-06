use std::time::Duration;

use gpui_cn::{
    ActiveTheme as _, InputEvent, InputState, Tag, TooltipExt as _,
    gpui_kit::assets::IconName,
    theme::{ThemeTokens, contrast_ratio, to_hex},
};
use gpui_kit::{
    AnyView, App, AppContext as _, ClipboardItem, Context, ElementId, Entity, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Task, Window, base::TestSupportExt as _, div,
    prelude::FluentBuilder as _, px,
};

use crate::{Story, note, page, search_input, search_state, section};

/// The roles the swatches group by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// One token of the registry: its name, the group it shows in, how to read
/// it from the theme, and the code that reads it, which a swatch copies.
/// The getter and the snippet come from the same tokens, so they cannot
/// drift. Adding a token to the page is adding a row.
#[derive(Clone, Copy)]
pub struct ColorToken {
    /// The name shown on the swatch.
    pub name: &'static str,
    /// The group it shows in.
    pub group: ColorGroup,
    /// The code that reads the color, such as `cx.theme().field_border`.
    pub snippet: &'static str,
    /// Reads the color from the theme.
    pub get: fn(&ThemeTokens) -> Hsla,
}

/// A token read as a field, such as `sidebar` or `base.colors.surface`, or
/// as a method, such as `foreground()`.
macro_rules! token {
    ($group:ident, $name:literal, $($segment:ident).+ ()) => {
        ColorToken {
            name: $name,
            group: ColorGroup::$group,
            snippet: concat!("cx.theme().", stringify!($($segment).+), "()"),
            get: |t| t.$($segment).+(),
        }
    };
    ($group:ident, $name:literal, $($segment:ident).+) => {
        ColorToken {
            name: $name,
            group: ColorGroup::$group,
            snippet: concat!("cx.theme().", stringify!($($segment).+)),
            get: |t| t.$($segment).+,
        }
    };
}

/// Every color the page shows.
pub const REGISTRY: &[ColorToken] = &[
    token!(Surfaces, "background", background()),
    token!(Surfaces, "sidebar", sidebar),
    token!(Surfaces, "field", field),
    token!(Surfaces, "popover", popover),
    token!(Surfaces, "card", base.colors.surface),
    token!(Surfaces, "composer_card", composer_card),
    token!(Surfaces, "status_tab", status_tab),
    token!(Surfaces, "select_trigger", select_trigger),
    token!(Text, "foreground", foreground()),
    token!(Text, "muted_foreground", muted_foreground()),
    token!(Text, "composer_placeholder", composer_placeholder),
    token!(Text, "composer_muted", composer_muted),
    token!(Text, "popover_muted_foreground", popover_muted_foreground),
    token!(Text, "sidebar_muted_foreground", sidebar_muted_foreground),
    token!(Text, "warning_text", warning_text),
    token!(Borders, "border", border()),
    token!(Borders, "input", input()),
    token!(Borders, "field_border", field_border),
    token!(Borders, "field_focus_border", field_focus_border),
    token!(Borders, "select_trigger_border", select_trigger_border),
    token!(Borders, "popover_border", popover_border),
    token!(Borders, "popover_separator", popover_separator),
    token!(Borders, "sidebar_border", sidebar_border),
    token!(Borders, "composer_border", composer_border),
    token!(Borders, "ring", ring()),
    token!(Borders, "focus_ring", focus_ring()),
    token!(Accent, "control_accent", control_accent),
    token!(Accent, "primary", primary()),
    token!(Accent, "primary_foreground", primary_foreground()),
    token!(Accent, "accent", accent()),
    token!(Accent, "link", link),
    token!(Accent, "selection", selection()),
    token!(Rows, "selected", selected),
    token!(Rows, "sidebar_accent", sidebar_accent),
    token!(Rows, "sidebar_selected", sidebar_selected),
    token!(Rows, "popover_hover", popover_hover),
    token!(Rows, "popover_accent", popover_accent),
    token!(Status, "destructive", destructive()),
    token!(Status, "success", success),
    token!(Status, "warning", warning),
    token!(Status, "info", info),
    token!(Status, "destructive_tint", destructive_tint),
    token!(Status, "success_tint", success_tint),
    token!(Status, "warning_tint", warning_tint),
    token!(Status, "info_tint", info_tint),
    token!(Status, "destructive_solid", destructive_solid),
    token!(Status, "success_solid", success_solid),
    token!(Status, "warning_solid", warning_solid),
    token!(Status, "info_solid", info_solid),
];

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

/// Whether `token` matches the search `query`. A query that starts with `#`
/// or is only hex digits also matches the token's hex by substring. Names
/// and group titles match fuzzily, so "surface" shows the whole group and
/// "focus" the tokens with focus in their names. An empty query matches
/// everything.
pub fn color_matches(token: &ColorToken, hex: &str, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let by_name = !gpui_cn::fuzzy::rank(query, &[token.name], |name| name).is_empty();
    let by_group = !gpui_cn::fuzzy::rank(query, &[token.group.title()], |title| title).is_empty();
    let digits = query.trim_start_matches('#').to_lowercase();
    let hex_query = query.starts_with('#') || query.chars().all(|c| c.is_ascii_hexdigit());
    let by_hex = hex_query && !digits.is_empty() && hex.trim_start_matches('#').contains(&digits);
    by_name || by_group || by_hex
}

/// The theme's colors, read live, by role.
pub struct ColorStory {
    copied: Option<&'static str>,
    reset: Option<Task<()>>,
    search: Entity<InputState>,
    handles: Vec<(&'static str, FocusHandle)>,
}

impl ColorStory {
    /// The tokens the search shows, in page order.
    pub fn visible(&self, cx: &App) -> Vec<&'static ColorToken> {
        let query = self.search.read(cx).value();
        let theme = cx.theme();
        let mut shown = Vec::new();
        for group in ColorGroup::ALL {
            shown.extend(REGISTRY.iter().filter(|token| {
                token.group == group && color_matches(token, &to_hex((token.get)(theme)), &query)
            }));
        }
        shown
    }

    /// The search field's state.
    pub fn search(&self) -> &Entity<InputState> {
        &self.search
    }

    /// The name of the swatch that was just copied.
    pub fn copied(&self) -> Option<&'static str> {
        self.copied
    }

    /// Copies the code that reads the token `name`, such as
    /// `cx.theme().field_border`, and shows "Copied" on its swatch for a
    /// moment. The library asks for tokens, never for literal colors, so a
    /// hex is never what is copied.
    pub fn copy(&mut self, name: &'static str, cx: &mut Context<Self>) {
        let Some(token) = REGISTRY.iter().find(|token| token.name == name) else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(token.snippet.to_string()));
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
         text pairs. Click a swatch to copy the code that reads its token."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        let search = search_state("Search colors", window, cx);
        cx.new(|cx| {
            cx.subscribe_in(
                &search,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => cx.notify(),
                    InputEvent::PressEnter { .. } => {
                        if let Some(first) = this.visible(cx).first()
                            && let Some((_, handle)) =
                                this.handles.iter().find(|(name, _)| *name == first.name)
                        {
                            handle.focus(window, cx);
                        }
                    }
                    _ => {}
                },
            )
            .detach();
            Self {
                copied: None,
                reset: None,
                search: search.clone(),
                handles: REGISTRY
                    .iter()
                    .map(|token| (token.name, cx.focus_handle()))
                    .collect(),
            }
        })
        .into()
    }
}

/// The width of one swatch card.
const SWATCH_WIDTH: f32 = 168.;

impl Render for ColorStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mono = theme.mono_font_family().clone();
        let (border, muted, radius) = (theme.border(), theme.muted_foreground(), theme.radius_md());
        let copied = self.copied;
        let visible = self.visible(cx);
        let query = self.search.read(cx).value();

        let swatch =
            |token: &ColorToken, handle: FocusHandle, window: &Window, cx: &mut Context<Self>| {
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
                    .track_focus(&handle)
                    .tab_index(0)
                    .aria_label(format!("{name} {hex}, press to copy token"))
                    .managed_tooltip(format!("Copy {}", token.snippet), window, &*cx)
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
                                    .label(format!("Copied {}", token.snippet))
                                    .success(),
                                )
                            }),
                    )
            };

        let mut sections = Vec::new();
        sections.push(
            div()
                .w_full()
                .max_w(px(420.))
                .child(search_input(&self.search, "color-search", "Search colors"))
                .into_any_element(),
        );
        for group in ColorGroup::ALL {
            let swatches: Vec<_> = visible
                .iter()
                .filter(|token| token.group == group)
                .map(|token| {
                    let handle = self
                        .handles
                        .iter()
                        .find(|(name, _)| *name == token.name)
                        .map(|(_, handle)| handle.clone())
                        .expect("every token has a focus handle");
                    swatch(token, handle, window, cx)
                })
                .collect();
            if swatches.is_empty() {
                continue;
            }
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
            .filter(|(label, _, _)| {
                query.trim().is_empty()
                    || !gpui_cn::fuzzy::rank(&query, &[*label], |label| label).is_empty()
            })
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
        let empty = visible.is_empty() && rows.is_empty();
        if !rows.is_empty() {
            sections.push(
                section(
                    "Contrast",
                    div()
                        .id("color-contrast")
                        .test_support()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(note(
                            "The ratio of each main text color over the surface it sits on, \
                             from the live theme: AAA from 7:1 and AA from 4.5:1.",
                            cx,
                        ))
                        .children(rows),
                )
                .into_any_element(),
            );
        }
        if empty {
            sections.push(
                div()
                    .id("color-empty")
                    .test_support()
                    .w_full()
                    .py_8()
                    .flex()
                    .justify_center()
                    .text_color(muted)
                    .child("No colors match")
                    .into_any_element(),
            );
        }
        page(sections)
    }
}
