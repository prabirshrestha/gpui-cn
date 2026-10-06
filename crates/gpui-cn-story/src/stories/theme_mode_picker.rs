use gpui_cn::{Theme, ThemeMode, ThemeModePicker, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Disableable as _, div,
};

use crate::{Story, note, page, section};

/// The three appearance cards.
pub struct ThemeModePickerStory {
    value: ThemeMode,
}

impl Story for ThemeModePickerStory {
    fn title() -> &'static str {
        "Theme"
    }

    fn icon() -> IconName {
        IconName::Sun
    }

    fn description() -> &'static str {
        "Picks light, dark, or system: three pictures of the appearance the theme can follow, \
         with the chosen one ringed."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self {
            value: ThemeMode::Light,
        })
        .into()
    }
}

impl Render for ThemeModePickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = Theme::global(cx).mode;
        page([
            section(
                "Live",
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Bound to the theme: choosing a card changes the whole gallery, as \
                         the appearance settings page does.",
                        cx,
                    ))
                    .child(
                        ThemeModePicker::new("live")
                            .value(mode)
                            .on_change(|mode, _, cx| Theme::change(*mode, cx)),
                    ),
            )
            .into_any_element(),
            section(
                "Controlled",
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The choice is the story's own. Tab to a card and press Space or \
                         Enter to choose it from the keyboard.",
                        cx,
                    ))
                    .child(
                        ThemeModePicker::new("controlled")
                            .value(self.value)
                            .on_change(cx.listener(|this, mode, _, cx| {
                                this.value = *mode;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element(),
            section(
                "Disabled",
                div().w_full().child(
                    ThemeModePicker::new("disabled")
                        .value(ThemeMode::Dark)
                        .disabled(true),
                ),
            )
            .into_any_element(),
        ])
    }
}
