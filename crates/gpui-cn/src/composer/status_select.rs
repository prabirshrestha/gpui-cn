use gpui_kit::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StyleRefinement, Styled, Subscription, Window,
    assets::IconName,
    base::{Align, StyledExt as _},
    div,
    prelude::FluentBuilder as _,
};

use crate::{
    ActiveTheme as _, Button, ButtonSize, DropdownMenu, Icon, MenuEntry, MenuEvent, MenuItem,
    MenuState,
};

/// One choice of a [`StatusSelect`]: an id from the domain, a label, and an
/// optional line of description in the menu.
///
/// ```
/// use gpui_cn::StatusOption;
///
/// let option = StatusOption::new("main", "main").description("The default branch");
/// assert_eq!(option.label().as_ref(), "main");
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct StatusOption {
    id: SharedString,
    label: SharedString,
    description: Option<SharedString>,
}

impl StatusOption {
    /// An option with a stable id and the label it shows.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
        }
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
    style: StyleRefinement,
}

impl StatusSelect {
    /// An item on `state`, with a stable id.
    pub fn new(id: impl Into<ElementId>, state: &Entity<StatusSelectState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            icon: None,
            style: StyleRefinement::default(),
        }
    }

    /// The icon before the label.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

impl Styled for StatusSelect {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StatusSelect {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (ink, muted, status_icon) = {
            let theme = cx.theme();
            (
                theme.foreground(),
                theme.composer_muted,
                theme.metrics.status_icon,
            )
        };
        let read = self.state.read(cx);
        let menu = read.menu.clone();
        let label = read.selected_label().cloned().unwrap_or_default();
        let state = self.state.clone();
        let trigger = Button::new(ElementId::NamedChild(
            self.id.clone().into(),
            "trigger".into(),
        ))
        .ghost()
        .size(ButtonSize::Sm)
        .accessibility_label(label.clone())
        .text_color(ink)
        .when_some(self.icon, |button, icon| {
            button.icon(icon.size(status_icon))
        })
        .child(div().child(label))
        .trailing_icon(Icon::from(IconName::ChevronDown).size_3().text_color(muted));
        DropdownMenu::new(self.id, &menu)
            .align(Align::Start)
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
    }
}
