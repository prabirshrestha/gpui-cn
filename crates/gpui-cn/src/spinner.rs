use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, TestSupportExt as _, Window, assets::IconName, base::StyledExt as _,
    div, percentage,
};

use crate::{Icon, Theme};

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
/// `role="status" aria-label="Loading"`. Under reduced motion it holds
/// still and asks for no frames.
#[derive(IntoElement)]
pub struct Spinner {
    id: ElementId,
    style: StyleRefinement,
    accessibility_label: Option<SharedString>,
}

impl Spinner {
    /// A spinner with a stable id, which keys its turn.
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

/// The accessible name when none is given, as shadcn's.
const DEFAULT_LABEL: &str = "Loading";

impl RenderOnce for Spinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = Theme::holds_still(cx);
        let icon = Icon::from(IconName::LoaderCircle).size_full();
        let turn_id = ElementId::NamedChild(self.id.clone().into(), "turn".into());
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
            .child(if still {
                icon.into_any_element()
            } else {
                // `Animation` paces linearly unless given an easing.
                icon.with_animation(turn_id, Animation::new(TURN).repeat(), |icon, delta| {
                    icon.rotate(percentage(delta))
                })
                .into_any_element()
            })
    }
}
