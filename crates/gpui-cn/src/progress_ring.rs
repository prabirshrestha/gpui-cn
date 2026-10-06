use std::f32::consts::TAU;

use gpui_kit::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder,
    Pixels, Point, RenderOnce, SharedString, StyleRefinement, Styled, TestSupportExt as _, Window,
    base::{self, StyledExt as _, transition},
    canvas, div, point,
    prelude::FluentBuilder as _,
    px,
};

use crate::{ActiveTheme as _, Theme};

/// The strength of the track under the arc: 20%, as the bar's track
/// (shadcn's `bg-primary/20`).
const TRACK_STRENGTH: f32 = 0.2;

/// A circular progress indicator on `gpui_base::Progress`: a track ring
/// with an arc that grows clockwise from twelve o'clock.
///
/// Base owns the progress role and the value assistive technology reads.
/// This type owns the look: a ring of the theme's ring size and stroke,
/// painted in the inherited text color, with the track at a fifth of
/// that. `Styled` sizes and `text_color` colors it:
///
/// ```
/// use gpui_cn::ProgressRing;
/// use gpui_kit::Styled as _;
///
/// let _ = ProgressRing::new("context").value(57.);
/// let _ = ProgressRing::new("big").value(80.).size_8();
/// ```
///
/// A value is a percentage, clamped to `0..=100`. A new value moves the
/// arc with the theme's slide motion. The id keys the motion, so it must
/// be stable across frames.
#[derive(IntoElement)]
#[non_exhaustive]
pub struct ProgressRing {
    id: ElementId,
    style: StyleRefinement,
    value: f32,
    accessibility_label: Option<SharedString>,
}

impl ProgressRing {
    /// An empty ring with a stable id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            value: 0.,
            accessibility_label: None,
        }
    }

    /// The percentage done, clamped to `0..=100`.
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0., 100.);
        self
    }

    /// The percentage done. The builder takes the name `value`, so the
    /// reader says what the value is.
    pub fn percentage(&self) -> f32 {
        self.value
    }

    /// The name a screen reader announces.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }
}

impl Styled for ProgressRing {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The point on a circle at `fraction` of a turn clockwise from the top.
fn on_circle(center: Point<Pixels>, radius: Pixels, fraction: f32) -> Point<Pixels> {
    let angle = fraction * TAU;
    point(
        center.x + radius * angle.sin(),
        center.y - radius * angle.cos(),
    )
}

/// Paints the arc from the top through `fraction` of a turn clockwise.
/// A full turn is two half arcs, because an arc cannot end where it began.
fn paint_arc(
    center: Point<Pixels>,
    radius: Pixels,
    fraction: f32,
    width: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    if fraction <= 0. {
        return;
    }
    let mut path = PathBuilder::stroke(width);
    let radii = point(radius, radius);
    path.move_to(on_circle(center, radius, 0.));
    if fraction >= 1. {
        path.arc_to(radii, px(0.), false, true, on_circle(center, radius, 0.5));
        path.arc_to(radii, px(0.), false, true, on_circle(center, radius, 0.));
    } else {
        path.arc_to(
            radii,
            px(0.),
            fraction > 0.5,
            true,
            on_circle(center, radius, fraction),
        );
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

impl RenderOnce for ProgressRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let slide = Theme::global(cx).motion.slide_transition();
        let color = window.text_style().color;
        let (size, stroke) = {
            let metrics = &cx.theme().metrics;
            (metrics.ring_size, metrics.ring_stroke)
        };
        let fraction = transition(
            ElementId::NamedChild(self.id.clone().into(), "value".into()),
            self.value / 100.,
            slide,
            window,
            cx,
        )
        .clamp(0., 1.);
        let progress_id = ElementId::NamedChild(self.id.clone().into(), "progress".into());
        div()
            .id(self.id)
            .test_support()
            .flex_shrink_0()
            .size(size)
            .refine_style(&self.style)
            .child(
                base::Progress::new(progress_id)
                    .value(self.value)
                    .indeterminate(false)
                    .when_some(self.accessibility_label, |this, label| {
                        this.accessibility_label(label)
                    })
                    .size_full()
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let width = stroke;
                                let radius =
                                    (bounds.size.width.min(bounds.size.height) - width) / 2.;
                                let center = bounds.center();
                                let track = color.opacity(TRACK_STRENGTH);
                                paint_arc(center, radius, 1., width, track, window);
                                paint_arc(center, radius, fraction, width, color, window);
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_clamp() {
        assert_eq!(ProgressRing::new("r").value(40.).percentage(), 40.);
        assert_eq!(ProgressRing::new("r").value(150.).percentage(), 100.);
        assert_eq!(ProgressRing::new("r").value(-5.).percentage(), 0.);
    }

    #[test]
    fn the_arc_starts_at_the_top_and_runs_clockwise() {
        let center = point(px(10.), px(10.));
        let radius = px(8.);
        let top = on_circle(center, radius, 0.);
        assert_eq!((top.x, top.y), (px(10.), px(2.)));
        let right = on_circle(center, radius, 0.25);
        assert!((right.x - px(18.)).abs() < px(0.001) && (right.y - px(10.)).abs() < px(0.001));
        let bottom = on_circle(center, radius, 0.5);
        assert!((bottom.x - px(10.)).abs() < px(0.001) && (bottom.y - px(18.)).abs() < px(0.001));
    }
}
