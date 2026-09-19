//! The gpui-cn gallery shell: a list of stories, one shown at a time, with
//! theme controls in the toolbar. Built from gpui-cn components only, so the
//! gallery is also the first consumer of the library.

pub mod stories;

use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, ReduceMotion, Theme, ThemeMode,
    gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    base::{Selectable as _, StyledExt as _},
    div, px,
};

/// One page of the gallery.
pub trait Story: 'static {
    /// The name in the story list.
    fn title() -> &'static str;
    /// One sentence under the title.
    fn description() -> &'static str;
    /// Builds the page.
    fn view(window: &mut Window, cx: &mut App) -> AnyView;
}

/// A registered story.
#[derive(Clone)]
pub struct StoryEntry {
    title: &'static str,
    description: &'static str,
    build: fn(&mut Window, &mut App) -> AnyView,
}

impl StoryEntry {
    fn of<S: Story>() -> Self {
        Self {
            title: S::title(),
            description: S::description(),
            build: S::view,
        }
    }
}

/// Every story, in display order.
pub fn stories() -> Vec<StoryEntry> {
    vec![StoryEntry::of::<stories::ButtonStory>()]
}

/// The gallery window content.
pub struct Gallery {
    entries: Vec<StoryEntry>,
    selected: usize,
    page: AnyView,
}

impl Gallery {
    /// A gallery showing the first story.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let entries = stories();
        let page = (entries[0].build)(window, cx);
        Self {
            entries,
            selected: 0,
            page,
        }
    }

    fn select(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix == self.selected || ix >= self.entries.len() {
            return;
        }
        self.selected = ix;
        self.page = (self.entries[ix].build)(window, cx);
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .w_56()
            .h_full()
            .flex_shrink_0()
            .p_2()
            .gap_0p5()
            .border_r_1()
            .border_color(theme.border())
            .child(
                div()
                    .px_2()
                    .py_1p5()
                    .text_xs()
                    .font_medium()
                    .text_color(theme.muted_foreground())
                    .child("Components"),
            )
            .children(self.entries.iter().enumerate().map(|(ix, entry)| {
                Button::new(ElementId::from(("story", ix)))
                    .ghost()
                    .size(ButtonSize::Sm)
                    .w_full()
                    .justify_start()
                    .label(entry.title)
                    .selected(ix == self.selected)
                    .on_click(cx.listener(move |this, _, window, cx| this.select(ix, window, cx)))
            }))
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = Theme::global(cx);
        let mode = settings.mode;
        let reduce_motion = settings.reduce_motion;
        let pointer_cursors = settings.pointer_cursors;
        let font_size = settings.ui_font_size;
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .px_6()
            .py_3()
            .gap_3()
            .border_b_1()
            .border_color(theme.border())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_lg()
                            .font_medium()
                            .child(self.entries[self.selected].title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground())
                            .child(self.entries[self.selected].description),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_x_5()
                    .gap_y_2()
                    .child(labeled(
                        "Appearance",
                        segmented(
                            "mode",
                            [
                                ("System", mode == ThemeMode::System),
                                ("Light", mode == ThemeMode::Light),
                                ("Dark", mode == ThemeMode::Dark),
                            ],
                            |ix, cx| {
                                let mode =
                                    [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark][ix];
                                Theme::change(mode, cx);
                            },
                        ),
                    ))
                    .child(labeled(
                        "Reduce motion",
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
                    ))
                    .child(labeled(
                        "Pointer cursors",
                        segmented(
                            "pointer",
                            [("On", pointer_cursors), ("Off", !pointer_cursors)],
                            |ix, cx| {
                                Theme::update(cx, |theme| theme.pointer_cursors = ix == 0);
                            },
                        ),
                    ))
                    .child(labeled(
                        "UI font size",
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
                                            (font_size - px(1.)).max(px(10.)),
                                        );
                                    }),
                            )
                            .child(
                                div()
                                    .w_8()
                                    .text_center()
                                    .text_sm()
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
                                            (font_size + px(1.)).min(px(28.)),
                                        );
                                    }),
                            ),
                    ))
                    .child(
                        Button::new("reset-appearance")
                            .ghost()
                            .size(ButtonSize::Sm)
                            .label("Reset")
                            .tooltip("Back to the system appearance, motion, and text size")
                            .on_click(|_, _, cx| {
                                Theme::update(cx, |theme| {
                                    theme.mode = ThemeMode::System;
                                    theme.reduce_motion = ReduceMotion::System;
                                    theme.pointer_cursors = false;
                                    theme.ui_font_size = px(16.);
                                    theme.code_font_size = px(13.);
                                });
                            }),
                    ),
            )
    }
}

/// A row of pill buttons where the selected one carries the soft fill.
fn segmented<const N: usize>(
    id: &'static str,
    options: [(&'static str, bool); N],
    on_select: fn(usize, &mut App),
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .children(
            options
                .into_iter()
                .enumerate()
                .map(move |(ix, (label, selected))| {
                    Button::new(ElementId::from((id, ix)))
                        .ghost()
                        .size(ButtonSize::Sm)
                        .rounded_full()
                        .label(label)
                        .toggled(selected)
                        .selected(selected)
                        .on_click(move |_, _, cx| on_select(ix, cx))
                }),
        )
}

fn labeled(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(div().text_sm().child(label))
        .child(control)
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_stretch()
            .size_full()
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_toolbar(cx))
                    .child(
                        div()
                            .id("page")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_6()
                            .child(self.page.clone()),
                    ),
            )
    }
}

/// A titled block inside a story.
pub fn section(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_sm().font_medium().child(title))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(content),
        )
}

/// A story page: sections stacked with room between them.
pub fn page(sections: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    div().flex().flex_col().gap_8().children(sections)
}
