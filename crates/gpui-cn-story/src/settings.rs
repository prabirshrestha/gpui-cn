//! The settings page: pushed over the components page, with its own
//! sidebar ("Back to app" and the sections) and cards of rows.

use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, NavMotion, NavStackState, ReduceMotion, ScrollArea,
    Sidebar, SidebarCollapsible, SidebarGroup, SidebarMenuButton, SidebarState, Switch, Theme,
    ThemeMode, ThemeModePicker, TitleBar, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, base::Selectable as _, div, prelude::FluentBuilder as _, px,
};

use crate::{
    PAGE_WIDTH, UI_FONT_SIZE_RANGE, segmented, segmented_from, segmented_with, sidebar_title_bar,
};

/// One page of settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    /// Pointer cursors and text size.
    General,
    /// Theme and motion.
    Appearance,
}

impl SettingsSection {
    const ALL: [Self; 2] = [Self::General, Self::Appearance];

    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::General => IconName::Settings,
            Self::Appearance => IconName::Palette,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::General => "section-general",
            Self::Appearance => "section-appearance",
        }
    }
}

/// The settings page.
pub struct SettingsPage {
    sidebar: Entity<SidebarState>,
    stack: Entity<NavStackState>,
    section: SettingsSection,
}

impl SettingsPage {
    /// A settings page sharing the gallery's sidebar and stack.
    pub fn new(
        sidebar: &Entity<SidebarState>,
        stack: &Entity<NavStackState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(sidebar, |_, _, cx| cx.notify()).detach();
        cx.observe(stack, |_, _, cx| cx.notify()).detach();
        Self {
            sidebar: sidebar.clone(),
            stack: stack.clone(),
            section: SettingsSection::General,
        }
    }

    /// Shows `section`.
    pub fn show(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    /// The settings sidebar: back to the app, and the sections. The
    /// gallery draws it in its one sidebar while this page shows.
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> Sidebar {
        let stack = self.stack.clone();
        Sidebar::new()
            .header(sidebar_title_bar())
            .child(
                SidebarMenuButton::new("back-to-app")
                    .icon(IconName::ArrowLeft)
                    .label("Back to app")
                    .on_click(move |_, _, cx| {
                        stack.update(cx, |stack, cx| {
                            stack.pop(NavMotion::Animated, cx);
                        });
                    }),
            )
            .child(SidebarGroup::new().label("Preferences").children(
                SettingsSection::ALL.into_iter().map(|section| {
                    SidebarMenuButton::new(section.id())
                        .icon(section.icon())
                        .label(section.title())
                        .selected(section == self.section)
                        .on_click(cx.listener(move |this, _, _, cx| this.show(section, cx)))
                }),
            ))
    }

    fn render_general(&self, cx: &App) -> Vec<AnyElement> {
        let settings = Theme::global(cx);
        let pointer_cursors = settings.pointer_cursors;
        let font_size = settings.ui_font_size;
        let collapsible = self.sidebar.read(cx).collapsible();
        let sidebar = self.sidebar.clone();
        vec![
            card(
                cx,
                [
                    row(
                        "Sidebar collapse",
                        "What hiding the sidebar does: slide it away, or keep a rail of icons.",
                        segmented_with(
                            "collapsible",
                            [
                                ("Offcanvas", collapsible == SidebarCollapsible::Offcanvas),
                                ("Icon", collapsible == SidebarCollapsible::Icon),
                            ],
                            move |ix, _, cx| {
                                let mode =
                                    [SidebarCollapsible::Offcanvas, SidebarCollapsible::Icon][ix];
                                sidebar.update(cx, |state, cx| state.set_collapsible(mode, cx));
                            },
                        ),
                    ),
                    row(
                        "Pointer cursors",
                        "Show the hand cursor over buttons. Links always use it.",
                        Switch::new("pointer")
                            .checked(pointer_cursors)
                            .accessibility_label("Pointer cursors")
                            .on_change(|checked, _, cx| {
                                let checked = *checked;
                                Theme::update(cx, |theme| theme.pointer_cursors = checked);
                            }),
                    ),
                    row(
                        "Text size",
                        "The interface font size. Everything on the rem scale follows it.",
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("zoom-out")
                                    .size(ButtonSize::Sm)
                                    .icon(IconName::Minus)
                                    .accessibility_label("Smaller text")
                                    .tooltip("Smaller text")
                                    .on_click(move |_, _, cx| {
                                        Theme::set_ui_font_size(
                                            cx,
                                            (font_size - px(1.)).max(UI_FONT_SIZE_RANGE.start),
                                        );
                                    }),
                            )
                            .child(
                                div()
                                    .w_8()
                                    .text_center()
                                    .text_size(cx.theme().text_control.size)
                                    .child(SharedString::from(format!("{}", f32::from(font_size)))),
                            )
                            .child(
                                Button::new("zoom-in")
                                    .size(ButtonSize::Sm)
                                    .icon(IconName::Plus)
                                    .accessibility_label("Larger text")
                                    .tooltip("Larger text")
                                    .on_click(move |_, _, cx| {
                                        Theme::set_ui_font_size(
                                            cx,
                                            (font_size + px(1.)).min(UI_FONT_SIZE_RANGE.end),
                                        );
                                    }),
                            ),
                    ),
                ],
            ),
            heading("Defaults", cx).into_any_element(),
            card(
                cx,
                [row(
                    "Reset preferences",
                    "Back to the system appearance and motion, no pointer cursors, and 16px text.",
                    Button::new("reset")
                        .size(ButtonSize::Sm)
                        .label("Reset")
                        .on_click(|_, _, cx| {
                            Theme::update(cx, |theme| {
                                theme.mode = ThemeMode::System;
                                theme.reduce_motion = ReduceMotion::System;
                                theme.pointer_cursors = false;
                                theme.ui_font_size = px(16.);
                                theme.code_font_size = px(13.);
                            });
                        }),
                )],
            ),
        ]
    }

    fn render_appearance(&self, cx: &App) -> Vec<AnyElement> {
        let settings = Theme::global(cx);
        let mode = settings.mode;
        let reduce_motion = settings.reduce_motion;
        let bundled = gpui_cn::theme::fonts::bundled_code_fonts();
        let code_font = settings.active_config().fonts.code.clone();
        let mut fonts: Vec<(SharedString, bool)> = bundled
            .iter()
            .map(|font| (font.clone(), code_font.as_ref() == Some(font)))
            .collect();
        fonts.push((
            "System".into(),
            code_font.is_none() || !bundled.contains(code_font.as_ref().unwrap()),
        ));
        vec![
            heading("Theme", cx).pt_0().into_any_element(),
            ThemeModePicker::new("mode")
                .value(mode)
                .on_change(|mode, _, cx| Theme::change(*mode, cx))
                .into_any_element(),
            card(
                cx,
                [
                    row(
                        "Code font",
                        "The fonts that ship with the gallery, or the platform's monospace.",
                        segmented_from("code-font", fonts, move |ix, _, cx| {
                            let font = bundled.get(ix).cloned();
                            Theme::update(cx, |theme| {
                                theme.light.fonts.code = font.clone();
                                theme.dark.fonts.code = font;
                            });
                        }),
                    ),
                    row(
                        "Reduce motion",
                        "Skip transitions. System follows the accessibility setting.",
                        segmented(
                            "motion",
                            [
                                ("System", reduce_motion == ReduceMotion::System),
                                ("On", reduce_motion == ReduceMotion::On),
                                ("Off", reduce_motion == ReduceMotion::Off),
                            ],
                            |ix, cx| {
                                let value =
                                    [ReduceMotion::System, ReduceMotion::On, ReduceMotion::Off][ix];
                                Theme::update(cx, |theme| theme.reduce_motion = value);
                            },
                        ),
                    ),
                ],
            ),
        ]
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (open, icon_only) = {
            let state = self.sidebar.read(cx);
            (state.is_open(), state.is_icon_only())
        };
        let section = self.section;
        let title = cx.theme().text_title;
        let content = match section {
            SettingsSection::General => self.render_general(cx),
            SettingsSection::Appearance => self.render_appearance(cx),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(TitleBar::new().inset(!open && !icon_only))
            .child(
                ScrollArea::new("settings-scroll")
                    .flex()
                    .flex_col()
                    .items_center()
                    .flex_1()
                    .min_h_0()
                    .px_8()
                    .pb_10()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .max_w(PAGE_WIDTH)
                            .gap_4()
                            .child(
                                div()
                                    .pt_8()
                                    .pb_4()
                                    .text_size(title.size)
                                    .line_height(title.line_height)
                                    .font_weight(title.weight)
                                    .child(section.title()),
                            )
                            .children(content),
                    ),
            )
    }
}

/// A group heading between cards.
fn heading(text: &'static str, cx: &App) -> Div {
    let heading = cx.theme().text_heading;
    div()
        .pt_6()
        .text_size(heading.size)
        .line_height(heading.line_height)
        .font_weight(heading.weight)
        .child(text)
}

/// A card of rows on the elevated surface, hairlines between the rows.
fn card<const N: usize>(cx: &App, rows: [Row; N]) -> AnyElement {
    let theme = cx.theme();
    let border = theme.border();
    let control = theme.text_control.size;
    let caption = theme.base.typography.xs.size;
    div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(theme.radius_xl())
        .border_1()
        .border_color(border)
        .bg(theme.base.colors.surface)
        .px_4()
        .children(rows.into_iter().enumerate().map(move |(ix, row)| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_6()
                .py_3p5()
                .when(ix > 0, |this| this.border_t_1().border_color(border))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .min_w_0()
                        .child(div().text_size(control).child(row.title))
                        .child(
                            div()
                                .text_size(caption)
                                .text_color(theme.muted_foreground())
                                .child(row.description),
                        ),
                )
                .child(div().flex_shrink_0().child(row.control))
        }))
        .into_any_element()
}

struct Row {
    title: &'static str,
    description: &'static str,
    control: AnyElement,
}

fn row(title: &'static str, description: &'static str, control: impl IntoElement) -> Row {
    Row {
        title,
        description,
        control: control.into_any_element(),
    }
}
