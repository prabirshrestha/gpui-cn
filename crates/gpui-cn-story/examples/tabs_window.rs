//! A plain window with a sidebar and a tab strip in the content's title
//! bar, to compare with the reference app.

use gpui_cn::{
    ActiveTheme as _, Button, Sidebar, SidebarCollapsible, SidebarGroup, SidebarLayout,
    SidebarMenuButton, SidebarState, SidebarTrigger, Tab, Tabs, TabsEvent, TabsState, TitleBar,
    gpui_kit::assets::IconName,
    gpui_kit::base::{Disableable as _, Selectable as _, transition},
};
use gpui_kit::*;

actions!(
    tabs_window,
    [
        /// Quits the example.
        Quit,
        /// Closes the selected tab, and the window once no tab is left.
        CloseTab
    ]
);

/// The window: a sidebar with a few rows, and a tab strip over the content.
pub struct Shell {
    sidebar: Entity<SidebarState>,
    tabs: Entity<TabsState>,
    added: usize,
}

impl Shell {
    /// The sidebar's state.
    pub fn sidebar(&self) -> &Entity<SidebarState> {
        &self.sidebar
    }

    /// The strip's state.
    pub fn tabs(&self) -> &Entity<TabsState> {
        &self.tabs
    }

    /// Four terminal tabs and a pinned list.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sidebar =
            cx.new(|cx| SidebarState::new(cx).with_collapsible(SidebarCollapsible::Offcanvas));
        let tabs = cx.new(|_| {
            TabsState::new(
                ["~", "~/app", "~/docs", "~/app"]
                    .into_iter()
                    .enumerate()
                    .map(|(n, label)| {
                        Tab::new(format!("t{n}"), label).with_icon(IconName::SquareTerminal)
                    }),
            )
        });
        cx.observe(&tabs, |_, _, cx| cx.notify()).detach();
        cx.observe(&sidebar, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&tabs, |this, tabs, event: &TabsEvent, cx| {
            if *event == TabsEvent::AddRequested {
                this.added += 1;
                let n = this.added;
                tabs.update(cx, |tabs, cx| {
                    tabs.push(
                        Tab::new(format!("new{n}"), format!("~/new/{n}"))
                            .with_icon(IconName::SquareTerminal),
                        cx,
                    )
                });
            }
        })
        .detach();
        Self {
            sidebar,
            tabs,
            added: 0,
        }
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (muted, gap, title_bar, window_controls, control, offset) = {
            let theme = cx.theme();
            (
                theme.muted_foreground(),
                theme.base.spacing.xs,
                theme.metrics.title_bar,
                theme.metrics.window_controls_inset,
                theme.metrics.control_md,
                theme.metrics.title_bar_content_offset,
            )
        };
        let open = self.sidebar.read(cx).is_open();
        // The trigger and the arrows float over the top-left corner, after
        // the window controls, so they stay put while the sidebar slides
        // away under them. The tabs start after them while it is closed.
        let controls_width = window_controls + control * 3. + gap * 3.;
        // The room follows the sidebar's slide, so the tabs stay clear of
        // the floating controls while it opens and closes.
        let leading = px(transition(
            "tabs-leading",
            if open { 0. } else { f32::from(controls_width) },
            gpui_cn::Theme::global(cx).motion.fold_transition(),
            window,
            cx,
        ));
        let controls = div()
            .id("shell-controls")
            .absolute()
            .top_0()
            .left_0()
            .h(title_bar)
            .pt(offset)
            .pl(window_controls)
            .flex()
            .items_center()
            .gap(gap)
            .child(SidebarTrigger::new("trigger", &self.sidebar))
            .child(
                Button::new("back")
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .accessibility_label("Back")
                    .disabled(true),
            )
            .child(
                Button::new("forward")
                    .ghost()
                    .icon(IconName::ArrowRight)
                    .accessibility_label("Forward")
                    .disabled(true),
            );
        let layout = SidebarLayout::new(&self.sidebar)
            .sidebar(
                Sidebar::new().header(TitleBar::new()).child(
                    SidebarGroup::new().label("Pinned").children(
                        ["app", "docs", "tools"]
                            .into_iter()
                            .enumerate()
                            .map(|(ix, name)| {
                                SidebarMenuButton::new(ElementId::from(("row", ix)))
                                    .icon(IconName::Folder)
                                    .label(name)
                                    .selected(ix == 0)
                            }),
                    ),
                ),
            )
            .child(
                TitleBar::new()
                    .inset(false)
                    .pl_0()
                    .pr_0()
                    .pt_0()
                    .child(Tabs::new("tabs", &self.tabs).with_leading(leading)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(muted)
                    .child("Content"),
            );
        div().relative().size_full().child(layout).child(controls)
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_cn::init(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-w", CloseTab, None),
            ]);
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.set_menus(vec![Menu {
                name: "tabs".into(),
                items: vec![
                    MenuItem::action("Close Tab", CloseTab),
                    MenuItem::action("Quit", Quit),
                ],
                disabled: false,
            }]);
            cx.activate(true);
            let bounds = Bounds::centered(None, size(px(1180.), px(720.)), cx);
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("tabs".into()),
                    ..TitleBar::title_bar_options(cx)
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..TitleBar::window_options(cx)
            };
            let (handle, shell) = gpui_kit::open_window(options, cx, |_, cx| cx.new(Shell::new))
                .expect("open the window");
            cx.on_action(move |_: &CloseTab, cx: &mut App| {
                let tabs = shell.read(cx).tabs.clone();
                let selected = tabs.read(cx).selected().cloned();
                match selected {
                    Some(id) => tabs.update(cx, |tabs, cx| tabs.remove(id, cx)),
                    None => {
                        let _ = cx.update_window(handle, |_, window, _| {
                            window.remove_window();
                        });
                    }
                }
            });
        });
}
