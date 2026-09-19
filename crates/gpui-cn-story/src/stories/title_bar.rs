use gpui_cn::{ActiveTheme as _, Button, ButtonSize, TitleBar, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Disableable as _, div, px,
};

use crate::{Story, frame, note, page, section};

/// The title bar, shown in a frame. The gallery's own title bars are the
/// live ones: the sidebar's carries the window controls, the content's
/// picks them up when the sidebar closes.
pub struct TitleBarStory;

impl Story for TitleBarStory {
    fn title() -> &'static str {
        "Title bar"
    }

    fn icon() -> IconName {
        IconName::WindowMaximize
    }

    fn description() -> &'static str {
        "A title bar the application draws: a drag region with room for the window controls."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

impl Render for TitleBarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sidebar = theme.sidebar;
        let border = theme.border();
        let height = theme.metrics.title_bar + px(2.);
        page([
            section(
                "With the platform inset",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "The bar is transparent and takes the surface under it. On macOS it \
                         leaves room for the window controls; on Windows and Linux it draws \
                         them at its right edge. Dragging any of these bars moves the window.",
                        cx,
                    ))
                    .child(
                        frame(height, cx).child(
                            TitleBar::new()
                                .bg(sidebar)
                                .child(
                                    Button::new("demo-toggle")
                                        .ghost()
                                        .icon(IconName::PanelLeft)
                                        .accessibility_label("Toggle sidebar"),
                                )
                                .child(
                                    Button::new("demo-back")
                                        .ghost()
                                        .icon(IconName::ArrowLeft)
                                        .accessibility_label("Back"),
                                )
                                .child(
                                    Button::new("demo-forward")
                                        .ghost()
                                        .icon(IconName::ArrowRight)
                                        .accessibility_label("Forward")
                                        .disabled(true),
                                ),
                        ),
                    ),
            )
            .into_any_element(),
            section(
                "Without the inset",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "The content's title bar while a sidebar is open: it does not reach the \
                         window's edge, so it starts at the padding.",
                        cx,
                    ))
                    .child(
                        frame(height, cx).child(
                            TitleBar::new()
                                .inset(false)
                                .border_b_1()
                                .border_color(border)
                                .child(div().text_sm().child("Untitled"))
                                .child(div().flex_1())
                                .child(
                                    Button::new("demo-share")
                                        .size(ButtonSize::Sm)
                                        .label("Share"),
                                ),
                        ),
                    ),
            )
            .into_any_element(),
        ])
    }
}
