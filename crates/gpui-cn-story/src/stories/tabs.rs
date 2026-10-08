use gpui_cn::{
    ActiveTheme as _, Command, CommandEvent, CommandItem, CommandState, MenuItem, MenuState,
    Popover, Tab, Tabs, TabsEvent, TabsState, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, Global,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels, Render,
    SharedString, Styled as _, Window, actions, div, prelude::FluentBuilder as _, px,
};

use crate::{Story, frame, note, page, section};

actions!(
    tabs_story,
    [
        /// Opens a project tab.
        NewProject,
        /// Opens a file tab.
        NewFile,
        /// Opens a terminal tab.
        NewTerminal,
        /// Opens a browser tab.
        NewBrowser,
        /// Opens a changes tab.
        NewChanges,
        /// Opens a desktop tab.
        NewDesktop,
        /// Opens a subscriptions tab.
        NewSubscriptions,
    ]
);

/// The key context of the new-tab menu demo, where its shortcuts are
/// bound, so they work inside it and show in its menu.
const CONTEXT: &str = "TabsStory";

/// Marks that the story's key bindings are in, so a second visit does not
/// bind them again.
struct Bindings;

impl Global for Bindings {}

/// Binds the story's shortcuts, once.
fn bind_keys(cx: &mut App) {
    if cx.has_global::<Bindings>() {
        return;
    }
    let keys = if cfg!(target_os = "macos") {
        ["cmd-g", "cmd-j", "cmd-shift-b", "cmd-e"]
    } else {
        ["ctrl-g", "ctrl-j", "ctrl-shift-b", "ctrl-e"]
    };
    cx.bind_keys([
        KeyBinding::new(keys[0], NewFile, Some(CONTEXT)),
        KeyBinding::new(keys[1], NewTerminal, Some(CONTEXT)),
        KeyBinding::new(keys[2], NewBrowser, Some(CONTEXT)),
        KeyBinding::new(keys[3], NewChanges, Some(CONTEXT)),
    ]);
    cx.set_global(Bindings);
}

/// What the new-tab menu offers: each kind's label and icon, in menu
/// order. [`TabsStory::kind_items`] pairs them with their actions.
const KINDS: [(&str, IconName); 7] = [
    ("Project", IconName::LayoutDashboard),
    ("File", IconName::File),
    ("Terminal", IconName::SquareTerminal),
    ("Browser", IconName::Globe),
    ("Changes", IconName::FileText),
    ("Desktop", IconName::PanelBottom),
    ("Subscriptions", IconName::Bell),
];

/// The tab strip over a content panel, so the selected tab's shoulders
/// read as one surface with the content.
pub struct TabsStory {
    default: Entity<TabsState>,
    overflow: Entity<TabsState>,
    plain: Entity<TabsState>,
    /// Tabs whose new-tab control opens a searchable menu of kinds.
    menu_tabs: Entity<TabsState>,
    /// The palette of what to add, in a popover under the + control.
    add_command: Entity<CommandState>,
    add_open: bool,
    /// The demo's own focus, which the palette's actions go to.
    focus: FocusHandle,
    /// The number of tabs the new-tab controls have added, for their labels.
    added: usize,
    /// The menu a right click on a default tab opens.
    tab_menu: Entity<MenuState>,
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

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        bind_keys(cx);
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
            let menu_tabs = cx.new(|_| {
                TabsState::new(
                    [Tab::new("project", "Project").with_icon(IconName::LayoutDashboard)],
                )
            });
            cx.observe(&menu_tabs, |_, _, cx| cx.notify()).detach();
            let focus = cx.focus_handle();
            let add_command = cx.new(|cx| {
                CommandState::new("Open any file, URL, ...", window, cx)
                    .with_entries(Self::kind_items(&focus))
            });
            cx.observe(&add_command, |_, _, cx| cx.notify()).detach();
            cx.subscribe(&add_command, |this: &mut Self, _, event, cx| match event {
                CommandEvent::Confirmed(_) => {
                    this.add_open = false;
                    cx.notify();
                }
                CommandEvent::Submitted(query) => {
                    this.add_open = false;
                    this.add(query.clone(), IconName::Globe, cx);
                }
                CommandEvent::QueryChanged(_) => {}
            })
            .detach();
            Self {
                default,
                overflow,
                plain,
                menu_tabs,
                add_command,
                add_open: false,
                focus,
                added: 0,
                tab_menu: cx.new(MenuState::new),
            }
        })
        .into()
    }
}

impl TabsStory {
    /// Adds a tab to the menu demo and selects it.
    fn add(&mut self, label: SharedString, icon: IconName, cx: &mut Context<Self>) {
        self.added += 1;
        let tab = Tab::new(format!("added-{}", self.added), label).with_icon(icon);
        self.menu_tabs.update(cx, |state, cx| {
            let id = tab.id().clone();
            state.push(tab, cx);
            state.select(id, cx);
        });
    }

    /// Adds a tab of the kind at `index` of [`KINDS`].
    fn add_kind(&mut self, index: usize, cx: &mut Context<Self>) {
        let (label, icon) = KINDS[index];
        self.add(label.into(), icon, cx);
    }

    /// The new-tab menu's rows: each kind with the action that adds it,
    /// sent to the demo, whose bindings the rows show.
    fn kind_items(focus: &FocusHandle) -> Vec<CommandItem> {
        let item = |index: usize| {
            let (label, icon) = KINDS[index];
            CommandItem::new(label.to_lowercase(), label)
                .icon(icon)
                .action_context(focus)
        };
        vec![
            item(0).action(NewProject),
            item(1).action(NewFile),
            item(2).action(NewTerminal),
            item(3).action(NewBrowser),
            item(4).action(NewChanges),
            item(5).action(NewDesktop),
            item(6).action(NewSubscriptions),
        ]
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
                         a tab focused, Left and Right select its neighbor. A double click \
                         renames a tab in place, and a right click opens its menu.",
                        cx,
                    ))
                    .child(bar_and_panel(
                        {
                            let state = self.default.clone();
                            Tabs::new("tabs-default", &self.default)
                                .renamable(true)
                                .context_menu(&self.tab_menu, move |id, _, _| {
                                    let rename = (state.clone(), id.clone());
                                    let close = (state.clone(), id.clone());
                                    vec![
                                        MenuItem::new("rename", "Rename Tab...")
                                            .on_select(move |window, cx| {
                                                let (state, id) = rename.clone();
                                                state.update(cx, |tabs, cx| {
                                                    tabs.start_rename(id, window, cx)
                                                });
                                            })
                                            .into(),
                                        MenuItem::new("close", "Close Tab")
                                            .on_select(move |_, cx| {
                                                let (state, id) = close.clone();
                                                state.update(cx, |tabs, cx| tabs.remove(id, cx));
                                            })
                                            .into(),
                                    ]
                                })
                        },
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
            section(
                "New-tab menu",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "The + control opens a command palette in a popover. Type to \
                         narrow the commands; Enter adds the highlighted kind, or opens what \
                         you typed in a browser tab when nothing matches. The shortcuts \
                         work while the demo has focus.",
                        cx,
                    ))
                    .child(
                        div()
                            .key_context(CONTEXT)
                            .track_focus(&self.focus)
                            .on_action(
                                cx.listener(|this, _: &NewProject, _, cx| this.add_kind(0, cx)),
                            )
                            .on_action(cx.listener(|this, _: &NewFile, _, cx| this.add_kind(1, cx)))
                            .on_action(
                                cx.listener(|this, _: &NewTerminal, _, cx| this.add_kind(2, cx)),
                            )
                            .on_action(
                                cx.listener(|this, _: &NewBrowser, _, cx| this.add_kind(3, cx)),
                            )
                            .on_action(
                                cx.listener(|this, _: &NewChanges, _, cx| this.add_kind(4, cx)),
                            )
                            .on_action(
                                cx.listener(|this, _: &NewDesktop, _, cx| this.add_kind(5, cx)),
                            )
                            .on_action(
                                cx.listener(|this, _: &NewSubscriptions, _, cx| {
                                    this.add_kind(6, cx)
                                }),
                            )
                            .child(bar_and_panel(
                                Tabs::new("tabs-menu", &self.menu_tabs).add_trigger({
                                    let command = self.add_command.clone();
                                    let command_focus = command.focus_handle(cx);
                                    let open = self.add_open;
                                    let this = cx.entity().downgrade();
                                    move |button| {
                                        Popover::new("tabs-menu-add")
                                            .trigger(button)
                                            .open(open)
                                            .on_open_change(move |open, _, cx| {
                                                this.update(cx, |this, cx| {
                                                    this.add_open = open;
                                                    cx.notify();
                                                })
                                                .ok();
                                            })
                                            .track_focus(&command_focus)
                                            .content(move |_, _| {
                                                Command::new("tabs-menu-command", &command)
                                                    .bordered(false)
                                            })
                                    }
                                }),
                                None,
                            )),
                    ),
            )
            .into_any_element(),
        ])
    }
}
