use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use std::path::PathBuf;

use gpui_cn::prelude::{Disableable as _, StyledExt as _};
use gpui_cn::terminal::{
    FixtureSource, LocalTerminalOptions, Terminal, TerminalColors, TerminalConfig, TerminalEvent,
    TerminalState, WorkingDirectory, actions,
};
use gpui_cn::{
    ActiveTheme as _, Button, ContextMenu, Dialog, MenuEntry, MenuItem, MenuState, Select,
    SelectEvent, SelectItem, SelectState, Tab, Tabs, TabsEvent, TabsState, Theme, ThemeScope,
    ThemeTokens, gpui_kit::assets::IconName,
};
use gpui_kit::{
    Action, AnyView, App, AppContext as _, Axis, Context, ElementId, Entity, Global,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    Styled as _, WeakEntity, Window, actions, div, prelude::FluentBuilder as _, px,
};

use crate::{Story, frame, note, page, section};

actions!(
    terminal_story,
    [
        /// Opens a tab with a new shell.
        NewTab,
        /// Selects the next tab.
        NextTab,
        /// Selects the previous tab.
        PreviousTab,
        /// Splits the focused pane, the new one on the right.
        SplitRight,
        /// Splits the focused pane, the new one below.
        SplitDown,
        /// Splits the focused pane, the new one on the left.
        SplitLeft,
        /// Splits the focused pane, the new one above.
        SplitUp,
        /// Focuses the pane on the left.
        FocusLeft,
        /// Focuses the pane on the right.
        FocusRight,
        /// Focuses the pane above.
        FocusUp,
        /// Focuses the pane below.
        FocusDown,
        /// Focuses the next pane of the tab.
        FocusNext,
        /// Closes the focused pane, and its tab with the last pane.
        ClosePane,
        /// Sends the leader key itself to the program.
        SendLeader,
        /// Makes the focused pane fill its tab, or puts the tab's layout
        /// back.
        TogglePaneZoom,
    ]
);

/// Selects a tab by number; the first tab is 1.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = terminal_story, no_json)]
pub struct SelectTab(pub usize);

/// The key context the story's bindings are in, so they work only while a
/// pane of the story has focus.
const CONTEXT: &str = "TerminalStory";

/// The tmux-style leader key.
const LEADER: &str = "ctrl-a";

/// What a fixture pane shows: a prompt and a listing, in color.
const FIXTURE: &str = "\x1b[1;32m~/gpui-cn\x1b[0m $ ls\r\n\
    \x1b[1;34mcrates\x1b[0m  \x1b[1;34mscripts\x1b[0m  \x1b[1;34mvendor\x1b[0m  \
    AGENTS.md  Cargo.toml  README.md\r\n\
    \x1b[1;32m~/gpui-cn\x1b[0m $ ";

/// Marks that the story's key bindings are in.
struct Bindings;

impl Global for Bindings {}

/// Makes every pane a `FixtureSource` instead of a shell, so an offscreen
/// render or a benchmark draws the same thing on every run.
struct Fixtures;

impl Global for Fixtures {}

/// Binds the terminal's default keys, the leader keys and, on macOS, the
/// Command shortcuts, once.
fn bind_keys(cx: &mut App) {
    if cx.has_global::<Bindings>() {
        return;
    }
    let context = Some(CONTEXT);
    let leader = |key: &str| format!("{LEADER} {key}");
    let mut bindings = vec![
        KeyBinding::new(&leader("c"), NewTab, context),
        KeyBinding::new(&leader("n"), NextTab, context),
        KeyBinding::new(&leader("p"), PreviousTab, context),
        KeyBinding::new(&leader("|"), SplitRight, context),
        KeyBinding::new(&leader("\\"), SplitRight, context),
        KeyBinding::new(&leader("%"), SplitRight, context),
        KeyBinding::new(&leader("-"), SplitDown, context),
        KeyBinding::new(&leader("\""), SplitDown, context),
        KeyBinding::new(&leader("h"), FocusLeft, context),
        KeyBinding::new(&leader("j"), FocusDown, context),
        KeyBinding::new(&leader("k"), FocusUp, context),
        KeyBinding::new(&leader("l"), FocusRight, context),
        KeyBinding::new(&leader("o"), FocusNext, context),
        KeyBinding::new(&leader("x"), ClosePane, context),
        KeyBinding::new(&leader("z"), TogglePaneZoom, context),
        KeyBinding::new(&leader(LEADER), SendLeader, context),
    ];
    for number in 1..=9 {
        bindings.push(KeyBinding::new(
            &leader(&number.to_string()),
            SelectTab(number),
            context,
        ));
    }
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-t", NewTab, context),
            KeyBinding::new("cmd-w", ClosePane, context),
            KeyBinding::new("cmd-d", SplitRight, context),
            KeyBinding::new("cmd-shift-d", SplitDown, context),
            // Ghostty's toggle_split_zoom.
            KeyBinding::new("cmd-shift-enter", TogglePaneZoom, context),
            KeyBinding::new("cmd-shift-]", NextTab, context),
            KeyBinding::new("cmd-shift-[", PreviousTab, context),
            KeyBinding::new("cmd-alt-left", FocusLeft, context),
            KeyBinding::new("cmd-alt-right", FocusRight, context),
            KeyBinding::new("cmd-alt-up", FocusUp, context),
            KeyBinding::new("cmd-alt-down", FocusDown, context),
        ]);
        for number in 1..=9 {
            bindings.push(KeyBinding::new(
                &format!("cmd-{number}"),
                SelectTab(number),
                context,
            ));
        }
    }
    cx.bind_keys(gpui_cn::terminal::default_key_bindings());
    cx.bind_keys(bindings);
    cx.set_global(Bindings);
}

/// A pane's identity, which names its element and its terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PaneId(u64);

impl PaneId {
    fn element_id(self) -> ElementId {
        ElementId::NamedInteger("terminal-pane".into(), self.0)
    }
}

/// A tab's layout: a pane, or panes side by side along an axis.
#[derive(Debug, PartialEq)]
enum Node {
    Leaf(PaneId),
    Split { axis: Axis, children: Vec<Node> },
}

impl Node {
    fn first_leaf(&self) -> PaneId {
        match self {
            Node::Leaf(id) => *id,
            Node::Split { children, .. } => children[0].first_leaf(),
        }
    }

    fn last_leaf(&self) -> PaneId {
        match self {
            Node::Leaf(id) => *id,
            Node::Split { children, .. } => children[children.len() - 1].last_leaf(),
        }
    }

    fn leaves(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<PaneId>) {
        match self {
            Node::Leaf(id) => out.push(*id),
            Node::Split { children, .. } => children.iter().for_each(|c| c.collect(out)),
        }
    }

    /// The child indexes from the root to `target`.
    fn path(&self, target: PaneId, path: &mut Vec<usize>) -> bool {
        match self {
            Node::Leaf(id) => *id == target,
            Node::Split { children, .. } => {
                for (index, child) in children.iter().enumerate() {
                    path.push(index);
                    if child.path(target, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
        }
    }

    fn at(&self, path: &[usize]) -> &Node {
        match (self, path) {
            (Node::Split { children, .. }, [index, rest @ ..]) => children[*index].at(rest),
            (node, _) => node,
        }
    }

    /// Puts `new` beside `target` along `axis`: after it, or before it
    /// with `before`.
    fn split(&mut self, target: PaneId, axis: Axis, new: PaneId, before: bool) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                let pair = if before {
                    vec![Node::Leaf(new), Node::Leaf(target)]
                } else {
                    vec![Node::Leaf(target), Node::Leaf(new)]
                };
                *self = Node::Split {
                    axis,
                    children: pair,
                };
                true
            }
            Node::Leaf(_) => false,
            Node::Split {
                axis: own,
                children,
            } => {
                if *own == axis
                    && let Some(index) = children
                        .iter()
                        .position(|c| matches!(c, Node::Leaf(id) if *id == target))
                {
                    children.insert(index + usize::from(!before), Node::Leaf(new));
                    return true;
                }
                children
                    .iter_mut()
                    .any(|child| child.split(target, axis, new, before))
            }
        }
    }

    /// Removes `target` and collapses a split left with one child. False
    /// when the root itself is the target.
    fn remove(&mut self, target: PaneId) -> bool {
        let Node::Split { children, .. } = self else {
            return false;
        };
        if let Some(index) = children
            .iter()
            .position(|c| matches!(c, Node::Leaf(id) if *id == target))
        {
            children.remove(index);
        } else if !children.iter_mut().any(|child| child.remove(target)) {
            return false;
        }
        if children.len() == 1 {
            *self = children.remove(0);
        }
        true
    }

    /// The pane beside `from` in a direction: up the path to the nearest
    /// split along `axis` with a sibling on that side.
    fn neighbor(&self, from: PaneId, axis: Axis, forward: bool) -> Option<PaneId> {
        let mut path = Vec::new();
        if !self.path(from, &mut path) {
            return None;
        }
        while let Some(index) = path.pop() {
            let Node::Split {
                axis: own,
                children,
            } = self.at(&path)
            else {
                continue;
            };
            if *own != axis {
                continue;
            }
            let sibling = if forward {
                index.checked_add(1)
            } else {
                index.checked_sub(1)
            };
            if let Some(node) = sibling.and_then(|sibling| children.get(sibling)) {
                return Some(if forward {
                    node.first_leaf()
                } else {
                    node.last_leaf()
                });
            }
        }
        None
    }
}

/// One tab: its layout and the pane that has, or last had, focus.
struct TabPanes {
    root: Node,
    focused: PaneId,
    /// Whether the focused pane fills the tab, the others hidden.
    zoomed: bool,
}

/// Where Ghostty looks for theme files, in its order: the user's config
/// directory, then the application's resources, as Ghostty's
/// `src/config/theme.zig` does. A test can name its own directories.
fn theme_dirs(cx: &App) -> Vec<PathBuf> {
    if let Some(ThemeDirs(dirs)) = cx.try_global::<ThemeDirs>() {
        return dirs.clone();
    }
    let mut dirs = Vec::new();
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    if let Some(config) = config {
        dirs.push(config.join("ghostty/themes"));
    }
    let resources = std::env::var_os("GHOSTTY_RESOURCES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(if cfg!(target_os = "macos") {
                "/Applications/Ghostty.app/Contents/Resources/ghostty"
            } else {
                "/usr/share/ghostty"
            })
        });
    dirs.push(resources.join("themes"));
    dirs
}

/// Theme directories a test uses instead of the machine's.
struct ThemeDirs(Vec<PathBuf>);

impl Global for ThemeDirs {}

/// The Ghostty theme names on this machine, sorted, without duplicates.
fn installed_themes(cx: &App) -> Vec<String> {
    let mut names: Vec<String> = theme_dirs(cx)
        .into_iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

/// Reads the Ghostty theme `name` from the first theme directory that has
/// it.
fn read_theme(name: &str, cx: &App) -> Option<TerminalColors> {
    theme_dirs(cx)
        .into_iter()
        .find_map(|dir| std::fs::read_to_string(dir.join(name)).ok())
        .and_then(|text| TerminalColors::from_ghostty_theme(&text).ok())
}

/// The value of the theme picker's first row: Ghostty's built-in colors.
const DEFAULT_THEME: &str = "";

/// What a close is for, kept while the story asks to confirm it.
#[derive(Clone, Debug)]
enum Closing {
    Pane(PaneId),
    Tabs(Vec<SharedString>),
}

/// A close waiting for the user, and the programs it would end.
struct PendingClose {
    closing: Closing,
    running: Vec<String>,
}

/// Terminal tabs with splits and tmux-style leader keys, each pane a shell.
pub struct TerminalStory {
    tabs: Entity<TabsState>,
    /// The Ghostty theme every pane and the tab strip draw with.
    colors: TerminalColors,
    themes: Entity<SelectState<SharedString>>,
    layouts: HashMap<SharedString, TabPanes>,
    panes: HashMap<PaneId, Entity<TerminalState>>,
    /// Tabs the user named. The others follow their focused pane's title.
    named: HashSet<SharedString>,
    /// Set while the story itself renames a tab to a title, so the
    /// `Renamed` it causes is not taken for the user's.
    titling: bool,
    pane_menu: Entity<MenuState>,
    tab_menu: Entity<MenuState>,
    /// A close that would end a running program, while the story asks.
    pending_close: Option<PendingClose>,
    /// Whether the shortcuts dialog shows.
    shortcuts_open: bool,
    /// The story itself, for menus built when they open.
    this: WeakEntity<Self>,
    next_pane: u64,
    next_tab: u64,
}

impl Story for TerminalStory {
    fn title() -> &'static str {
        "Terminal"
    }

    fn icon() -> IconName {
        IconName::SquareTerminal
    }

    fn description() -> &'static str {
        "A terminal on libghostty-vt, with tabs, splits, and tmux-style leader keys."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        bind_keys(cx);
        cx.new(|cx| {
            // A fixture run lists no themes, so its pictures do not depend
            // on the machine, unless a test names its own directories.
            let names = if cx.has_global::<Fixtures>() && !cx.has_global::<ThemeDirs>() {
                Vec::new()
            } else {
                installed_themes(cx)
            };
            let themes = cx.new(|cx| {
                SelectState::new(
                    std::iter::once(SelectItem::new(
                        SharedString::from(DEFAULT_THEME),
                        "Ghostty default",
                    ))
                    .chain(
                        names
                            .into_iter()
                            .map(|name| SelectItem::new(SharedString::from(name.clone()), name)),
                    ),
                    cx,
                )
                .with_selected([SharedString::from(DEFAULT_THEME)])
                .with_search("Search themes", window, cx)
            });
            cx.observe(&themes, |_, _, cx| cx.notify()).detach();
            cx.subscribe(&themes, |this: &mut Self, _, event, cx| {
                if let SelectEvent::Changed(names) = event
                    && let Some(name) = names.first()
                {
                    let colors = if name.is_empty() {
                        TerminalColors::default()
                    } else {
                        read_theme(name, cx).unwrap_or_default()
                    };
                    this.set_colors(colors, cx);
                }
            })
            .detach();
            let tabs = cx.new(|_| TabsState::new([]));
            cx.observe(&tabs, |_, _, cx| cx.notify()).detach();
            cx.subscribe_in(
                &tabs,
                window,
                |this: &mut Self, _, event, window, cx| match event {
                    TabsEvent::Selected(_) => this.show_selected(window, cx),
                    TabsEvent::Closed(id) => this.tab_closed(id, window, cx),
                    TabsEvent::CloseRequested(id) => {
                        this.request_close(Closing::Tabs(vec![id.clone()]), window, cx);
                    }
                    TabsEvent::AddRequested => this.new_tab(window, cx),
                    TabsEvent::Renamed(id) if !this.titling => {
                        this.named.insert(id.clone());
                    }
                    _ => {}
                },
            )
            .detach();
            let mut story = Self {
                tabs,
                colors: TerminalColors::default(),
                themes,
                layouts: HashMap::new(),
                panes: HashMap::new(),
                named: HashSet::new(),
                titling: false,
                pane_menu: cx.new(MenuState::new),
                tab_menu: cx.new(MenuState::new),
                pending_close: None,
                shortcuts_open: false,
                this: cx.weak_entity(),
                next_pane: 0,
                next_tab: 0,
            };
            story.set_colors(TerminalColors::default(), cx);
            story.new_tab(window, cx);
            story
        })
        .into()
    }
}

impl TerminalStory {
    /// The theme scope the tab strip and the panes draw in.
    pub const SCOPE: &'static str = "terminal-story";

    /// Draws every pane and the tab strip with `colors`: the panes take
    /// them as their terminal colors, and the strip a gpui-cn theme derived
    /// from them, so the two read as one surface.
    pub fn set_colors(&mut self, colors: TerminalColors, cx: &mut Context<Self>) {
        let config = colors.theme_config(Theme::global(cx));
        Theme::set_scope(cx, Self::SCOPE, config);
        for terminal in self.panes.values() {
            let colors = colors.clone();
            terminal.update(cx, |terminal, cx| terminal.set_colors(colors, cx));
        }
        self.colors = colors;
        cx.notify();
    }

    /// The colors the panes draw with.
    pub fn colors(&self) -> &TerminalColors {
        &self.colors
    }

    /// Every pane's terminal, in no order.
    pub fn terminals(&self) -> Vec<Entity<TerminalState>> {
        self.panes.values().cloned().collect()
    }

    /// The tokens the tab strip draws with.
    fn tokens<'a>(&self, cx: &'a App) -> &'a ThemeTokens {
        Theme::global(cx).scope(Self::SCOPE).unwrap_or(cx.theme())
    }

    /// Makes every pane opened from now on render canned output instead of
    /// running a shell. The snapshot and benchmark binaries call it, so
    /// their pictures and numbers do not depend on the user's shell.
    pub fn use_fixtures(cx: &mut App) {
        cx.set_global(Fixtures);
    }

    /// Makes the theme picker list the Ghostty themes in `dirs` instead of
    /// the machine's, for a test.
    pub fn use_theme_dirs(dirs: impl IntoIterator<Item = PathBuf>, cx: &mut App) {
        cx.set_global(ThemeDirs(dirs.into_iter().collect()));
    }

    /// The tab strip's state.
    pub fn tabs(&self) -> &Entity<TabsState> {
        &self.tabs
    }

    /// The theme picker's state.
    pub fn themes(&self) -> &Entity<SelectState<SharedString>> {
        &self.themes
    }

    /// The tab that holds pane `id`.
    fn tab_of(&self, id: PaneId) -> Option<SharedString> {
        self.layouts
            .iter()
            .find(|(_, layout)| layout.root.leaves().contains(&id))
            .map(|(tab, _)| tab.clone())
    }

    /// Names tab `tab` after its focused pane's title, unless the user
    /// named it.
    fn follow_title(&mut self, tab: &SharedString, cx: &mut Context<Self>) {
        if self.named.contains(tab) {
            return;
        }
        let Some(title) = self
            .layouts
            .get(tab)
            .and_then(|layout| self.panes.get(&layout.focused))
            .map(|terminal| terminal.read(cx).title().clone())
            .filter(|title| !title.is_empty())
        else {
            return;
        };
        self.titling = true;
        let id = tab.clone();
        self.tabs.update(cx, |tabs, cx| tabs.rename(id, title, cx));
        self.titling = false;
    }

    fn selected_tab(&self, cx: &App) -> Option<SharedString> {
        self.tabs.read(cx).selected().cloned()
    }

    fn focused_terminal(&self, cx: &App) -> Option<&Entity<TerminalState>> {
        let tab = self.selected_tab(cx)?;
        self.panes.get(&self.layouts.get(&tab)?.focused)
    }

    /// A new pane, a shell started in the focused pane's directory.
    fn spawn_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> PaneId {
        let id = PaneId(self.next_pane);
        self.next_pane += 1;
        let cwd = self
            .focused_terminal(cx)
            .and_then(|terminal| terminal.read(cx).cwd().map(Into::into))
            .map_or(WorkingDirectory::Home, WorkingDirectory::Path);
        let fixtures = cx.has_global::<Fixtures>();
        let config = TerminalConfig::default().with_colors(self.colors.clone());
        let terminal = cx.new(|cx| {
            if fixtures {
                TerminalState::new(FixtureSource::new(FIXTURE), config, window, cx)
            } else {
                TerminalState::local(
                    LocalTerminalOptions::default().with_cwd(cwd),
                    config,
                    window,
                    cx,
                )
            }
        });
        cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&terminal, window, move |this, _, event, window, cx| {
            match event {
                // A shell that ended normally takes its pane with it. One
                // that failed or ended at once stays, so its status shows.
                TerminalEvent::Exited(status) if !status.is_abnormal() => {
                    this.close_pane(id, window, cx);
                }
                // A click focuses a pane too, and the splits and the tab's
                // name follow the pane with focus.
                TerminalEvent::Focused => {
                    if let Some(tab) = this.tab_of(id)
                        && let Some(layout) = this.layouts.get_mut(&tab)
                    {
                        layout.focused = id;
                        this.follow_title(&tab, cx);
                        cx.notify();
                    }
                }
                TerminalEvent::TitleChanged => {
                    if let Some(tab) = this.tab_of(id)
                        && this.layouts.get(&tab).is_some_and(|l| l.focused == id)
                    {
                        this.follow_title(&tab, cx);
                    }
                }
                _ => {}
            }
        })
        .detach();
        self.panes.insert(id, terminal);
        id
    }

    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.spawn_pane(window, cx);
        self.next_tab += 1;
        let tab_id = SharedString::from(format!("shell-{}", self.next_tab));
        self.layouts.insert(
            tab_id.clone(),
            TabPanes {
                root: Node::Leaf(pane),
                focused: pane,
                zoomed: false,
            },
        );
        let tab = Tab::new(tab_id.clone(), format!("Shell {}", self.next_tab))
            .with_icon(IconName::SquareTerminal);
        self.tabs.update(cx, |tabs, cx| {
            tabs.push(tab, cx);
            tabs.select(tab_id, cx);
        });
        self.show_selected(window, cx);
    }

    /// Shows the selected tab's panes, hides the others so their engines
    /// stop building frames, and focuses the selected tab's pane.
    fn show_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selected) = self.selected_tab(cx) else {
            return;
        };
        // A zoomed tab shows its focused pane only. The others keep their
        // programs and drain their output, but paint nothing, and may park.
        for (tab, layout) in &self.layouts {
            for pane in layout.root.leaves() {
                let visible = *tab == selected && (!layout.zoomed || pane == layout.focused);
                if let Some(terminal) = self.panes.get(&pane) {
                    terminal.update(cx, |terminal, cx| terminal.set_visible(visible, cx));
                }
            }
        }
        if let Some(focused) = self.layouts.get(&selected).map(|layout| layout.focused) {
            self.focus_pane(focused, window, cx);
        }
    }

    fn focus_pane(&mut self, id: PaneId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.selected_tab(cx)
            && let Some(layout) = self.layouts.get_mut(&tab)
        {
            layout.focused = id;
        }
        if let Some(terminal) = self.panes.get(&id) {
            let handle = terminal.read(cx).focus_handle().clone();
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let id = self
            .tabs
            .read(cx)
            .tabs()
            .get(index)
            .map(|tab| tab.id().clone());
        if let Some(id) = id {
            self.tabs.update(cx, |tabs, cx| tabs.select(id, cx));
        }
    }

    /// Zooms the selected tab's focused pane, or unzooms it. A tab with one
    /// pane has nothing to zoom.
    fn toggle_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.selected_tab(cx) else {
            return;
        };
        let zoomed = match self.layouts.get_mut(&tab) {
            Some(layout) if matches!(layout.root, Node::Split { .. }) => {
                layout.zoomed = !layout.zoomed;
                layout.zoomed
            }
            _ => return,
        };
        self.mark_zoom(&tab, zoomed, cx);
        self.show_selected(window, cx);
    }

    /// Puts tab `tab`'s layout back if a pane is zoomed. Splitting, closing
    /// and moving focus do this first, as in tmux. Returns whether it did.
    fn unzoom(&mut self, tab: &SharedString, cx: &mut Context<Self>) -> bool {
        match self.layouts.get_mut(tab) {
            Some(layout) if layout.zoomed => {
                layout.zoomed = false;
                self.mark_zoom(tab, false, cx);
                true
            }
            _ => false,
        }
    }

    /// The tab's icon marks a zoomed pane.
    fn mark_zoom(&mut self, tab: &SharedString, zoomed: bool, cx: &mut Context<Self>) {
        let icon = if zoomed {
            IconName::Maximize
        } else {
            IconName::SquareTerminal
        };
        let id = tab.clone();
        self.tabs
            .update(cx, |tabs, cx| tabs.set_icon(id, Some(icon), cx));
        cx.notify();
    }

    /// Whether pane `id`'s tab has a zoomed pane.
    fn zoomed(&self, id: PaneId) -> bool {
        self.tab_of(id)
            .and_then(|tab| self.layouts.get(&tab))
            .is_some_and(|layout| layout.zoomed)
    }

    fn split(&mut self, axis: Axis, before: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.selected_tab(cx) else {
            return;
        };
        let Some(target) = self.layouts.get(&tab).map(|layout| layout.focused) else {
            return;
        };
        self.unzoom(&tab, cx);
        let pane = self.spawn_pane(window, cx);
        if let Some(layout) = self.layouts.get_mut(&tab)
            && layout.root.split(target, axis, pane, before)
        {
            self.focus_pane(pane, window, cx);
        }
    }

    fn focus_direction(
        &mut self,
        axis: Axis,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.selected_tab(cx)
            && self.unzoom(&tab, cx)
        {
            self.show_selected(window, cx);
        }
        let next = self
            .selected_tab(cx)
            .and_then(|tab| self.layouts.get(&tab))
            .and_then(|layout| layout.root.neighbor(layout.focused, axis, forward));
        if let Some(next) = next {
            self.focus_pane(next, window, cx);
        }
    }

    fn focus_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.selected_tab(cx)
            && self.unzoom(&tab, cx)
        {
            self.show_selected(window, cx);
        }
        let next = self
            .selected_tab(cx)
            .and_then(|tab| self.layouts.get(&tab))
            .map(|layout| {
                let leaves = layout.root.leaves();
                let at = leaves.iter().position(|id| *id == layout.focused);
                leaves[at.map_or(0, |at| (at + 1) % leaves.len())]
            });
        if let Some(next) = next {
            self.focus_pane(next, window, cx);
        }
    }

    fn close_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self
            .selected_tab(cx)
            .and_then(|tab| self.layouts.get(&tab))
            .map(|layout| layout.focused);
        if let Some(focused) = focused {
            self.request_close(Closing::Pane(focused), window, cx);
        }
    }

    /// The programs other than the shell that closing `closing` would end,
    /// by name, as each pane's foreground process reports them.
    fn running(&self, closing: &Closing, cx: &App) -> Vec<String> {
        let panes = match closing {
            Closing::Pane(pane) => vec![*pane],
            Closing::Tabs(tabs) => tabs
                .iter()
                .filter_map(|tab| self.layouts.get(tab))
                .flat_map(|layout| layout.root.leaves())
                .collect(),
        };
        panes
            .iter()
            .filter_map(|pane| self.panes.get(pane))
            .filter_map(|terminal| {
                let terminal = terminal.read(cx);
                let process = terminal.foreground()?;
                (terminal.is_live() && !process.is_shell()).then(|| process.name().to_owned())
            })
            .collect()
    }

    /// Closes at once when only shells would end, and asks first when a
    /// pane runs another program, as Ghostty does.
    fn request_close(&mut self, closing: Closing, window: &mut Window, cx: &mut Context<Self>) {
        let running = self.running(&closing, cx);
        if running.is_empty() {
            self.close(closing, window, cx);
        } else {
            self.pending_close = Some(PendingClose { closing, running });
            cx.notify();
        }
    }

    fn close(&mut self, closing: Closing, window: &mut Window, cx: &mut Context<Self>) {
        match closing {
            Closing::Pane(pane) => self.close_pane(pane, window, cx),
            Closing::Tabs(tabs) => self.tabs.update(cx, |state, cx| {
                for tab in tabs {
                    state.remove(tab, cx);
                }
            }),
        }
    }

    fn resolve_close(&mut self, confirmed: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_close.take() else {
            return;
        };
        if confirmed {
            self.close(pending.closing, window, cx);
        }
        cx.notify();
    }

    /// Stops a pane's program and takes it out of its tab. The last pane
    /// takes its tab with it, and the last tab is replaced by a new one.
    fn close_pane(&mut self, id: PaneId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(terminal) = self.panes.remove(&id) else {
            return;
        };
        terminal.update(cx, |terminal, cx| terminal.close(cx));
        let Some(tab) = self
            .layouts
            .iter()
            .find(|(_, layout)| layout.root.leaves().contains(&id))
            .map(|(tab, _)| tab.clone())
        else {
            return;
        };
        self.unzoom(&tab, cx);
        let layout = self.layouts.get_mut(&tab).expect("found above");
        if !layout.root.remove(id) {
            // The removal is reported back through `TabsEvent::Closed`.
            self.tabs.update(cx, |tabs, cx| tabs.remove(tab, cx));
            return;
        }
        if layout.focused == id {
            layout.focused = layout.root.first_leaf();
        }
        if self.selected_tab(cx) == Some(tab) {
            self.show_selected(window, cx);
        }
        cx.notify();
    }

    fn tab_closed(&mut self, id: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layout) = self.layouts.remove(id) {
            for pane in layout.root.leaves() {
                if let Some(terminal) = self.panes.remove(&pane) {
                    terminal.update(cx, |terminal, cx| terminal.close(cx));
                }
            }
        }
        if self.tabs.read(cx).tabs().is_empty() {
            self.new_tab(window, cx);
        } else {
            self.show_selected(window, cx);
        }
    }

    /// Closes every tab but `keep`, or only the tabs after it with
    /// `right_only`, asking first when one runs a program.
    fn close_others(
        &mut self,
        keep: &SharedString,
        right_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<SharedString> = self
            .tabs
            .read(cx)
            .tabs()
            .iter()
            .map(|tab| tab.id().clone())
            .collect();
        let Some(at) = ids.iter().position(|id| id == keep) else {
            return;
        };
        let doomed: Vec<SharedString> = ids
            .into_iter()
            .enumerate()
            .filter(|(index, _)| {
                if right_only {
                    *index > at
                } else {
                    *index != at
                }
            })
            .map(|(_, id)| id)
            .collect();
        self.tabs
            .update(cx, |tabs, cx| tabs.select(keep.clone(), cx));
        self.request_close(Closing::Tabs(doomed), window, cx);
    }

    /// The rows of a tab's context menu.
    fn tab_entries(story: &WeakEntity<Self>, tab: &SharedString, cx: &App) -> Vec<MenuEntry> {
        let Some(this) = story.upgrade() else {
            return Vec::new();
        };
        let ids: Vec<SharedString> = this
            .read(cx)
            .tabs
            .read(cx)
            .tabs()
            .iter()
            .map(|tab| tab.id().clone())
            .collect();
        let at = ids.iter().position(|id| id == tab).unwrap_or(0);
        let item =
            |key: &'static str,
             label: &'static str,
             run: fn(&mut Self, SharedString, &mut Window, &mut Context<Self>)| {
                let story = story.clone();
                let tab = tab.clone();
                MenuItem::new(key, label).on_select(move |window, cx| {
                    let tab = tab.clone();
                    let _ = story.update(cx, |this, cx| run(this, tab, window, cx));
                })
            };
        vec![
            item("close-tab", "Close Tab", |this, tab, window, cx| {
                this.request_close(Closing::Tabs(vec![tab]), window, cx);
            })
            .into(),
            item(
                "close-other-tabs",
                "Close Other Tabs",
                |this, tab, window, cx| {
                    this.close_others(&tab, false, window, cx);
                },
            )
            .disabled(ids.len() < 2)
            .into(),
            item(
                "close-tabs-right",
                "Close Tabs to the Right",
                |this, tab, window, cx| {
                    this.close_others(&tab, true, window, cx);
                },
            )
            .disabled(at + 1 >= ids.len())
            .into(),
            MenuEntry::Separator,
            item("rename-tab", "Rename Tab...", |this, tab, window, cx| {
                this.tabs
                    .update(cx, |tabs, cx| tabs.start_rename(tab, window, cx));
            })
            .into(),
        ]
    }

    /// The rows of a pane's context menu. Copy and Paste go to the pane,
    /// the splits to the story, through the pane's focus.
    fn pane_entries(
        story: &WeakEntity<Self>,
        pane: PaneId,
        terminal: &Entity<TerminalState>,
        cx: &App,
    ) -> Vec<MenuEntry> {
        let has_selection = terminal.read(cx).has_selection();
        let (zoomed, split) = story
            .upgrade()
            .map(|story| {
                let story = story.read(cx);
                let split = story
                    .tab_of(pane)
                    .and_then(|tab| story.layouts.get(&tab))
                    .is_some_and(|layout| matches!(layout.root, Node::Split { .. }));
                (story.zoomed(pane), split)
            })
            .unwrap_or_default();
        vec![
            MenuItem::new("copy", "Copy")
                .action(actions::Copy)
                .disabled(!has_selection)
                .into(),
            MenuItem::new("paste", "Paste")
                .action(actions::Paste)
                .into(),
            MenuEntry::Separator,
            MenuItem::new("split-right", "Split Right")
                .action(SplitRight)
                .into(),
            MenuItem::new("split-left", "Split Left")
                .action(SplitLeft)
                .into(),
            MenuItem::new("split-down", "Split Down")
                .action(SplitDown)
                .into(),
            MenuItem::new("split-up", "Split Up").action(SplitUp).into(),
            MenuEntry::Separator,
            MenuItem::new(
                "zoom-pane",
                if zoomed { "Unzoom Pane" } else { "Zoom Pane" },
            )
            .action(TogglePaneZoom)
            .disabled(!split)
            .into(),
            MenuEntry::Separator,
            MenuItem::new("zoom-in", "Zoom In")
                .action(actions::IncreaseFontSize)
                .into(),
            MenuItem::new("zoom-out", "Zoom Out")
                .action(actions::DecreaseFontSize)
                .into(),
            MenuItem::new("reset-zoom", "Reset Zoom")
                .action(actions::ResetFontSize)
                .into(),
        ]
    }

    fn send_leader(&mut self, cx: &mut Context<Self>) {
        if let Some(terminal) = self.focused_terminal(cx).cloned() {
            terminal.update(cx, |terminal, cx| terminal.send_text("\u{1}", cx));
        }
    }

    fn render_node(&self, node: &Node, focused: PaneId, split: bool, cx: &App) -> gpui_kit::Div {
        let theme = self.tokens(cx);
        match node {
            // In a split, a hairline frames each pane and the focused one
            // takes the ring color.
            Node::Leaf(id) => div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .when(split, |this| {
                    this.border_1().border_color(if *id == focused {
                        theme.ring()
                    } else {
                        theme.border()
                    })
                })
                .children(self.panes.get(id).map(|terminal| {
                    let focus = terminal.read(cx).focus_handle().clone();
                    let entries = terminal.clone();
                    let story = self.this.clone();
                    let pane = *id;
                    // A right click focuses the pane first, so its menu's
                    // commands go to that pane. A program that reports the
                    // mouse gets the click instead, as in Ghostty.
                    ContextMenu::new(
                        ElementId::NamedChild(Arc::new(id.element_id()), "menu".into()),
                        &self.pane_menu,
                    )
                    .action_context(&focus)
                    .items(move |_, cx| Self::pane_entries(&story, pane, &entries, cx))
                    .size_full()
                    .child(Terminal::new(id.element_id(), terminal))
                })),
            Node::Split { axis, children } => div()
                .flex()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .map(|this| match axis {
                    Axis::Horizontal => this.flex_row(),
                    Axis::Vertical => this.flex_col(),
                })
                .children(
                    children
                        .iter()
                        .map(|child| self.render_node(child, focused, split, cx)),
                ),
        }
    }
}

/// The story's shortcuts by group, as an action and its keys, for this
/// platform's bindings.
fn shortcut_groups() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    let mac = cfg!(target_os = "macos");
    let pick = |mac_keys, other_keys| if mac { mac_keys } else { other_keys };
    vec![
        (
            "Tabs",
            vec![
                ("New tab", pick("Ctrl-A c, Cmd-T", "Ctrl-A c")),
                ("Next tab", pick("Ctrl-A n, Cmd-Shift-]", "Ctrl-A n")),
                ("Previous tab", pick("Ctrl-A p, Cmd-Shift-[", "Ctrl-A p")),
                ("Select a tab", pick("Ctrl-A 1-9, Cmd-1-9", "Ctrl-A 1-9")),
            ],
        ),
        (
            "Panes",
            vec![
                ("Split right", pick("Ctrl-A | or %, Cmd-D", "Ctrl-A | or %")),
                (
                    "Split down",
                    pick("Ctrl-A - or \", Cmd-Shift-D", "Ctrl-A - or \""),
                ),
                (
                    "Move between panes",
                    pick("Ctrl-A h j k l, Cmd-Alt-Arrows", "Ctrl-A h j k l"),
                ),
                ("Next pane", "Ctrl-A o"),
                ("Close pane", pick("Ctrl-A x, Cmd-W", "Ctrl-A x")),
                ("Send Ctrl-A", "Ctrl-A Ctrl-A"),
            ],
        ),
        (
            "Zoom",
            vec![
                (
                    "Zoom the pane",
                    pick("Ctrl-A z, Cmd-Shift-Enter", "Ctrl-A z"),
                ),
                ("Larger font", pick("Cmd-+", "Ctrl-+")),
                ("Smaller font", pick("Cmd--", "Ctrl--")),
                ("Reset font", pick("Cmd-0", "Ctrl-0")),
            ],
        ),
        (
            "Copy and paste",
            vec![
                ("Copy", pick("Cmd-C", "Ctrl-Shift-C")),
                ("Paste", pick("Cmd-V", "Ctrl-Shift-V")),
                ("Select all", pick("Cmd-A", "Ctrl-Shift-A")),
            ],
        ),
    ]
}

impl TerminalStory {
    /// The id of the button that opens the shortcuts dialog.
    pub const SHORTCUTS: &str = "terminal-shortcuts";

    /// Lists the story's shortcuts in a table, a group per heading.
    fn render_shortcuts_dialog(&self, cx: &mut Context<Self>) -> Dialog {
        let muted = cx.theme().muted_foreground();
        let groups = shortcut_groups().into_iter().map(|(group, rows)| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_xs().font_medium().text_color(muted).child(group))
                .children(rows.into_iter().map(|(action, keys)| {
                    div()
                        .flex()
                        .gap_4()
                        .child(div().w_40().flex_shrink_0().child(action))
                        .child(div().flex_1().text_color(muted).child(keys))
                }))
        });
        Dialog::new("terminal-shortcuts-dialog")
            .open(self.shortcuts_open)
            .title("Shortcuts")
            .description("Ctrl-A is the leader: press it, then the key.")
            .on_open_change({
                let this = self.this.clone();
                move |open, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.shortcuts_open = open;
                        cx.notify();
                    });
                }
            })
            .child(div().flex().flex_col().gap_4().children(groups))
    }

    /// Asks before a close that would end a running program, in Ghostty's
    /// words.
    fn render_close_dialog(&self, cx: &mut Context<Self>) -> Dialog {
        let (title, description) = match &self.pending_close {
            Some(pending) => {
                let title = match &pending.closing {
                    Closing::Pane(_) => "Close Terminal?",
                    Closing::Tabs(tabs) if tabs.len() > 1 => "Close Tabs?",
                    Closing::Tabs(_) => "Close Tab?",
                };
                let names = match pending.running.as_slice() {
                    [one] => one.clone(),
                    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
                    [] => String::new(),
                };
                let verb = if pending.running.len() == 1 {
                    "is"
                } else {
                    "are"
                };
                (
                    title,
                    format!("{names} {verb} still running. Closing the terminal ends it."),
                )
            }
            None => ("Close Terminal?", String::new()),
        };
        Dialog::new("terminal-close")
            .open(self.pending_close.is_some())
            .title(title)
            .description(description)
            .on_open_change({
                let this = self.this.clone();
                move |open, window, cx| {
                    if !open {
                        let _ = this.update(cx, |this, cx| this.resolve_close(false, window, cx));
                    }
                }
            })
            .footer(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("terminal-close-cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.resolve_close(false, window, cx);
                            })),
                    )
                    .child(
                        Button::new("terminal-close-confirm")
                            .destructive()
                            .label("Close")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.resolve_close(true, window, cx);
                            })),
                    ),
            )
    }
}

impl Render for TerminalStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bar = cx.theme().metrics.title_bar;
        let (background, border) = {
            let tokens = self.tokens(cx);
            (tokens.background(), tokens.border())
        };
        let body = self
            .selected_tab(cx)
            .and_then(|tab| self.layouts.get(&tab))
            .map(|layout| {
                if layout.zoomed {
                    self.render_node(&Node::Leaf(layout.focused), layout.focused, false, cx)
                } else {
                    let split = matches!(layout.root, Node::Split { .. });
                    self.render_node(&layout.root, layout.focused, split, cx)
                }
            });
        page([section(
            "Shell",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .child(note(
                    "Each pane runs your login shell in a pty, through Ghostty's terminal \
                     engine. Click a pane to type into it.",
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Select::new("terminal-theme", &self.themes)
                                .accessibility_label("Terminal theme")
                                .w(px(280.)),
                        )
                        .child(
                            Button::new(Self::SHORTCUTS)
                                .outline()
                                .label("Shortcuts")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.shortcuts_open = true;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    frame(px(480.), cx)
                        .key_context(CONTEXT)
                        .flex()
                        .flex_col()
                        .on_action(
                            cx.listener(|this, _: &NewTab, window, cx| this.new_tab(window, cx)),
                        )
                        .on_action(cx.listener(|this, _: &NextTab, _, cx| {
                            this.tabs.update(cx, |tabs, cx| tabs.select_neighbor(1, cx));
                        }))
                        .on_action(cx.listener(|this, _: &PreviousTab, _, cx| {
                            this.tabs
                                .update(cx, |tabs, cx| tabs.select_neighbor(-1, cx));
                        }))
                        .on_action(cx.listener(|this, action: &SelectTab, _, cx| {
                            this.select_tab(action.0.saturating_sub(1), cx);
                        }))
                        .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                            this.split(Axis::Horizontal, false, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                            this.split(Axis::Vertical, false, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &SplitLeft, window, cx| {
                            this.split(Axis::Horizontal, true, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &SplitUp, window, cx| {
                            this.split(Axis::Vertical, true, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &FocusLeft, window, cx| {
                            this.focus_direction(Axis::Horizontal, false, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &FocusRight, window, cx| {
                            this.focus_direction(Axis::Horizontal, true, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &FocusUp, window, cx| {
                            this.focus_direction(Axis::Vertical, false, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &FocusDown, window, cx| {
                            this.focus_direction(Axis::Vertical, true, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &FocusNext, window, cx| {
                            this.focus_next(window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &ClosePane, window, cx| {
                            this.close_focused(window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &SendLeader, _, cx| {
                            this.send_leader(cx);
                        }))
                        .on_action(cx.listener(|this, _: &TogglePaneZoom, window, cx| {
                            this.toggle_zoom(window, cx);
                        }))
                        .bg(background)
                        .border_color(border)
                        // The strip and its controls draw in the
                        // terminal's colors, so a theme change reaches
                        // both.
                        .child(ThemeScope::new(
                            Self::SCOPE,
                            div().h(bar).w_full().flex_shrink_0().child({
                                let story = cx.entity().downgrade();
                                Tabs::new("terminal-tabs", &self.tabs)
                                    .renamable(true)
                                    .confirm_close(true)
                                    .context_menu(&self.tab_menu, move |tab, _, cx| {
                                        Self::tab_entries(&story, tab, cx)
                                    })
                            }),
                        ))
                        .child(div().flex().flex_1().min_h_0().children(body)),
                )
                .child(self.render_close_dialog(cx))
                .child(self.render_shortcuts_dialog(cx)),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u64) -> PaneId {
        PaneId(id)
    }

    #[test]
    fn splits_nest_across_axes_and_flatten_along_one() {
        let mut root = Node::Leaf(pane(0));
        assert!(root.split(pane(0), Axis::Horizontal, pane(1), false));
        assert!(root.split(pane(1), Axis::Horizontal, pane(2), false));
        assert!(root.split(pane(2), Axis::Vertical, pane(3), false));
        assert_eq!(root.leaves(), [pane(0), pane(1), pane(2), pane(3)]);
        let Node::Split { axis, children } = &root else {
            panic!("a split");
        };
        assert_eq!(*axis, Axis::Horizontal);
        assert_eq!(
            children.len(),
            3,
            "a split along the same axis adds a sibling"
        );
    }

    #[test]
    fn a_split_before_puts_the_new_pane_first() {
        let mut root = Node::Leaf(pane(0));
        assert!(root.split(pane(0), Axis::Horizontal, pane(1), true));
        assert!(root.split(pane(0), Axis::Horizontal, pane(2), true));
        assert_eq!(root.leaves(), [pane(1), pane(2), pane(0)]);
        assert!(root.split(pane(0), Axis::Vertical, pane(3), true));
        assert_eq!(root.leaves(), [pane(1), pane(2), pane(3), pane(0)]);
    }

    #[test]
    fn neighbors_follow_the_split_axes() {
        let mut root = Node::Leaf(pane(0));
        root.split(pane(0), Axis::Horizontal, pane(1), false);
        root.split(pane(1), Axis::Vertical, pane(2), false);
        assert_eq!(
            root.neighbor(pane(0), Axis::Horizontal, true),
            Some(pane(1))
        );
        assert_eq!(
            root.neighbor(pane(2), Axis::Horizontal, false),
            Some(pane(0))
        );
        assert_eq!(root.neighbor(pane(1), Axis::Vertical, true), Some(pane(2)));
        assert_eq!(root.neighbor(pane(0), Axis::Vertical, true), None);
        assert_eq!(root.neighbor(pane(0), Axis::Horizontal, false), None);
    }

    #[test]
    fn removing_a_pane_collapses_a_split_of_one() {
        let mut root = Node::Leaf(pane(0));
        root.split(pane(0), Axis::Horizontal, pane(1), false);
        root.split(pane(1), Axis::Vertical, pane(2), false);
        assert!(root.remove(pane(2)));
        assert_eq!(
            root,
            Node::Split {
                axis: Axis::Horizontal,
                children: vec![Node::Leaf(pane(0)), Node::Leaf(pane(1))],
            }
        );
        assert!(root.remove(pane(0)));
        assert_eq!(root, Node::Leaf(pane(1)));
        assert!(!root.remove(pane(1)), "the last pane is the tab");
    }
}
