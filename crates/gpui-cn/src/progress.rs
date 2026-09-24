use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StyleRefinement, Styled, TestSupportExt as _,
    Window,
    base::{self, ProgressIndicator, ProgressTrack, StyledExt as _, transition},
    div, ease_in_out,
    prelude::FluentBuilder as _,
    relative,
};

use crate::{ActiveTheme as _, Theme};

/// A shadcn-style progress bar on `gpui_base::Progress`.
///
/// Base owns the progress role and the value assistive technology reads.
/// This type owns the look, which is shadcn's `Progress`: as wide as its
/// parent, `h-2` (8px at the default font size) on the rem scale, fully
/// rounded, a track of `primary` at 20% (the theme's `progress_track`),
/// and an indicator of `primary`.
///
/// ```
/// use gpui_cn::Progress;
/// use gpui_kit::Styled as _;
///
/// let _ = Progress::new("upload").value(40.);
/// let _ = Progress::new("sync").accessibility_label("Syncing");
/// let _ = Progress::new("thin").value(None).h_1();
/// ```
///
/// A value is a percentage, clamped to `0..=100`. With no value the bar is
/// indeterminate, as in Radix: a segment slides across the track and
/// repeats, and under reduced motion it holds still in the middle. A new
/// value moves the indicator with the theme's slide motion, shadcn's
/// `transition-all`.
///
/// The id keys the motion, so it must be stable across frames.
#[derive(IntoElement)]
pub struct Progress {
    id: ElementId,
    style: StyleRefinement,
    value: Option<f32>,
    accessibility_label: Option<SharedString>,
}

impl Progress {
    /// An indeterminate bar with a stable id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            value: None,
            accessibility_label: None,
        }
    }

    /// The percentage done, clamped to `0..=100`, or `None` for work whose
    /// length is not known.
    pub fn value(mut self, value: impl Into<Option<f32>>) -> Self {
        self.value = value.into().map(|value| value.clamp(0., 100.));
        self
    }

    /// The percentage done, or `None` when the bar is indeterminate. The
    /// builder takes the name `value`, so the reader says what the value is.
    pub fn percentage(&self) -> Option<f32> {
        self.value
    }

    /// The name a screen reader announces.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }
}

impl Styled for Progress {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// How long one sweep of the indeterminate segment takes: one second, as
/// gpui-kit's progress bar. shadcn has no indeterminate bar.
const SWEEP: Duration = Duration::from_secs(1);

/// How far in from each end the still indeterminate segment sits under
/// reduced motion, as a fraction of the track: the middle 35%, as
/// gpui-kit's.
const STILL_INSET: f32 = 0.325;

/// Where the indeterminate segment spans at `delta` through a sweep, as
/// fractions of the track: its trailing edge eases to the far end over the
/// whole sweep, and its leading edge follows over the second half.
fn sweep(delta: f32) -> (f32, f32) {
    let start = ease_in_out(((delta - 0.5) / 0.5).clamp(0., 1.));
    let end = 1. - ease_in_out(1. - delta);
    (start, end.max(start))
}

impl RenderOnce for Progress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = cx.reduce_motion() || Theme::global(cx).motion.slow.is_zero();
        let slide = Theme::global(cx).motion.slide_transition();
        let (track, fill, radius) = {
            let theme = cx.theme();
            (theme.progress_track, theme.primary(), theme.radius_full())
        };
        let id = self.id;
        let child_id = |name: &'static str| ElementId::NamedChild(id.clone().into(), name.into());
        let fraction = self.value.map(|value| {
            transition(child_id("value"), value / 100., slide, window, cx).clamp(0., 1.)
        });

        // GPUI clips to rectangles, so the indicator carries the track's
        // radius itself rather than relying on the track to round it off.
        let indicator = ProgressIndicator::new()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .child(
                div()
                    .id(child_id("indicator"))
                    .test_support()
                    .size_full()
                    .rounded(radius)
                    .bg(fill),
            );
        let indicator = match fraction {
            Some(fraction) => indicator.w(relative(fraction)).into_any_element(),
            None if still => indicator
                .left(relative(STILL_INSET))
                .w(relative(1. - STILL_INSET * 2.))
                .into_any_element(),
            None => indicator
                .with_animation(
                    child_id("sweep"),
                    Animation::new(SWEEP).repeat(),
                    |indicator, delta| {
                        let (start, end) = sweep(delta);
                        indicator.left(relative(start)).w(relative(end - start))
                    },
                )
                .into_any_element(),
        };

        div()
            .id(id.clone())
            .test_support()
            .flex()
            .w_full()
            .h_2()
            .refine_style(&self.style)
            .child(
                base::Progress::new(child_id("progress"))
                    .value(self.value.unwrap_or(0.))
                    .indeterminate(self.value.is_none())
                    .when_some(self.accessibility_label, |this, label| {
                        this.accessibility_label(label)
                    })
                    .relative()
                    .size_full()
                    .child(
                        ProgressTrack::new()
                            .absolute()
                            .size_full()
                            .rounded(radius)
                            .bg(track),
                    )
                    .child(indicator),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_clamp_and_none_is_indeterminate() {
        assert_eq!(Progress::new("p").percentage(), None);
        assert_eq!(Progress::new("p").value(40.).percentage(), Some(40.));
        assert_eq!(Progress::new("p").value(150.).percentage(), Some(100.));
        assert_eq!(Progress::new("p").value(-5.).percentage(), Some(0.));
        assert_eq!(Progress::new("p").value(40.).value(None).percentage(), None);
    }

    #[test]
    fn a_sweep_grows_from_the_start_and_leaves_by_the_end() {
        assert_eq!(sweep(0.), (0., 0.));
        assert_eq!(sweep(0.5).0, 0.);
        assert_eq!(sweep(1.), (1., 1.));
        let (start, end) = sweep(0.75);
        assert!(start > 0. && end > start && end <= 1.);
    }
}
