use gpui_cn::{ActiveTheme as _, Switch, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Disableable as _, div,
};

use crate::{Story, note, page, section};

/// The switch in every state.
pub struct SwitchStory {
    wifi: bool,
    bluetooth: bool,
}

impl Story for SwitchStory {
    fn title() -> &'static str {
        "Switch"
    }

    fn icon() -> IconName {
        IconName::Settings2
    }

    fn description() -> &'static str {
        "A control that allows the user to toggle between checked and not checked."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self {
            wifi: true,
            bluetooth: false,
        })
        .into()
    }
}

impl Render for SwitchStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let control = cx.theme().text_control;
        let labeled = |label: &'static str, switch: Switch| {
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(switch.accessibility_label(label))
                .child(
                    div()
                        .text_size(control.size)
                        .line_height(control.line_height)
                        .child(label),
                )
        };
        page([
            section(
                "Controlled",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The value is the story's own. Tab to a switch and press Space or \
                         Enter to flip it from the keyboard.",
                        cx,
                    ))
                    .child(labeled(
                        "Wi-Fi",
                        Switch::new("wifi")
                            .checked(self.wifi)
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.wifi = *checked;
                                cx.notify();
                            })),
                    ))
                    .child(labeled(
                        "Bluetooth",
                        Switch::new("bluetooth")
                            .checked(self.bluetooth)
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.bluetooth = *checked;
                                cx.notify();
                            })),
                    )),
            )
            .into_any_element(),
            section(
                "Disabled",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(labeled(
                        "On",
                        Switch::new("disabled-on").checked(true).disabled(true),
                    ))
                    .child(labeled("Off", Switch::new("disabled-off").disabled(true))),
            )
            .into_any_element(),
        ])
    }
}
