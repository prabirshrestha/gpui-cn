//! Text that gives up its middle when it does not fit, so both ends stay
//! readable: a branch such as `origin/fix/psmode-curre...x-evidence`.

use std::{cell::RefCell, rc::Rc};

use gpui_kit::{
    AbsoluteLength, App, AvailableSpace, Bounds, DefiniteLength, Element, ElementId,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, ShapedLine, SharedString,
    Size, Style, TextAlign, Window, size,
};

/// The mark that stands for the removed middle.
const ELLIPSIS: &str = "...";

/// `text` cut in the middle until `fits` accepts it: the same number of
/// characters, give or take one, stay at each end, and the ellipsis joins
/// them. Text that already fits comes back whole. When even the ellipsis
/// does not fit, the ellipsis alone comes back.
pub(crate) fn middle_ellipsis(text: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let candidate = |kept: usize| {
        let head = kept.div_ceil(2);
        let tail = kept / 2;
        let mut out: String = chars[..head].iter().collect();
        out.push_str(ELLIPSIS);
        out.extend(chars[chars.len() - tail..].iter());
        out
    };
    let (mut low, mut high) = (0, chars.len().saturating_sub(1));
    while low < high {
        let mid = (low + high).div_ceil(2);
        if fits(&candidate(mid)) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    candidate(low)
}

/// A single line of text in the inherited text style that is cut in the
/// middle to fit the width its parent gives it. It takes the width it
/// needs, up to what is available, and never wraps.
///
/// The text is measured while the layout runs, so the cut follows the
/// width the row has, not a guess made when the row was built.
pub(crate) struct MiddleText {
    text: SharedString,
}

impl MiddleText {
    /// A line of `text`.
    pub(crate) fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl IntoElement for MiddleText {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

type Shown = Rc<RefCell<Option<ShapedLine>>>;

impl Element for MiddleText {
    type RequestLayoutState = Shown;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Shown) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = style.line_height_in_pixels(window.rem_size());
        let shown: Shown = Rc::new(RefCell::new(None));
        let text = self.text.clone();
        let out = shown.clone();
        let mut layout = Style::default();
        layout.min_size.width =
            DefiniteLength::Absolute(AbsoluteLength::Pixels(Pixels::ZERO)).into();
        let id = window.request_measured_layout(layout, move |known, available, window, _| {
            let shape = |candidate: &str| {
                window.text_system().shape_line(
                    SharedString::from(candidate.to_string()),
                    font_size,
                    &[style.to_run(candidate.len())],
                    None,
                )
            };
            let limit = known.width.or(match available.width {
                AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let shown_text = match limit {
                Some(limit) => middle_ellipsis(&text, |candidate| shape(candidate).width <= limit),
                None => text.to_string(),
            };
            let line = shape(&shown_text);
            let width = line.width;
            *out.borrow_mut() = Some(line);
            Size {
                width: known.width.unwrap_or(width),
                height: known.height.unwrap_or(line_height),
            }
        });
        (id, shown)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Shown,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        shown: &mut Shown,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = window.text_style().line_height_in_pixels(window.rem_size());
        if let Some(line) = shown.borrow().as_ref() {
            let _ = line.paint(
                bounds.origin,
                line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        let _ = size(Pixels::ZERO, Pixels::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fits(width: usize) -> impl Fn(&str) -> bool {
        move |text: &str| text.chars().count() <= width
    }

    #[test]
    fn text_that_fits_is_whole() {
        assert_eq!(middle_ellipsis("main", fits(10)), "main");
    }

    #[test]
    fn a_long_name_keeps_both_ends() {
        let name = "origin/fix/psmode-current-evidence";
        let cut = middle_ellipsis(name, fits(20));
        assert_eq!(cut.chars().count(), 20);
        assert!(cut.starts_with("origin/"), "{cut}");
        assert!(cut.ends_with("evidence"), "{cut}");
        assert!(cut.contains("..."));
    }

    #[test]
    fn it_never_returns_more_than_fits_and_degrades_to_the_mark() {
        for width in 3..30 {
            let cut = middle_ellipsis("origin/renovate/anthropic-ai-sdk-0.x", fits(width));
            assert!(cut.chars().count() <= width, "{width}: {cut}");
        }
        assert_eq!(middle_ellipsis("abcdefgh", fits(2)), "...");
    }

    #[test]
    fn multibyte_text_is_cut_on_characters() {
        let cut = middle_ellipsis("\u{e9}t\u{e9}-\u{e9}t\u{e9}-\u{e9}t\u{e9}", fits(7));
        assert!(cut.chars().count() <= 7);
        assert!(cut.starts_with('\u{e9}') && cut.ends_with('\u{e9}'));
    }
}
