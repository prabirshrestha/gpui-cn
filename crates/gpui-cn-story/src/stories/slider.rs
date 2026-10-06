use gpui_cn::{ActiveTheme as _, Slider, SliderEvent, SliderState, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window, base::Disableable as _, div, px,
};

use crate::{Story, note, page, section};

/// The slider continuous, with stops, and disabled.
pub struct SliderStory {
    volume: Entity<SliderState>,
    effort: Entity<SliderState>,
    disabled: Entity<SliderState>,
}

impl Story for SliderStory {
    fn title() -> &'static str {
        "Slider"
    }

    fn icon() -> IconName {
        IconName::Settings
    }

    fn description() -> &'static str {
        "An input where the user selects a value from within a given range."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let volume = cx.new(|_| SliderState::new().min(0.).max(100.).default_value(40.));
            let effort = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(5.)
                    .step(1.)
                    .default_value(2.)
            });
            let disabled = cx.new(|_| SliderState::new().min(0.).max(100.).default_value(60.));
            for state in [&volume, &effort] {
                cx.subscribe(state, |_, _, _: &SliderEvent, cx| cx.notify())
                    .detach();
            }
            Self {
                volume,
                effort,
                disabled,
            }
        })
        .into()
    }
}

impl Render for SliderStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let control = cx.theme().text_control;
        let muted = cx.theme().muted_foreground();
        let width = px(320.);
        let readout = |label: &'static str, value: String| {
            div()
                .text_size(control.size)
                .line_height(control.line_height)
                .text_color(muted)
                .child(format!("{label}: {value}"))
        };
        let volume = self.volume.read(cx).value().end();
        let effort = self.effort.read(cx).value().end();
        page([
            section(
                "Continuous",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(width)
                    .child(note(
                        "Drag the thumb or press the track. Tab to the slider and use the arrow \
                         keys, Page Up, Page Down, Home, and End.",
                        cx,
                    ))
                    .child(
                        Slider::new(&self.volume)
                            .id("volume")
                            .accessibility_label("Volume"),
                    )
                    .child(readout("Volume", format!("{volume}"))),
            )
            .into_any_element(),
            section(
                "Stops",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(width)
                    .child(
                        Slider::new(&self.effort)
                            .id("effort")
                            .stops(6)
                            .accessibility_label("Effort"),
                    )
                    .child(readout("Level", format!("{effort}"))),
            )
            .into_any_element(),
            section(
                "Disabled",
                div()
                    .w(width)
                    .child(Slider::new(&self.disabled).id("locked").disabled(true)),
            )
            .into_any_element(),
        ])
    }
}
