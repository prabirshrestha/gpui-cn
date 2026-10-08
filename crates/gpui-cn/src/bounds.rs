//! Reading an element's bounds as it is laid out.

use gpui_kit::{App, Bounds, ParentElement, Pixels, Styled as _, Window, canvas};

/// Reports an element's padding box to `callback` during prepaint.
///
/// `gpui_base::ElementExt::on_prepaint` adds an absolute canvas with no
/// insets, which taffy places at the content edge but sizes to the padding
/// box, so the bounds it reports are off by the element's padding. This
/// pins the canvas to the padding box on every side instead.
pub(crate) trait OnPaddingBounds: ParentElement + Sized {
    fn on_padding_bounds(
        self,
        callback: impl FnOnce(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.child(
            canvas(
                move |bounds, window, cx| callback(bounds, window, cx),
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
    }
}

impl<T: ParentElement> OnPaddingBounds for T {}
