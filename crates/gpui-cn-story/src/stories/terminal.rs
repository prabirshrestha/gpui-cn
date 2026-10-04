use std::collections::HashMap;

use gpui_cn::terminal::{
    FixtureSource, LocalTerminalOptions, Terminal, TerminalConfig, TerminalEvent, TerminalState,
    WorkingDirectory,
};
use gpui_cn::{ActiveTheme as _, Tab, Tabs, TabsEvent, TabsState, gpui_kit::assets::IconName};
use gpui_kit::{
    Action, AnyView, App, AppContext as _, Axis, Context, ElementId, Entity, Global,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    Styled as _, Window, actions, div, prelude::FluentBuilder as _, px,
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

/// Binds the leader keys and, on macOS, the Command shortcuts, once.
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

    /// Puts `new` after `target` along `axis`.
    fn split(&mut self, target: PaneId, axis: Axis, new: PaneId) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                *self = Node::Split {
                    axis,
                    children: vec![Node::Leaf(target), Node::Leaf(new)],
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
                    children.insert(index + 1, Node::Leaf(new));
                    return true;
                }
                children
                    .iter_mut()
                    .any(|child| child.split(target, axis, new))
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
}

/// Terminal tabs with splits and tmux-style leader keys, each pane a shell.
pub struct TerminalStory {
    tabs: Entity<TabsState>,
    layouts: HashMap<SharedString, TabPanes>,
    panes: HashMap<PaneId, Entity<TerminalState>>,
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
            let tabs = cx.new(|_| TabsState::new([]));
            cx.observe(&tabs, |_, _, cx| cx.notify()).detach();
            cx.subscribe_in(
                &tabs,
                window,
                |this: &mut Self, _, event, window, cx| match event {
                    TabsEvent::Selected(_) => this.show_selected(window, cx),
                    TabsEvent::Closed(id) => this.tab_closed(id, window, cx),
                    TabsEvent::AddRequested => this.new_tab(window, cx),
                    _ => {}
                },
            )
            .detach();
            let mut story = Self {
                tabs,
                layouts: HashMap::new(),
                panes: HashMap::new(),
                next_pane: 0,
                next_tab: 0,
            };
            story.new_tab(window, cx);
            story
        })
        .into()
    }
}

impl TerminalStory {
    /// Makes every pane opened from now on render canned output instead of
    /// running a shell. The snapshot and benchmark binaries call it, so
    /// their pictures and numbers do not depend on the user's shell.
    pub fn use_fixtures(cx: &mut App) {
        cx.set_global(Fixtures);
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
        let terminal = cx.new(|cx| {
            if fixtures {
                TerminalState::new(
                    FixtureSource::new(FIXTURE),
                    TerminalConfig::default(),
                    window,
                    cx,
                )
            } else {
                TerminalState::local(
                    LocalTerminalOptions::default().with_cwd(cwd),
                    TerminalConfig::default(),
                    window,
                    cx,
                )
            }
        });
        cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&terminal, window, move |this, _, event, window, cx| {
            // A shell that ended normally takes its pane with it. One that
            // failed or ended at once stays, so its status shows.
            if let TerminalEvent::Exited(status) = event
                && !status.is_abnormal()
            {
                this.close_pane(id, window, cx);
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
        for (tab, layout) in &self.layouts {
            let visible = *tab == selected;
            for pane in layout.root.leaves() {
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

    fn split(&mut self, axis: Axis, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.selected_tab(cx) else {
            return;
        };
        let Some(target) = self.layouts.get(&tab).map(|layout| layout.focused) else {
            return;
        };
        let pane = self.spawn_pane(window, cx);
        if let Some(layout) = self.layouts.get_mut(&tab)
            && layout.root.split(target, axis, pane)
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
        let next = self
            .selected_tab(cx)
            .and_then(|tab| self.layouts.get(&tab))
            .and_then(|layout| layout.root.neighbor(layout.focused, axis, forward));
        if let Some(next) = next {
            self.focus_pane(next, window, cx);
        }
    }

    fn focus_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            self.close_pane(focused, window, cx);
        }
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

    fn send_leader(&mut self, cx: &mut Context<Self>) {
        if let Some(terminal) = self.focused_terminal(cx).cloned() {
            terminal.update(cx, |terminal, cx| terminal.send_text("\u{1}", cx));
        }
    }

    fn render_node(&self, node: &Node, focused: PaneId, split: bool, cx: &App) -> gpui_kit::Div {
        let theme = cx.theme();
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
                .children(
                    self.panes
                        .get(id)
                        .map(|terminal| Terminal::new(id.element_id(), terminal)),
                ),
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

impl Render for TerminalStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let bar = theme.metrics.title_bar;
        let body = self
            .selected_tab(cx)
            .and_then(|tab| self.layouts.get(&tab))
            .map(|layout| {
                let split = matches!(layout.root, Node::Split { .. });
                self.render_node(&layout.root, layout.focused, split, cx)
            });
        let leader = if cfg!(target_os = "macos") {
            "Ctrl-A, then: c new tab; n, p, 1-9 select a tab; | or % split right; - or \" split \
             down; h, j, k, l, o move between panes; x close the pane; Ctrl-A sends Ctrl-A. \
             Cmd-T, Cmd-W, Cmd-D, Cmd-Shift-D, Cmd-1 to Cmd-9, and Cmd-Shift-[ and ] do the same."
        } else {
            "Ctrl-A, then: c new tab; n, p, 1-9 select a tab; | or % split right; - or \" split \
             down; h, j, k, l, o move between panes; x close the pane; Ctrl-A sends Ctrl-A."
        };
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
                .child(note(leader, cx))
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
                            this.split(Axis::Horizontal, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                            this.split(Axis::Vertical, window, cx);
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
                        .child(
                            div()
                                .h(bar)
                                .w_full()
                                .flex_shrink_0()
                                .child(Tabs::new("terminal-tabs", &self.tabs)),
                        )
                        .child(div().flex().flex_1().min_h_0().children(body)),
                ),
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
        assert!(root.split(pane(0), Axis::Horizontal, pane(1)));
        assert!(root.split(pane(1), Axis::Horizontal, pane(2)));
        assert!(root.split(pane(2), Axis::Vertical, pane(3)));
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
    fn neighbors_follow_the_split_axes() {
        let mut root = Node::Leaf(pane(0));
        root.split(pane(0), Axis::Horizontal, pane(1));
        root.split(pane(1), Axis::Vertical, pane(2));
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
        root.split(pane(0), Axis::Horizontal, pane(1));
        root.split(pane(1), Axis::Vertical, pane(2));
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
