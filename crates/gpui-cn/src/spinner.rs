use std::time::Duration;

use gpui_kit::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, TestSupportExt as _,
    Window, assets::IconName, base::StyledExt as _, div, percentage,
};

use crate::{Icon, Theme, looping};

/// A shadcn-style spinner: the Lucide `loader-circle` icon turning in
/// place, for work whose length is not known.
///
/// The look is shadcn's `Spinner`: `size-4` (16px at the default font
/// size), painted with the inherited text color, and turning once a
/// second at a linear pace, Tailwind's `animate-spin`. `Styled` sizes and
/// colors it:
///
/// ```
/// use gpui_cn::Spinner;
/// use gpui_kit::Styled as _;
///
/// let _ = Spinner::new("saving");
/// let _ = Spinner::new("sync").size_6().accessibility_label("Syncing");
/// ```
///
/// Assistive technology reads it as a status named "Loading", as shadcn's
/// `role="status" aria-label="Loading"`.
///
/// The turn repaints at 30 fps on the app's shared loop clock, not at the
/// display rate: a 16px icon turning once a second moves 12 degrees a
/// step, which reads as smooth, and every spinner on screen repaints on
/// the same tick. Under reduced motion it holds still and asks for no
/// frames.
#[derive(IntoElement)]
pub struct Spinner {
    id: ElementId,
    style: StyleRefinement,
    accessibility_label: Option<SharedString>,
}

impl Spinner {
    /// A spinner with a stable id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            accessibility_label: None,
        }
    }

    /// The name a screen reader announces. The default is "Loading".
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }
}

impl Styled for Spinner {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// How long one turn takes: Tailwind's `animate-spin` is one second.
const TURN: Duration = Duration::from_secs(1);

/// How often the turn repaints: 30 fps, 12 degrees a step, which a small
/// icon shows as smooth at a fraction of the display rate's cost.
const TURN_FPS: u32 = 30;

/// The accessible name when none is given, as shadcn's.
const DEFAULT_LABEL: &str = "Loading";

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let icon = Icon::from(IconName::LoaderCircle).size_full();
        let icon = if Theme::holds_still(cx) {
            icon
        } else {
            icon.rotate(percentage(looping::phase(TURN, TURN_FPS, window, cx)))
        };
        div()
            .id(self.id)
            .role(Role::Status)
            .aria_label(
                self.accessibility_label
                    .unwrap_or_else(|| DEFAULT_LABEL.into()),
            )
            .test_support()
            .flex_shrink_0()
            .size_4()
            .refine_style(&self.style)
            .child(icon)
    }
}
