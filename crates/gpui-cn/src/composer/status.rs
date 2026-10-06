use gpui_kit::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    SharedString, StyleRefinement, Styled, TestSupportExt as _, Window, assets::IconName,
    base::StyledExt as _, div, prelude::FluentBuilder as _,
};

use crate::{ActiveTheme as _, Icon, ProgressRing};

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
        let (muted, gap) = (theme.muted_foreground(), theme.base.spacing.sm);
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

/// An icon and a label in the muted text color, one item of a
/// [`ComposerStatusTab`]: a branch, a folder.
///
/// ```
/// use gpui_cn::StatusLabel;
/// use gpui_kit::assets::IconName;
///
/// let _ = StatusLabel::new(IconName::Folder, "project-sea");
/// ```
#[derive(IntoElement)]
#[non_exhaustive]
pub struct StatusLabel {
    icon: Icon,
    label: SharedString,
    style: StyleRefinement,
}

impl StatusLabel {
    /// A label with the icon before it.
    pub fn new(icon: impl Into<Icon>, label: impl Into<SharedString>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for StatusLabel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StatusLabel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, gap) = (theme.muted_foreground(), theme.base.spacing.xs);
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(gap)
            .text_color(muted)
            .refine_style(&self.style)
            .child(self.icon.size_3p5())
            .child(self.label)
    }
}

/// The grey strip behind the top of a composer's card: items on the left,
/// such as a branch and a folder, and a [`ContextMeter`] at the far right.
///
/// The tab is inset from each side of the card and rounded on top only.
/// Its bottom edge is meant to sit behind the card, so the items are
/// centered above it, in the part of the tab that shows.
/// [`branch`](Self::branch), [`folder`](Self::folder), and
/// [`context`](Self::context) fill the default items; any other element
/// joins the left side as a child, and [`trailing`](Self::trailing)
/// replaces the meter.
///
/// ```
/// use gpui_cn::ComposerStatusTab;
///
/// let _ = ComposerStatusTab::new("status")
///     .branch("Main")
///     .folder("project-sea")
///     .context(57.);
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

    /// A git branch item.
    pub fn branch(self, name: impl Into<SharedString>) -> Self {
        self.child(StatusLabel::new(IconName::GitBranch, name))
    }

    /// A folder item.
    pub fn folder(self, name: impl Into<SharedString>) -> Self {
        self.child(StatusLabel::new(IconName::Folder, name))
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
        let (height, inset, overlap) = (
            metrics.status_tab_height,
            metrics.status_tab_inset,
            metrics.status_tab_overlap,
        );
        let (fill, radius, text) = (theme.status_tab, theme.radius_xl(), theme.text_control);
        let (gap, pad) = (theme.base.spacing.md, theme.base.spacing.md);
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .justify_between()
            .h(height)
            .mx(inset)
            .px(pad)
            .pb(overlap)
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
