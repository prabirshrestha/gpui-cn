//! The parts of the two pickers' dialogs that are the same: the fixed list
//! box, its messages, a row's frame, and the "Load more" row.

use gpui_kit::{
    AnyElement, ElementId, Entity, FontWeight, HighlightStyle, Hsla, InteractiveElement as _,
    IntoElement, ParentElement, Pixels, Role, SharedString, StatefulInteractiveElement, Styled,
    StyledText, TestSupportExt as _,
    base::{h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
};

use super::{Entry, Host, MoreState};
use crate::{Spinner, menu::MenuLook};

pub(crate) const EMPTY_FOLDERS: &str = "No folders found in this directory.";
pub(crate) const EMPTY_FILES: &str = "Nothing to show in this directory.";
pub(crate) const FAILED: &str = "Unable to load this folder";
pub(crate) const NO_MATCH: &str = "No folders match.";
pub(crate) const NO_FILE_MATCH: &str = "No matches.";
const LOAD_MORE: &str = "Load more";
const LOADING_MORE: &str = "Loading more";
const MORE_FAILED: &str = "Unable to load more. Try again.";

/// A message centered in the list box.
pub(crate) fn message(id: ElementId, look: &MenuLook, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .test_support()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(look.muted_foreground)
        .child(text)
        .into_any_element()
}

/// A spinner centered in the list box.
pub(crate) fn loading(id: ElementId, look: &MenuLook) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(look.muted_foreground)
        .child(Spinner::new(id))
        .into_any_element()
}

/// The bordered box that holds the list, as tall as the theme's list
/// height in every state, so the dialog never changes size.
pub(crate) fn list_box(
    id: ElementId,
    look: &MenuLook,
    list_height: Pixels,
    body: AnyElement,
) -> AnyElement {
    div()
        .id(id)
        .test_support()
        .h(list_height + look.padding * 2. + gpui_kit::px(2.))
        .p(look.padding)
        .rounded(look.radius)
        .border_1()
        .border_color(look.border)
        .overflow_hidden()
        .child(body)
        .into_any_element()
}

/// The row that chooses the current folder. Its slot is always there, so
/// the dialog is as tall while a folder loads or fails as when it is
/// listed.
pub(crate) fn slot(look: &MenuLook, content: Option<AnyElement>) -> AnyElement {
    h_flex()
        .h(look.row_height)
        .flex_shrink_0()
        .children(content)
        .into_any_element()
}

/// A name with the characters the query matched in the link color.
pub(crate) fn name_label(
    name: SharedString,
    ranges: Vec<std::ops::Range<usize>>,
    link: Hsla,
) -> StyledText {
    StyledText::new(name).with_highlights(ranges.into_iter().map(|range| {
        (
            range,
            HighlightStyle {
                color: Some(link),
                font_weight: Some(FontWeight::SEMIBOLD),
                ..Default::default()
            },
        )
    }))
}

/// The frame of a row: its height and padding, the highlight, the cursor.
pub(crate) fn row_frame(
    id: ElementId,
    look: &MenuLook,
    role: Role,
    highlighted: bool,
    selected: bool,
    pointer_cursors: bool,
) -> impl ParentElement + Styled + StatefulInteractiveElement + IntoElement {
    crate::menu::row_frame(look, highlighted || selected)
        .id(id)
        .test_support()
        .role(role)
        .h(look.row_height)
        .when(highlighted, |this| this.aria_active_descendant())
        .when(pointer_cursors, |this| this.cursor_pointer())
}

/// The last row while more entries follow: it loads them on a click, and
/// asks again after a failure.
pub(crate) fn more_row<E: Entry, O: Host<E>>(
    id: &ElementId,
    look: &MenuLook,
    state: &Entity<O>,
    more: MoreState,
    highlighted: bool,
    row: usize,
    pointer_cursors: bool,
) -> AnyElement {
    let click = state.clone();
    let hover = state.clone();
    row_frame(
        ElementId::NamedChild(id.clone().into(), "more".into()),
        look,
        Role::Button,
        highlighted,
        false,
        pointer_cursors,
    )
    .text_color(look.muted_foreground)
    .when(more == MoreState::Loading, |this| {
        this.child(Spinner::new(ElementId::NamedChild(
            id.clone().into(),
            "more-spinner".into(),
        )))
    })
    .child(match more {
        MoreState::Idle => LOAD_MORE,
        MoreState::Loading => LOADING_MORE,
        MoreState::Failed => MORE_FAILED,
    })
    .when(!look.touch, |this| {
        this.on_mouse_move(move |_, _, cx| highlight_on_hover::<E, O>(&hover, row, cx))
    })
    .on_click(move |_, _, cx| {
        click.update(cx, |owner, cx| owner.browser_mut().load_more(cx));
    })
    .into_any_element()
}

/// Puts the keyboard highlight on the row the pointer is on.
pub(crate) fn highlight_on_hover<E: Entry, O: Host<E>>(
    state: &Entity<O>,
    row: usize,
    cx: &mut gpui_kit::App,
) {
    state.update(cx, |owner, cx| {
        if owner.browser().highlighted != Some(row) {
            owner.browser_mut().highlighted = Some(row);
            cx.notify();
        }
    });
}

/// A column that stacks a dialog's parts.
pub(crate) fn stack() -> gpui_kit::Div {
    v_flex().gap_2()
}
