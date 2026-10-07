use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, Progress, Spinner, Switch, gpui_kit::assets::IconName,
};
use std::time::Duration;

use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Task, Window, base::Disableable as _, div, px,
};

use crate::{Story, note, page, section};

/// How far one press of the story's buttons moves the driven bar.
const STEP: f32 = 10.;

/// How far the live example moves on each tick, and how long a tick is.
const LIVE_STEP: f32 = 2.;
const LIVE_TICK: Duration = Duration::from_millis(100);

/// Progress as a bar and as a ring, at fixed values, driven by the story,
/// with no value, and live.
pub struct ProgressStory {
    value: f32,
    live: f32,
    ticker: Option<Task<()>>,
    saving: bool,
}

impl Story for ProgressStory {
    fn title() -> &'static str {
        "Progress"
    }

    fn icon() -> IconName {
        IconName::ChartPie
    }

    fn description() -> &'static str {
        "Displays an indicator showing the completion progress of a task, as a bar or a ring. \
         With no value it shows work of unknown length."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let mut story = Self {
                value: 40.,
                live: 0.,
                ticker: None,
                saving: true,
            };
            story.start_live(cx);
            story
        })
        .into()
    }
}

impl ProgressStory {
    /// The percentage the live example has reached.
    pub fn live(&self) -> f32 {
        self.live
    }

    /// Whether the live example is still running.
    pub fn is_live(&self) -> bool {
        self.ticker.is_some()
    }

    /// Starts the live example from nothing. It stops by itself at the
    /// end, so the page asks for nothing once it is done.
    pub fn start_live(&mut self, cx: &mut Context<Self>) {
        self.live = 0.;
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(LIVE_TICK).await;
                let more = this
                    .update(cx, |this, cx| {
                        this.live = (this.live + LIVE_STEP).min(100.);
                        let more = this.live < 100.;
                        if !more {
                            this.ticker = None;
                        }
                        cx.notify();
                        more
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn step(&mut self, by: f32, cx: &mut Context<Self>) {
        self.value = (self.value + by).clamp(0., 100.);
        cx.notify();
    }
}

impl Render for ProgressStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let control = cx.theme().text_control;
        let muted = cx.theme().muted_foreground();
        let (value, live) = (self.value, self.live);
        let width = px(320.);
        let percent = |value: f32, cx: &Context<Self>| {
            div()
                .text_size(control.size)
                .line_height(control.line_height)
                .text_color(cx.theme().muted_foreground())
                .child(format!("{value:.0}%"))
        };
        let saving = self.saving;
        page([
            section(
                "Bar",
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .w(width)
                    .child(note("Fixed values, from nothing to done.", cx))
                    .children(
                        [
                            ("zero", 0.),
                            ("third", 33.),
                            ("two-thirds", 66.),
                            ("full", 100.),
                        ]
                        .into_iter()
                        .map(|(id, value)| Progress::new(id).value(value)),
                    )
                    .child(note(
                        "A value the story owns: the indicator slides to each new value.",
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
                            .child(percent(value, cx)),
                    )
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
                "Circular",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(note(
                        "A ring fills clockwise from the top in the text color, and its size \
                         and color come from `Styled`. It follows the value the bar above \
                         holds.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                Progress::new("ring-driven")
                                    .circular()
                                    .value(value)
                                    .accessibility_label("Upload"),
                            )
                            .child(Progress::new("ring-quarter").circular().value(25.).size_8())
                            .child(
                                Progress::new("ring-full")
                                    .circular()
                                    .value(100.)
                                    .size_12()
                                    .text_color(cx.theme().info),
                            ),
                    )
                    .child(note(
                        "With no value the ring turns, as a spinner does: use a spinner for \
                         work of unknown length, and a ring for a known percentage.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                Progress::new("ring-indeterminate")
                                    .circular()
                                    .accessibility_label("Syncing"),
                            )
                            .child(
                                Progress::new("ring-indeterminate-large")
                                    .circular()
                                    .size_8(),
                            ),
                    )
                    .child(note(
                        "A live value: the ring and the bar fill together, and Reset starts \
                         them again.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .w(width)
                            .child(Progress::new("ring-live").circular().value(live).size_6())
                            .child(div().flex_1().child(Progress::new("bar-live").value(live)))
                            .child(percent(live, cx))
                            .child(
                                Button::new("live-reset")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .label("Reset")
                                    .on_click(cx.listener(|this, _, _, cx| this.start_live(cx))),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Spinner",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(note(
                        "A spinner is a ring with no value: use it for work of unknown \
                         length, and a circular Progress with a value for a known percentage.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(Spinner::new("default"))
                            .child(Spinner::new("large").size_8())
                            .child(Spinner::new("muted").text_color(muted)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(control.size)
                            .line_height(control.line_height)
                            .text_color(muted)
                            .child(Spinner::new("inline"))
                            .child("Loading..."),
                    )
                    .child(note(
                        "A busy button shows a spinner in place of its icon and ignores \
                         presses until the work ends. It keeps its colors and its focus.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("busy-primary")
                                    .primary()
                                    .loading(true)
                                    .label("Submit"),
                            )
                            .child(
                                Button::new("busy-outline")
                                    .outline()
                                    .loading(true)
                                    .label("Loading"),
                            )
                            .child(
                                Button::new("busy-ghost")
                                    .ghost()
                                    .size(ButtonSize::Sm)
                                    .loading(true)
                                    .label("Refreshing"),
                            )
                            .child(
                                Button::new("busy-icon")
                                    .outline()
                                    .icon(IconName::Copy)
                                    .loading(true)
                                    .accessibility_label("Copy")
                                    .tooltip("Copy"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                Button::new("save")
                                    .primary()
                                    .icon(IconName::Check)
                                    .loading(saving)
                                    .label(if saving { "Saving" } else { "Save" }),
                            )
                            .child(
                                Switch::new("saving")
                                    .checked(saving)
                                    .accessibility_label("Busy")
                                    .on_change(cx.listener(|this, checked, _, cx| {
                                        this.saving = *checked;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(control.size)
                                    .line_height(control.line_height)
                                    .child("Busy"),
                            ),
                    ),
            )
            .into_any_element(),
        ])
    }
}
