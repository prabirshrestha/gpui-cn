use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, EventEmitter, Focusable as _,
    IntoElement, ParentElement as _, RenderOnce, SharedString, StyleRefinement, Styled,
    Subscription, Window,
    assets::IconName,
    base::{Align, Selectable, StyledExt as _, h_flex},
    div,
    prelude::FluentBuilder as _,
};

use crate::{
    ActiveTheme as _, Button, ButtonSize, Command, CommandEntry, CommandEvent, CommandItem,
    CommandState, DropdownMenu, Icon, MenuEntry, MenuEvent, MenuItem, MenuState, Popover, fuzzy,
    menu::MenuLook, middle_text::MiddleText,
};

/// How many options make a [`StatusSelect`] searchable when it is not told
/// either way: three. A list of two, such as a device, is a plain menu; a
/// list of three or more, such as projects and branches, has a search
/// field and ranks what is typed.
pub const SEARCH_MIN_OPTIONS: usize = 3;

/// One choice of a [`StatusSelect`]: an id from the domain, a label, and
/// optionally a leading icon, a line of description in a menu, and a short
/// muted word at the right of a search list row, such as "current" or
/// "remote".
///
/// ```
/// use gpui_cn::StatusOption;
///
/// let option = StatusOption::new("main", "main").description("The default branch");
/// assert_eq!(option.label().as_ref(), "main");
/// ```
#[derive(Clone)]
#[non_exhaustive]
pub struct StatusOption {
    id: SharedString,
    label: SharedString,
    description: Option<SharedString>,
    trailing: Option<SharedString>,
    icon: Option<Icon>,
}

impl StatusOption {
    /// An option with a stable id and the label it shows.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            trailing: None,
            icon: None,
        }
    }

    /// A short muted word at the right of the row in a search list, such
    /// as "current".
    pub fn trailing(mut self, trailing: impl Into<SharedString>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    /// An icon before the label in the list.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// The word at the right of the row, when there is one.
    pub fn trailing_label(&self) -> Option<&SharedString> {
        self.trailing.as_ref()
    }

    /// A line under the label in the menu.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The label.
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// What a [`StatusSelectState`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusSelectEvent {
    /// Another option was chosen. The payload is its id.
    Changed(SharedString),
}

/// The options of a status item, the chosen one, and its open menu.
///
/// Owned by the view that shows the item, which observes it so a change
/// renders. The choice is kept by option id.
pub struct StatusSelectState {
    options: Vec<StatusOption>,
    selected: SharedString,
    menu: Entity<MenuState>,
    open: bool,
    _menu_events: Subscription,
}

impl EventEmitter<StatusSelectEvent> for StatusSelectState {}

impl StatusSelectState {
    /// A state over `options` with the first chosen.
    pub fn new(options: impl IntoIterator<Item = StatusOption>, cx: &mut Context<Self>) -> Self {
        let options: Vec<_> = options.into_iter().collect();
        let menu = cx.new(MenuState::new);
        let subscription = cx.subscribe(&menu, |this, _, event: &MenuEvent, cx| {
            if let MenuEvent::Activated(key) = event {
                this.select(key.clone(), cx);
            }
        });
        Self {
            selected: options
                .first()
                .map(|option| option.id.clone())
                .unwrap_or_default(),
            options,
            menu,
            open: false,
            _menu_events: subscription,
        }
    }

    /// Chooses the option with `id`, if there is one.
    pub fn with_selected(mut self, id: impl Into<SharedString>) -> Self {
        let id = id.into();
        if self.options.iter().any(|option| option.id == id) {
            self.selected = id;
        }
        self
    }

    /// The options, in order.
    pub fn options(&self) -> &[StatusOption] {
        &self.options
    }

    /// The id of the chosen option.
    pub fn selected(&self) -> &SharedString {
        &self.selected
    }

    /// The label of the chosen option.
    pub fn selected_label(&self) -> Option<&SharedString> {
        self.options
            .iter()
            .find(|option| option.id == self.selected)
            .map(|option| &option.label)
    }

    /// Whether the search list is open. A plain menu keeps its own state.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the search list.
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.open != open {
            self.open = open;
            cx.notify();
        }
    }

    /// Chooses the option with `id` and emits `Changed`. An unknown id, or
    /// the one already chosen, changes nothing.
    pub fn select(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        if id == self.selected || !self.options.iter().any(|option| option.id == id) {
            return;
        }
        self.selected = id.clone();
        cx.emit(StatusSelectEvent::Changed(id));
        cx.notify();
    }
}

/// An item of a [`ComposerStatusTab`](crate::ComposerStatusTab) that opens
/// a menu: an icon, the chosen option's label in the text color, and a
/// muted chevron. The surface shows only for the pointer, a press, or the
/// open menu, as the permission trigger's does. The menu is a
/// [`DropdownMenu`] with a check on the chosen option.
///
/// ```no_run
/// use gpui_cn::{StatusOption, StatusSelect, StatusSelectState};
/// use gpui_kit::{AppContext as _, Context};
/// use gpui_kit::assets::IconName;
///
/// fn branch(cx: &mut Context<()>) -> StatusSelect {
///     let state = cx.new(|cx| {
///         StatusSelectState::new(
///             [StatusOption::new("main", "main"), StatusOption::new("dev", "dev")],
///             cx,
///         )
///     });
///     StatusSelect::new("branch", &state).icon(IconName::GitBranch)
/// }
/// ```
#[derive(IntoElement)]
#[non_exhaustive]
pub struct StatusSelect {
    id: ElementId,
    state: Entity<StatusSelectState>,
    icon: Option<Icon>,
    searchable: Option<bool>,
    placeholder: SharedString,
    empty: SharedString,
    style: StyleRefinement,
}

impl StatusSelect {
    /// An item on `state`, with a stable id.
    pub fn new(id: impl Into<ElementId>, state: &Entity<StatusSelectState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            icon: None,
            searchable: None,
            placeholder: "Search...".into(),
            empty: "No results.".into(),
            style: StyleRefinement::default(),
        }
    }

    /// The icon before the label.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Forces the presentation: `true` opens a list with a search field,
    /// `false` a plain menu. Left alone, a select with
    /// [`SEARCH_MIN_OPTIONS`] options or more is searchable.
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = Some(searchable);
        self
    }

    /// The hint in the search field of the list. The default is
    /// "Search...". It is read when the list first opens.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The text shown while a search matches nothing. The default is
    /// "No results."
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = text.into();
        self
    }

    /// Whether the select opens a search list for `options` options.
    pub fn is_searchable(&self, options: usize) -> bool {
        self.searchable.unwrap_or(options >= SEARCH_MIN_OPTIONS)
    }
}

impl Styled for StatusSelect {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Chooses an option when the search list's command is chosen. It lives in
/// the window's keyed state, so the subscription is made once.
struct Chooser {
    _subscription: Subscription,
}

/// A built trigger, which the popover marks open through `Selectable`.
#[derive(IntoElement)]
struct Slot {
    trigger: AnyElement,
    open: bool,
}

impl Selectable for Slot {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }

    fn open(self, _: bool) -> Self {
        self
    }

    fn is_open(&self) -> bool {
        self.open
    }
}

impl RenderOnce for Slot {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.trigger
    }
}

/// The rows of the search list for the options at `order`, in that order.
fn search_entries(options: &[StatusOption], order: &[usize]) -> Vec<CommandEntry> {
    order
        .iter()
        .map(|&index| {
            let option = options[index].clone();
            let (label, trailing, icon) = (
                option.label.clone(),
                option.trailing.clone(),
                option.icon.clone(),
            );
            CommandEntry::from(
                CommandItem::new(option.id.clone(), option.label.clone()).render(
                    move |_, _, cx| {
                        let theme = cx.theme();
                        let caption_text = theme.base.typography.xs;
                        let muted = theme.muted_foreground();
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .when_some(icon.clone(), |row, icon| {
                                row.child(icon.size_4().flex_shrink_0().text_color(muted))
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(MiddleText::new(label.clone())),
                            )
                            .when_some(trailing.clone(), |row, word| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(caption_text.size)
                                        .line_height(caption_text.line_height)
                                        .text_color(muted)
                                        .child(word),
                                )
                            })
                    },
                ),
            )
        })
        .collect()
}

impl StatusSelect {
    fn trigger(&self, label: SharedString, cx: &App) -> Button {
        let (ink, muted, status_icon) = {
            let theme = cx.theme();
            (
                theme.foreground(),
                theme.composer_muted,
                theme.metrics.status_icon,
            )
        };
        Button::new(ElementId::NamedChild(
            self.id.clone().into(),
            "trigger".into(),
        ))
        .ghost()
        .size(ButtonSize::Sm)
        .accessibility_label(label.clone())
        .text_color(ink)
        .when_some(self.icon.clone(), |button, icon| {
            button.icon(icon.size(status_icon))
        })
        .label(label)
        .flex_shrink(1.)
        .min_w_0()
        .trailing_icon(Icon::from(IconName::ChevronDown).size_3().text_color(muted))
    }

    /// The search list: a popover under the trigger holding a palette over
    /// the options, ranked by what is typed.
    fn render_search(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let (options, open, label) = {
            let read = self.state.read(cx);
            (
                read.options.clone(),
                read.open,
                read.selected_label().cloned().unwrap_or_default(),
            )
        };
        let child = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let command = window.use_keyed_state(child("list"), cx, {
            let (options, placeholder) = (options.clone(), self.placeholder.clone());
            move |window, cx| {
                CommandState::new(placeholder, window, cx).with_search_handler(
                    move |query, cx| {
                        let options = options.clone();
                        let labels: Vec<String> = options
                            .iter()
                            .map(|option| option.label.to_string())
                            .collect();
                        let ranked = cx.background_spawn(async move {
                            fuzzy::rank(&query, &labels, |label| label.as_str())
                                .into_iter()
                                .map(|found| found.index)
                                .collect::<Vec<_>>()
                        });
                        cx.spawn(async move |_, _| search_entries(&options, &ranked.await))
                    },
                    cx,
                )
            }
        });
        let state = self.state.clone();
        let _chooser = window.use_keyed_state(child("chooser"), cx, {
            let (state, command) = (state.clone(), command.clone());
            move |window, cx| Chooser {
                _subscription: cx.subscribe_in(
                    &command,
                    window,
                    move |_, _, event: &CommandEvent, _, cx| {
                        if let CommandEvent::Confirmed(key) = event {
                            state.update(cx, |state, cx| {
                                state.select(key.clone(), cx);
                                state.set_open(false, cx);
                            });
                        }
                    },
                ),
            }
        });
        let look = MenuLook::of(cx.theme(), window.rem_size());
        let width = look.max_width;
        let height = look.palette_height();
        let focus = command.focus_handle(cx);
        let trigger = self.trigger(label, cx).selected(open).into_any_element();
        let toggle = state.clone();
        let empty = self.empty.clone();
        let palette_id = child("palette");
        Popover::new(child("popover"))
            .open(open)
            .on_open_change(move |open, _, cx| {
                toggle.update(cx, |state, cx| state.set_open(open, cx));
            })
            .align(Align::Start)
            .track_focus(&focus)
            .trigger(Slot { trigger, open })
            .content(move |_, _| {
                Command::new(palette_id.clone(), &command)
                    .bordered(false)
                    .empty(empty.clone())
                    .w_full()
                    .max_w_full()
                    .h(height)
            })
            .w(width)
            .p_0()
            .flex_shrink(1.)
            .min_w_0()
            .refine_style(&self.style)
            .into_any_element()
    }
}

impl RenderOnce for StatusSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let options = self.state.read(cx).options.len();
        if self.is_searchable(options) {
            return self.render_search(window, cx);
        }
        let menu = self.state.read(cx).menu.clone();
        let label = self
            .state
            .read(cx)
            .selected_label()
            .cloned()
            .unwrap_or_default();
        let state = self.state.clone();
        let trigger = self.trigger(label, cx);
        DropdownMenu::new(self.id, &menu)
            .align(Align::Start)
            .flex_shrink(1.)
            .min_w_0()
            .refine_style(&self.style)
            .trigger(trigger)
            .items(move |_, cx| {
                let state = state.read(cx);
                state
                    .options
                    .iter()
                    .map(|option| {
                        let item = MenuItem::new(option.id.clone(), option.label.clone())
                            .checked(option.id == state.selected);
                        MenuEntry::from(match &option.description {
                            Some(text) => item.description(text.clone()),
                            None => item,
                        })
                    })
                    .collect()
            })
            .into_any_element()
    }
}
