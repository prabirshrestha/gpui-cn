//! A vertical scroll region that stretches past its ends and springs
//! back, on every platform.

use gpui_kit::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    ScrollHandle, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    base::{ScrollBounce, StyledExt as _, TestSupportExt as _},
    div, px,
};

/// A vertical scroll region with the overscroll bounce of a `UIScrollView`.
///
/// `gpui_base::ScrollBounce` plays the stretch and the return, and turns
/// itself on for iOS and Android alone. gpui-cn turns it on everywhere,
/// so a trackpad on a desktop feels the same end of the page as a finger
/// on a phone. Reduced motion turns the bounce off; the region still
/// scrolls. The scroll position lives in window state under `id`, so it
/// survives a re-render.
///
/// The region fills its parent. Give it a sized parent, and put fixed
/// chrome such as a toolbar outside it.
///
/// ```
/// use gpui_cn::ScrollArea;
/// use gpui_kit::{IntoElement, ParentElement as _, div};
///
/// fn page() -> impl IntoElement {
///     ScrollArea::new("page").child(div().child("A long page"))
/// }
/// ```
#[derive(IntoElement)]
pub struct ScrollArea {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ScrollArea {
    /// An empty scroll region. The id owns the scroll position and the
    /// bounce state.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for ScrollArea {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for ScrollArea {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state(
                ElementId::NamedChild(self.id.clone().into(), "scroll".into()),
                cx,
                |_, _| ScrollHandle::new(),
            )
            .read(cx)
            .clone();
        // The caller styles one element, but there are two: an outer box
        // that sits in the caller's layout, and the viewport inside it
        // that lays out the content. The box takes what places it (size,
        // flex, margin, position); the viewport takes the rest (padding,
        // direction, gap, colors).
        let (frame_style, viewport_style) = split_style(self.style);
        let viewport = div()
            .id(self.id.clone())
            .test_support()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&handle)
            .refine_style(&viewport_style)
            .children(self.children);
        let bounce = ScrollBounce::new(
            ElementId::NamedChild(self.id.clone().into(), "bounce".into()),
            &handle,
            viewport,
        )
        .enabled(true);
        // GPUI lets every scroll region under the pointer take a wheel
        // step, so a region inside a page would scroll the page too. The
        // box keeps the step when the region has room to scroll, after
        // the viewport moved and the bounce took what was left; a region
        // whose content fits passes it on.
        div()
            .id(ElementId::NamedChild(self.id.into(), "frame".into()))
            .refine_style(&frame_style)
            .on_scroll_wheel(move |_, _, cx| {
                if handle.max_offset().y > px(0.) {
                    cx.stop_propagation();
                }
            })
            .child(bounce)
    }
}

/// Splits a refinement into what places an element in its parent and what
/// lays out its content.
fn split_style(style: StyleRefinement) -> (StyleRefinement, StyleRefinement) {
    let mut frame = StyleRefinement::default();
    let mut viewport = style;
    frame.size = std::mem::take(&mut viewport.size);
    frame.min_size = std::mem::take(&mut viewport.min_size);
    frame.max_size = std::mem::take(&mut viewport.max_size);
    frame.flex_grow = viewport.flex_grow.take();
    frame.flex_shrink = viewport.flex_shrink.take();
    frame.flex_basis = viewport.flex_basis.take();
    frame.align_self = viewport.align_self.take();
    frame.margin = std::mem::take(&mut viewport.margin);
    frame.position = viewport.position.take();
    frame.inset = std::mem::take(&mut viewport.inset);
    (frame, viewport)
}
