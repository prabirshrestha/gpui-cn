use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt as _, App, ElementId, IntoElement, RenderOnce, StyleRefinement, Styled,
    Window, base::StyledExt as _, bounce, div, ease_in_out,
};

use crate::{ActiveTheme as _, Theme};

/// A shadcn-style skeleton: a rounded block in the accent fill that pulses
/// while content loads, so a list or a card keeps its shape before its
/// data arrives.
///
/// The block is a line of text high and as wide as its parent; `Styled`
/// sizes it into anything else, such as a circle for an avatar. Its fill
/// is the theme's `accent`, shadcn's `bg-accent`; on a popover use the
/// popover's own accent, so the block reads as a step above that surface
/// rather than a hole in it.
///
/// ```
/// use gpui_cn::Skeleton;
/// use gpui_kit::Styled as _;
///
/// let _ = Skeleton::new("title").w_48();
/// let _ = Skeleton::new("avatar").size_8().rounded_full();
/// ```
///
/// The pulse is shadcn's `animate-pulse`: opacity from full to half and
/// back over two seconds, eased both ways, repeating while the block is
/// rendered and still under reduced motion.
#[derive(IntoElement)]
pub struct Skeleton {
    id: ElementId,
    style: StyleRefinement,
}

impl Skeleton {
    /// A block with a stable id, which keys its pulse.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// How long one pulse takes, up and back: shadcn's `animate-pulse` is two
/// seconds.
const PULSE: Duration = Duration::from_secs(2);

/// The opacity the pulse falls to: half, as shadcn's.
const PULSE_FLOOR: f32 = 0.5;

impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (fill, radius, height) = {
            let theme = cx.theme();
            (
                theme.accent(),
                theme.radius_md(),
                theme.text_control.line_height,
            )
        };
        let block = div()
            .w_full()
            .h(height)
            .rounded(radius)
            .bg(fill)
            .refine_style(&self.style);
        if Theme::global(cx).motion.slow.is_zero() {
            return block.opacity(PULSE_FLOOR).into_any_element();
        }
        block
            .with_animation(
                self.id,
                Animation::new(PULSE)
                    .repeat()
                    .with_easing(bounce(ease_in_out)),
                |block, delta| block.opacity(1. - delta * (1. - PULSE_FLOOR)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_refine_the_block() {
        let skeleton = Skeleton::new("s").w_full();
        assert!(skeleton.style.size.width.is_some());
    }
}
