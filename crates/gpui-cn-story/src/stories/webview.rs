use gpui_cn::{ActiveTheme as _, BrowserState, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div,
};

use crate::{Story, note, page, section};

/// A browser with tabs, back, forward, reload and an address field, on
/// native web views.
pub struct WebViewStory {
    browser: Result<Entity<BrowserState>, SharedString>,
}

impl Story for WebViewStory {
    fn title() -> &'static str {
        "Browser"
    }

    fn icon() -> IconName {
        IconName::Globe
    }

    fn description() -> &'static str {
        "Use to embed web pages in a window, with tabs and an address field."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        let browser = BrowserState::open("https://example.com", window, cx)
            .map_err(|error| SharedString::from(error.to_string()));
        cx.new(|_| Self { browser }).into()
    }
}

impl Render for WebViewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let radius = cx.theme().radius_lg();
        let content = match &self.browser {
            Ok(browser) => div()
                .w_full()
                .h_96()
                .rounded(radius)
                .overflow_hidden()
                .child(browser.clone())
                .into_any_element(),
            Err(error) => {
                note(format!("The platform has no native web view: {error}"), cx).into_any_element()
            }
        };
        page([section(
            "A browser",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(
                    "Pages are native views above the window. Type an address or a search in \
                     the field, open tabs with the plus, and links that ask for a new window \
                     open a tab.",
                    cx,
                ))
                .child(content),
        )])
    }
}
