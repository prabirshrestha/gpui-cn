//! The open menu of a select: the panel, the search field, the rows in a
//! virtual list, and what stands in for them while empty or loading.

use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, ElementId, Entity, FocusHandle, Hsla, InteractiveElement as _, IntoElement,
    Length, ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window,
    assets::IconName,
    base::{self, TestSupportExt as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px, rems,
};

use super::{
    item::{SelectEntry, SelectItem, SelectValue},
    state::SelectState,
};
use crate::{Icon, ScrollArea, ThemeTokens};

pub(super) type ItemRenderer<V> =
    Rc<dyn Fn(&SelectItem<V>, SelectRow, &mut Window, &mut App) -> AnyElement>;
pub(super) type LabelRenderer = Rc<dyn Fn(&SharedString, &mut Window, &mut App) -> AnyElement>;
pub(super) type PartRenderer = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// The state of a row a custom item renderer draws; see
/// [`Select::render_item`](super::Select::render_item).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SelectRow {
    highlighted: bool,
    selected: bool,
    disabled: bool,
}

impl SelectRow {
    /// Whether the keyboard or the pointer is on the row.
    pub fn is_highlighted(&self) -> bool {
        self.highlighted
    }

    /// Whether the row's item is selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Whether the row's item is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

/// Everything the menu reads from the theme, in one borrow.
#[derive(Clone)]
pub(super) struct MenuLook {
    pub surface: Hsla,
    pub border: Hsla,
    pub accent: Hsla,
    pub separator: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    pub description: Hsla,
    pub indicator: Hsla,
    pub shadow: Vec<gpui_kit::BoxShadow>,
    pub radius: Pixels,
    pub row_radius: Pixels,
    pub row_height: Pixels,
    pub row_padding: Pixels,
    pub search_height: Pixels,
    /// The menu's inset around its rows, and a separator's margin: one
    /// step of the spacing scale (`p_1`), so it follows the rem size.
    pub padding: Pixels,
    pub text_size: Pixels,
    pub line_height: Pixels,
    pub min_width: Pixels,
    pub search_min_width: Pixels,
    pub max_width: Pixels,
    pub max_height: Pixels,
    pub gap: Pixels,
    /// How far above its resting place the menu starts: two steps of the
    /// spacing scale, shadcn's `slide-in-from-top-2`.
    pub enter_offset: Pixels,
}

impl MenuLook {
    pub fn of(theme: &ThemeTokens, rem_size: Pixels) -> Self {
        let metrics = &theme.metrics;
        Self {
            surface: theme.popover,
            border: theme.popover_border,
            accent: theme.popover_accent,
            separator: theme.popover_separator,
            foreground: theme.popover_foreground,
            muted_foreground: theme.muted_foreground(),
            description: theme.popover_muted_foreground,
            indicator: theme.select_indicator,
            shadow: theme.base.shadow.md.clone(),
            radius: theme.radius_xl(),
            row_radius: theme.radius_sm(),
            row_height: metrics.row_sm,
            row_padding: metrics.control_padding_md,
            search_height: metrics.control_md,
            padding: rems(0.25).to_pixels(rem_size),
            text_size: theme.text_control.size,
            line_height: theme.text_control.line_height,
            min_width: metrics.menu_min_width,
            search_min_width: metrics.menu_search_min_width,
            max_width: metrics.menu_max_width,
            max_height: metrics.menu_max_height,
            gap: metrics.menu_gap,
            enter_offset: -rems(0.5).to_pixels(rem_size),
        }
    }
}

impl MenuLook {
    /// The height the rows can take: the menu's cap less its padding
    /// and border, and less the search field and the separator under it
    /// when the menu has one.
    fn rows_max_height(&self, has_search: bool) -> Pixels {
        let border = px(2.);
        let search = if has_search {
            self.search_height + self.padding * 2. + px(1.)
        } else {
            px(0.)
        };
        self.max_height - self.padding * 2. - border - search
    }
}

/// Where the menu is in its entrance: how much of it shows, how much of
/// its shadow, and how far it still has to slide.
#[derive(Clone, Copy)]
pub(super) struct MenuMotion {
    pub opacity: f32,
    pub shadow_strength: f32,
    pub offset: Pixels,
}

/// What the rows of one menu are drawn from, owned by the list's row
/// builder, which outlives the render that made it.
pub(super) struct Rows<V: SelectValue> {
    pub id: ElementId,
    pub state: Entity<SelectState<V>>,
    pub look: MenuLook,
    pub render_item: Option<ItemRenderer<V>>,
    pub render_label: Option<LabelRenderer>,
    pub pointer_cursors: bool,
}

/// The menu: the search field, the rows in a scroll region, or the empty
/// or loading element.
pub(super) struct Menu<V: SelectValue> {
    pub rows: Rows<V>,
    pub content_focus: FocusHandle,
    pub width: Option<Length>,
    pub trigger_width: Pixels,
    pub empty_text: SharedString,
    pub render_empty: Option<PartRenderer>,
    pub render_loading: Option<PartRenderer>,
    pub motion: MenuMotion,
}

impl<V: SelectValue> Menu<V> {
    pub fn render(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let Menu {
            rows,
            content_focus,
            width,
            trigger_width,
            empty_text,
            render_empty,
            render_loading,
            motion,
        } = self;
        let look = rows.look.clone();
        let look = &look;
        let id = rows.id.clone();
        let state = rows.state.clone();
        let (list, row_count, row_hint, searching, search) = {
            let state = state.read(cx);
            (
                state.list().clone(),
                state.row_count(),
                state.row_hint(cx),
                state.is_searching(),
                state.search_input().cloned(),
            )
        };
        let menu_id = ElementId::NamedChild(id.clone().into(), "menu".into());
        let rows_id = ElementId::NamedChild(id.clone().into(), "rows".into());
        let rows_max_height = look.rows_max_height(search.is_some());
        // While a handler is still answering, the loading element stands
        // in for the rows, so what shows is never the answer to an older
        // query beside a promise of the new one. Without one the last rows
        // stay until the answer, and an empty menu shows nothing rather
        // than "No results" it may take back.
        let loading = searching
            .then(|| render_loading.as_ref().map(|render| render(window, cx)))
            .flatten();
        let body = if let Some(loading) = loading {
            loading
        } else if row_count == 0 {
            match (searching, render_empty) {
                (true, _) => div().into_any_element(),
                (false, Some(render)) => render(window, cx),
                (false, None) => div()
                    .py_6()
                    .text_center()
                    .text_color(look.muted_foreground)
                    .child(empty_text)
                    .into_any_element(),
            }
        } else {
            ScrollArea::list(rows_id, &list, row_hint, move |row, window, cx| {
                rows.render(row, window, cx)
            })
            .max_h(rows_max_height)
            .into_any_element()
        };
        v_flex()
            .id(menu_id)
            .test_support()
            .track_focus(&content_focus)
            .tab_group()
            .map(|this| match width {
                Some(width) => this.w(width),
                None => {
                    let min_width = if search.is_some() {
                        look.search_min_width
                    } else {
                        look.min_width
                    };
                    this.min_w(trigger_width.max(min_width))
                        .max_w(look.max_width)
                }
            })
            // The fade sits on the panel itself, since GPUI shapes the
            // shadow from the element that carries it.
            .relative()
            .top(motion.offset)
            .opacity(motion.opacity)
            .p(look.padding)
            .rounded(look.radius)
            .bg(look.surface)
            .border_1()
            .border_color(look.border)
            .shadow(
                look.shadow
                    .iter()
                    .map(|shadow| gpui_kit::BoxShadow {
                        color: shadow.color.opacity(motion.shadow_strength),
                        ..*shadow
                    })
                    .collect(),
            )
            .text_size(look.text_size)
            .line_height(look.line_height)
            .text_color(look.foreground)
            .on_mouse_down_out({
                let state = state.clone();
                move |_, window, cx| state.update(cx, |state, cx| state.close(window, cx))
            })
            .when_some(search, |this, search| {
                this.child(
                    h_flex()
                        .id(ElementId::NamedChild(id.clone().into(), "search".into()))
                        .test_support()
                        .flex_shrink_0()
                        .items_center()
                        .gap_2()
                        .h(look.search_height)
                        .px(look.row_padding)
                        .child(
                            Icon::from(IconName::Search)
                                .size_4()
                                .text_color(look.muted_foreground),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(base::input::Input::new(&search)),
                        ),
                )
                .child(separator(look))
            })
            .child(body)
            .into_any_element()
    }
}

/// A hairline between two groups, inset from the menu's edge.
fn separator(look: &MenuLook) -> AnyElement {
    div()
        .w_full()
        .my(look.padding)
        .px(look.padding * 2.)
        .child(div().h_px().w_full().bg(look.separator))
        .into_any_element()
}

impl<V: SelectValue> Rows<V> {
    fn render(&self, row: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let Rows {
            id,
            state,
            look,
            render_item,
            render_label,
            pointer_cursors,
        } = self;
        let pointer_cursors = *pointer_cursors;
        let (entry, highlighted, selected, row_height) = {
            let state = state.read(cx);
            let Some(entry) = state.entry_at(row) else {
                return div().into_any_element();
            };
            let selected = entry
                .item()
                .is_some_and(|item| state.is_selected(item.value()));
            (
                entry.clone(),
                state.is_highlighted(row),
                selected,
                state.row_height(),
            )
        };
        match entry {
            SelectEntry::Separator => separator(look),
            SelectEntry::Label(text) => match render_label {
                Some(render) => div()
                    .w_full()
                    .child(render(&text, window, cx))
                    .into_any_element(),
                None => div()
                    .w_full()
                    .h(look.row_height)
                    .px(look.row_padding)
                    .flex()
                    .items_center()
                    .text_color(look.muted_foreground)
                    .child(text)
                    .into_any_element(),
            },
            SelectEntry::Item(item) => {
                let disabled = item.is_disabled();
                let value = item.value().clone();
                let choose_state = state.clone();
                let hover_state = state.clone();
                let content: Vec<AnyElement> = match render_item {
                    Some(render) => vec![render(
                        &item,
                        SelectRow {
                            highlighted,
                            selected,
                            disabled,
                        },
                        window,
                        cx,
                    )],
                    None => default_row_content(&item, selected, look, window, cx),
                };
                h_flex()
                    .id(ElementId::NamedChild(id.clone().into(), item.key()))
                    .test_support()
                    .w_full()
                    .items_center()
                    .gap_2()
                    // A fixed height is what a scroll to a far row counts
                    // on before the row is laid out.
                    .map(|this| match row_height {
                        Some(height) => this.h(height),
                        None => this.min_h(look.row_height),
                    })
                    .px(look.row_padding)
                    .py_1p5()
                    .rounded(look.row_radius)
                    .when(highlighted && !disabled, |this| this.bg(look.accent))
                    .text_color(if disabled {
                        look.muted_foreground
                    } else {
                        look.foreground
                    })
                    .map(|this| {
                        if !disabled && pointer_cursors {
                            this.cursor_pointer()
                        } else {
                            this.cursor_default()
                        }
                    })
                    .children(content)
                    // A pointer that moves over a row highlights it; a row that
                    // scrolls under a resting pointer does not, so the
                    // keyboard's highlight is not snatched back.
                    .on_mouse_move(move |_, _, cx| {
                        hover_state.update(cx, |state, cx| state.highlight_row(row, cx));
                    })
                    .on_click(move |_, window, cx| {
                        choose_state.update(cx, |state, cx| state.choose(&value, window, cx));
                    })
                    .into_any_element()
            }
        }
    }
}

/// The inside of an item row as the reference app draws it: the leading
/// element, the label over its description, and the check.
fn default_row_content<V: SelectValue>(
    item: &SelectItem<V>,
    selected: bool,
    look: &MenuLook,
    window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    let disabled = item.is_disabled();
    let mut content = Vec::with_capacity(3);
    if let Some(leading) = item.render_leading(window, cx) {
        content.push(div().flex_shrink_0().child(leading).into_any_element());
    }
    content.push(
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_1p5()
            .child(
                div()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(item.label().clone()),
            )
            .when_some(item.description_text().cloned(), |this, text| {
                this.child(div().text_color(look.description).child(text))
            })
            .into_any_element(),
    );
    content.push(
        div()
            .flex_shrink_0()
            .size_4()
            .when(selected, |this| {
                this.child(
                    Icon::from(IconName::Check)
                        .size_4()
                        .text_color(if disabled {
                            look.muted_foreground
                        } else {
                            look.indicator
                        }),
                )
            })
            .into_any_element(),
    );
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ThemeConfig, to_hex};
    use gpui_kit::base::ThemeAppearance;

    #[test]
    fn the_menu_reads_the_reference_values() {
        let theme = crate::theme::test_tokens(&ThemeConfig::dark(), ThemeAppearance::Dark);
        let menu = MenuLook::of(&theme, px(16.));
        assert_eq!(to_hex(menu.surface), "#2d2d2d");
        assert_eq!(to_hex(menu.accent), "#3d3d3d");
        assert_eq!(to_hex(menu.indicator), "#cacaca");
        assert_eq!(menu.row_height, px(28.));
        assert_eq!(menu.radius, px(14.));
        assert_eq!(menu.row_radius, px(6.));
        assert_eq!(menu.padding, px(4.));
        assert_eq!(menu.enter_offset, px(-8.));
        assert_eq!(menu.rows_max_height(false), px(380.));
        assert_eq!(menu.rows_max_height(true), px(343.));
    }
}
