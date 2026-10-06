use gpui_kit::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StyleRefinement, Styled, TestSupportExt as _, Window, base::StyledExt as _, div,
    prelude::FluentBuilder as _,
};

use crate::{ActiveTheme as _, ProgressRing, StatusSelect};

/// The context a conversation has used, as a ring and a percent: a
/// [`ProgressRing`] beside the number, in the muted text color.
///
/// The percent is a plain value the application sets.
///
/// ```
/// use gpui_cn::ContextMeter;
///
/// let _ = ContextMeter::new("context").percent(57.);
/// ```
#[derive(IntoElement)]
#[non_exhaustive]
pub struct ContextMeter {
    id: ElementId,
    percent: f32,
    style: StyleRefinement,
}

impl ContextMeter {
    /// An empty meter with a stable id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            percent: 0.,
            style: StyleRefinement::default(),
        }
    }

    /// The percentage used, clamped to `0..=100`.
    pub fn percent(mut self, percent: f32) -> Self {
        self.percent = percent.clamp(0., 100.);
        self
    }

    /// The percentage used.
    pub fn percent_used(&self) -> f32 {
        self.percent
    }
}

impl Styled for ContextMeter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ContextMeter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, gap) = (theme.muted_foreground(), theme.metrics.status_label_gap);
        let percent = self.percent;
        div()
            .id(self.id.clone())
            .test_support()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(gap)
            .text_color(muted)
            .refine_style(&self.style)
            .child(
                ProgressRing::new(ElementId::NamedChild(self.id.into(), "ring".into()))
                    .value(percent)
                    .accessibility_label("Context used"),
            )
            .child(format!("{percent:.0}%"))
    }
}

/// The strip behind the top of a composer's card: any number of items
/// from the left, such as a folder, a device, and a branch, in the full
/// text color (each a [`StatusSelect`] that opens a menu), and optionally a [`ContextMeter`] or any element at the far
/// right. With no trailing element there is nothing at the right.
///
/// The tab is inset from each side of the card and rounded on top only.
/// The tab is exactly [`status_tab_height`](crate::theme::MetricTokens::status_tab_height)
/// tall, with the items centered in it and square bottom corners, and it
/// carries no hidden height. A [`Composer`](crate::Composer) sets its card
/// directly under the tab, tucked over the tab's bottom edge by the card's
/// hairline.
/// [`select`](Self::select) adds a dropdown item, any other element joins
/// the left side as a child, [`context`](Self::context) adds the meter,
/// and [`trailing`](Self::trailing) replaces what is at the right.
///
/// ```
/// use gpui_cn::ComposerStatusTab;
///
/// let _ = ComposerStatusTab::new("status").context(57.);
/// ```
#[derive(IntoElement)]
#[non_exhaustive]
pub struct ComposerStatusTab {
    id: ElementId,
    items: Vec<AnyElement>,
    trailing: Option<AnyElement>,
    style: StyleRefinement,
}

impl ComposerStatusTab {
    /// An empty tab with a stable id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            trailing: None,
            style: StyleRefinement::default(),
        }
    }

    /// An item that opens a menu of options. See [`StatusSelect`].
    pub fn select(self, select: StatusSelect) -> Self {
        self.child(select)
    }

    /// The context meter at the far right, at `percent` used.
    pub fn context(mut self, percent: f32) -> Self {
        let id = ElementId::NamedChild(self.id.clone().into(), "context".into());
        self.trailing = Some(ContextMeter::new(id).percent(percent).into_any_element());
        self
    }

    /// Replaces what sits at the far right.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }
}

impl Styled for ComposerStatusTab {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for ComposerStatusTab {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.items.extend(elements);
    }
}

impl RenderOnce for ComposerStatusTab {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let (height, inset, gap) = (
            metrics.status_tab_height,
            metrics.status_tab_inset,
            metrics.status_tab_gap,
        );
        let (pad_left, pad_right) = (metrics.status_tab_inset, theme.base.spacing.lg);
        let (fill, radius, text) = (
            theme.status_tab,
            metrics.status_tab_radius,
            theme.text_control,
        );
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .justify_between()
            .h(height)
            .mx(inset)
            .pl(pad_left)
            .pr(pad_right)
            .gap(gap)
            .rounded_t(radius)
            .bg(fill)
            .text_size(text.size)
            .line_height(text.line_height)
            .refine_style(&self.style)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(gap)
                    .children(self.items),
            )
            .when_some(self.trailing, |this, trailing| this.child(trailing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_percent_clamps() {
        assert_eq!(ContextMeter::new("m").percent(57.).percent_used(), 57.);
        assert_eq!(ContextMeter::new("m").percent(180.).percent_used(), 100.);
        assert_eq!(ContextMeter::new("m").percent(-1.).percent_used(), 0.);
    }
}
