use gpui_cn::{ActiveTheme as _, Tab, Tabs, TabsEvent, TabsState, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Pixels,
    Render, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::{Story, frame, note, page, section};

/// The tab strip over a content panel, so the selected tab's shoulders
/// read as one surface with the content.
pub struct TabsStory {
    default: Entity<TabsState>,
    overflow: Entity<TabsState>,
    plain: Entity<TabsState>,
    /// The number of tabs the new-tab controls have added, for their labels.
    added: usize,
}

/// Terminal tabs with generic working directories as their labels.
fn terminal_tabs(count: usize) -> Vec<Tab> {
    (0..count)
        .map(|n| {
            let label = match n % 3 {
                0 => "~",
                1 => "~/app",
                _ => "~/docs",
            };
            Tab::new(format!("terminal-{n}"), label).with_icon(IconName::SquareTerminal)
        })
        .collect()
}

impl Story for TabsStory {
    fn title() -> &'static str {
        "Tabs"
    }

    fn icon() -> IconName {
        IconName::SquareTerminal
    }

    fn description() -> &'static str {
        "A browser-style tab strip with a close control, a new-tab control, and scroll \
         controls when the tabs overflow."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let default = cx.new(|_| TabsState::new(terminal_tabs(4)));
            let overflow = cx.new(|_| TabsState::new(terminal_tabs(10)));
            let plain = cx.new(|_| {
                TabsState::new([
                    Tab::new("overview", "Overview"),
                    Tab::new("activity", "Activity"),
                    Tab::new("settings", "Settings"),
                ])
            });
            for state in [&default, &overflow, &plain] {
                cx.observe(state, |_, _, cx| cx.notify()).detach();
                cx.subscribe(state, |this: &mut Self, state, event, cx| {
                    if *event == TabsEvent::AddRequested {
                        this.added += 1;
                        let tab = Tab::new(
                            format!("added-{}", this.added),
                            format!("Tab {}", this.added),
                        )
                        .with_icon(IconName::SquareTerminal);
                        state.update(cx, |state, cx| state.push(tab, cx));
                    }
                })
                .detach();
            }
            Self {
                default,
                overflow,
                plain,
                added: 0,
            }
        })
        .into()
    }
}

impl Render for TabsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let bar = theme.metrics.title_bar;
        let background = theme.background();
        let panel = px(160.);
        let bar_and_panel = |tabs: Tabs, width: Option<Pixels>| {
            frame(bar + panel + px(2.), cx)
                .flex()
                .flex_col()
                .when_some(width, |this, width| this.w(width))
                .child(div().h(bar).w_full().child(tabs))
                .child(div().h(panel).w_full().bg(background))
        };
        page([
            section(
                "Default",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "Click a tab to select it. The selected tab shows its close control; \
                         an unselected tab shows it on hover. The + control adds a tab. With \
                         a tab focused, Left and Right select its neighbor.",
                        cx,
                    ))
                    .child(bar_and_panel(
                        Tabs::new("tabs-default", &self.default),
                        None,
                    )),
            )
            .into_any_element(),
            section(
                "Overflow",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "Ten tabs in a 480px bar: the tabs shrink to their minimum width, then \
                         the strip scrolls, and the scroll controls lead it.",
                        cx,
                    ))
                    .child(bar_and_panel(
                        Tabs::new("tabs-overflow", &self.overflow),
                        Some(px(480.)),
                    )),
            )
            .into_any_element(),
            section(
                "Without icons",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note("Tabs without icons, and without close controls.", cx))
                    .child(bar_and_panel(
                        Tabs::new("tabs-plain", &self.plain).closable(false),
                        None,
                    )),
            )
            .into_any_element(),
        ])
    }
}
