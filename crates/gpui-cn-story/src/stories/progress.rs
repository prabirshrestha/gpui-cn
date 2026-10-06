use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, Progress, ProgressRing, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Disableable as _, div, px,
};

use crate::{Story, note, page, section};

/// How far one press of the story's buttons moves the driven bar.
const STEP: f32 = 10.;

/// Progress bars at fixed values, one the story drives, and one with no
/// value.
pub struct ProgressStory {
    value: f32,
}

impl Story for ProgressStory {
    fn title() -> &'static str {
        "Progress"
    }

    fn icon() -> IconName {
        IconName::ChartPie
    }

    fn description() -> &'static str {
        "Displays an indicator showing the completion progress of a task, typically \
         displayed as a progress bar."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self { value: 40. }).into()
    }
}

impl ProgressStory {
    fn step(&mut self, by: f32, cx: &mut Context<Self>) {
        self.value = (self.value + by).clamp(0., 100.);
        cx.notify();
    }
}

impl Render for ProgressStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let control = cx.theme().text_control;
        let muted = cx.theme().muted_foreground();
        let value = self.value;
        let width = px(320.);
        page([
            section(
                "Values",
                div().flex().flex_col().gap_4().w(width).children(
                    [
                        ("zero", 0.),
                        ("third", 33.),
                        ("two-thirds", 66.),
                        ("full", 100.),
                    ]
                    .into_iter()
                    .map(|(id, value)| Progress::new(id).value(value)),
                ),
            )
            .into_any_element(),
            section(
                "Controlled",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(width)
                    .child(note(
                        "The value is the story's own. The indicator slides to each new value.",
                        cx,
                    ))
                    .child(
                        Progress::new("driven")
                            .value(value)
                            .accessibility_label("Upload"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("less")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .icon(IconName::Minus)
                                    .accessibility_label("Less")
                                    .tooltip("Less")
                                    .disabled(value <= 0.)
                                    .on_click(cx.listener(|this, _, _, cx| this.step(-STEP, cx))),
                            )
                            .child(
                                Button::new("more")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .icon(IconName::Plus)
                                    .accessibility_label("More")
                                    .tooltip("More")
                                    .disabled(value >= 100.)
                                    .on_click(cx.listener(|this, _, _, cx| this.step(STEP, cx))),
                            )
                            .child(
                                div()
                                    .text_size(control.size)
                                    .line_height(control.line_height)
                                    .text_color(muted)
                                    .child(format!("{value:.0}%")),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Indeterminate",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(width)
                    .child(note(
                        "With no value the bar does not know how far along the work is. A \
                         segment sweeps across it, and holds still in the middle when motion \
                         is reduced.",
                        cx,
                    ))
                    .child(Progress::new("indeterminate").accessibility_label("Syncing")),
            )
            .into_any_element(),
            section(
                "Ring",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "A ring fills clockwise from the top, in the text color. It follows the \
                         bar above, and its size and color come from `Styled`.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(ProgressRing::new("ring-driven").value(value))
                            .child(ProgressRing::new("ring-quarter").value(25.).size_8())
                            .child(
                                ProgressRing::new("ring-full")
                                    .value(100.)
                                    .size_12()
                                    .text_color(cx.theme().info),
                            ),
                    ),
            )
            .into_any_element(),
        ])
    }
}
