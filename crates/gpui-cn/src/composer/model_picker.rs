use std::f32::consts::PI;

use gpui_kit::{
    App, AppContext as _, ClickEvent, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable as _, InteractiveElement as _, IntoElement, KeyBinding, KeyDownEvent, ListAlignment,
    ListState, ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Subscription, Window, actions,
    assets::IconName,
    base::{
        Align, Selectable, StyledExt as _, TestSupportExt as _,
        actions::{Confirm, SelectDown, SelectUp},
        h_flex,
        input::{InputEvent, InputState},
        transition, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    radians, rems,
};

use crate::{
    ActiveTheme as _, Button, ButtonSize, Icon, MenuEvent, MenuState, Popover, ScrollArea, Theme,
    collapse::{self, Need},
    menu::{MenuLook, row_frame, search_row, search_style, separator},
};

/// The names of the six effort levels, from the lowest to the highest.
///
/// A model that supports effort runs at one of them. The composer's effort
/// menu lists them in this order.
pub const EFFORT_LABELS: [&str; 6] = ["Low", "Medium", "High", "Extra high", "Ultra", "Max"];

/// The effort a model starts at when it is first chosen: `Medium`.
const DEFAULT_EFFORT: u8 = 1;

/// How many rows get a number shortcut: Cmd+1 to Cmd+9.
const SHORTCUT_ROWS: usize = 9;

/// The key context of the panel, which takes Up, Down, Enter, Home, and End
/// the search field passes on.
const CONTEXT: &str = "GpuiCnModelPicker";

actions!(
    gpui_cn_model_picker,
    [
        /// Moves the highlight to the first row.
        FirstRow,
        /// Moves the highlight to the last row.
        LastRow
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
        // The search field binds Home and End to move its caret. A binding
        // for the field inside the panel is as deep and comes later, so it
        // runs first, and it hands the key on while a query is typed.
        KeyBinding::new("home", FirstRow, Some("GpuiCnModelPicker > Input")),
        KeyBinding::new("end", LastRow, Some("GpuiCnModelPicker > Input")),
    ]);
}

/// A model a [`ModelPicker`] offers.
///
/// The id comes from the domain and is unique across the whole catalog,
/// every provider included.
///
/// ```
/// use gpui_cn::ModelEntry;
///
/// let model = ModelEntry::new("gpt-5.6-mini", "GPT-5.6 Mini").effort(true);
/// assert!(model.supports_effort());
/// assert!(!model.is_legacy());
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ModelEntry {
    id: SharedString,
    name: SharedString,
    effort: bool,
    legacy: bool,
}

impl ModelEntry {
    /// A model with a stable id and the name it shows. It has no effort
    /// setting and is not legacy.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            effort: false,
            legacy: false,
        }
    }

    /// Whether the model has an effort setting. While it is the chosen
    /// model, the composer shows its effort menu.
    pub fn effort(mut self, supported: bool) -> Self {
        self.effort = supported;
        self
    }

    /// Whether the model is legacy. A provider's legacy models fold into a
    /// collapsed "Legacy models" group at the top of its list.
    pub fn legacy(mut self, legacy: bool) -> Self {
        self.legacy = legacy;
        self
    }

    /// The id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The name.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// Whether the model has an effort setting.
    pub fn supports_effort(&self) -> bool {
        self.effort
    }

    /// Whether the model is legacy.
    pub fn is_legacy(&self) -> bool {
        self.legacy
    }
}

/// A maker of models, one mark in the picker's rail and the models it owns.
///
/// The icon is any [`Icon`]: the picker ships no vendor logos, so an
/// application brings the marks it has the right to show.
///
/// ```
/// use gpui_cn::{ModelEntry, ModelProvider};
/// use gpui_kit::assets::IconName;
///
/// let provider = ModelProvider::new("claude", "Claude")
///     .icon(IconName::Star)
///     .models([ModelEntry::new("sonnet-5.5", "Sonnet 5.5")]);
/// assert_eq!(provider.models_of().len(), 1);
/// ```
#[derive(Clone)]
#[non_exhaustive]
pub struct ModelProvider {
    id: SharedString,
    name: SharedString,
    icon: Icon,
    models: Vec<ModelEntry>,
}

impl ModelProvider {
    /// A provider with a stable id and the name it shows, with a generic
    /// icon and no models.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: IconName::Bot.into(),
            models: Vec::new(),
        }
    }

    /// The provider's mark.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = icon.into();
        self
    }

    /// The provider's models, in the order shown.
    pub fn models(mut self, models: impl IntoIterator<Item = ModelEntry>) -> Self {
        self.models = models.into_iter().collect();
        self
    }

    /// The id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The name.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// The models.
    pub fn models_of(&self) -> &[ModelEntry] {
        &self.models
    }
}

/// What the list of a [`ModelPicker`] shows: the favorites, or one
/// provider's models. A query overrides both and searches every provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelView {
    /// The models the user starred, across providers.
    Favorites,
    /// The models of the provider with this id.
    Provider(SharedString),
}

/// What a [`ModelPickerState`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelPickerEvent {
    /// A model was chosen. The payload is its id.
    Selected(SharedString),
    /// The effort level changed, from `0` to `5`.
    EffortChanged(u8),
    /// The rail's entry changed. The payload is the new view.
    ViewChanged(ModelView),
    /// The panel opened or closed.
    OpenChanged(bool),
    /// A model was starred or unstarred. The application keeps the
    /// favorites, so it saves them on this event.
    FavoriteChanged {
        /// The id of the model.
        model: SharedString,
        /// Whether it is a favorite now.
        favorite: bool,
    },
}

/// One row of the list: the fold of legacy models, or a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    /// The "Legacy models" header, with how many models it folds.
    Legacy(usize),
    /// A model by provider and model index, and its shortcut number
    /// (from `1`) when it is among the first nine models.
    Model {
        provider: usize,
        model: usize,
        number: Option<usize>,
    },
}

/// The rows the list shows. A query shows every model of every provider
/// whose name or whose provider's name contains it, ignoring case. With no
/// query the favorites view shows the starred models in the order they
/// were starred, and a provider's view shows its models, its legacy ones
/// folded under a header at the top and listed only while the fold is
/// open.
fn build_rows(
    providers: &[ModelProvider],
    view: &ModelView,
    query: &str,
    favorites: &[SharedString],
    legacy_open: bool,
) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    let model = |provider, model| Row::Model {
        provider,
        model,
        number: None,
    };
    let mut rows = Vec::new();
    if !query.is_empty() {
        for (p, provider) in providers.iter().enumerate() {
            let provider_matches = provider.name.to_lowercase().contains(&query);
            for (m, entry) in provider.models.iter().enumerate() {
                if provider_matches || entry.name.to_lowercase().contains(&query) {
                    rows.push(model(p, m));
                }
            }
        }
    } else {
        match view {
            ModelView::Favorites => {
                for id in favorites {
                    if let Some((p, m)) = find_model(providers, id) {
                        rows.push(model(p, m));
                    }
                }
            }
            ModelView::Provider(id) => {
                if let Some((p, provider)) = providers
                    .iter()
                    .enumerate()
                    .find(|(_, provider)| &provider.id == id)
                {
                    let legacy = provider.models.iter().filter(|m| m.legacy).count();
                    if legacy > 0 {
                        rows.push(Row::Legacy(legacy));
                        if legacy_open {
                            rows.extend(
                                provider
                                    .models
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, m)| m.legacy)
                                    .map(|(m, _)| model(p, m)),
                            );
                        }
                    }
                    rows.extend(
                        provider
                            .models
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| !m.legacy)
                            .map(|(m, _)| model(p, m)),
                    );
                }
            }
        }
    }
    let mut number = 0;
    for row in &mut rows {
        if let Row::Model { number: slot, .. } = row {
            number += 1;
            *slot = (number <= SHORTCUT_ROWS).then_some(number);
        }
    }
    rows
}

fn find_model(providers: &[ModelProvider], id: &str) -> Option<(usize, usize)> {
    providers.iter().enumerate().find_map(|(p, provider)| {
        provider
            .models
            .iter()
            .position(|model| model.id == id)
            .map(|m| (p, m))
    })
}

/// The catalog, the chosen model and its effort, the favorites, the view
/// the rail shows, the search, and the open panel of a [`ModelPicker`].
///
/// Owned by the view that shows the picker, which observes it so a change
/// renders. The choice and the favorites are kept by model id, so a catalog
/// that changes under them keeps the ones whose model is still there. The
/// state does not save anything: the application listens for
/// [`ModelPickerEvent::FavoriteChanged`] and seeds the favorites again with
/// [`with_favorites`](Self::with_favorites).
pub struct ModelPickerState {
    providers: Vec<ModelProvider>,
    selected: Option<SharedString>,
    effort: u8,
    favorites: Vec<SharedString>,
    view: ModelView,
    search: Entity<InputState>,
    focus: FocusHandle,
    effort_menu: Entity<MenuState>,
    open: bool,
    legacy_open: bool,
    rows: Vec<Row>,
    highlighted: Option<usize>,
    list: ListState,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ModelPickerEvent> for ModelPickerState {}

/// The search field's focus, which holds the keyboard while the panel is
/// open.
impl gpui_kit::Focusable for ModelPickerState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }
}

impl ModelPickerState {
    /// A picker over `providers` with nothing chosen and no favorites. The
    /// rail starts on the first provider.
    pub fn new(
        providers: impl IntoIterator<Item = ModelProvider>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let providers: Vec<_> = providers.into_iter().collect();
        let style = search_style(cx.theme());
        let search = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Search models...");
            input.set_editor_style(style);
            input
        });
        let effort_menu = cx.new(MenuState::new);
        let overdraw = cx.theme().metrics.model_picker_height;
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.rebuild(cx);
                }
            }),
            cx.subscribe(&effort_menu, |this, _, event: &MenuEvent, cx| {
                if let MenuEvent::Activated(key) = event
                    && let Ok(level) = key.parse::<u8>()
                {
                    this.set_effort(level, cx);
                }
            }),
        ];
        let mut state = Self {
            view: providers
                .first()
                .map(|provider| ModelView::Provider(provider.id.clone()))
                .unwrap_or(ModelView::Favorites),
            providers,
            selected: None,
            effort: DEFAULT_EFFORT,
            favorites: Vec::new(),
            search,
            focus: cx.focus_handle(),
            effort_menu,
            open: false,
            legacy_open: false,
            rows: Vec::new(),
            highlighted: None,
            list: ListState::new(0, ListAlignment::Top, overdraw),
            _subscriptions: subscriptions,
        };
        state.refresh_rows("");
        state
    }

    /// Chooses the model with `id`, if the catalog has it. The rail moves
    /// to its provider.
    pub fn with_selected(mut self, id: impl Into<SharedString>) -> Self {
        let id = id.into();
        if let Some(provider) = self.provider_of(&id) {
            self.view = ModelView::Provider(provider.id.clone());
            self.selected = Some(id);
            self.refresh_rows("");
        }
        self
    }

    /// Starts on `view`: the favorites, or a provider the catalog holds.
    /// A provider it does not hold changes nothing.
    pub fn with_view(mut self, view: ModelView) -> Self {
        let known = match &view {
            ModelView::Favorites => true,
            ModelView::Provider(id) => self.providers.iter().any(|provider| &provider.id == id),
        };
        if known {
            self.view = view;
            self.refresh_rows("");
        }
        self
    }

    /// Starts the effort at `level`, clamped to the last level.
    pub fn with_effort(mut self, level: u8) -> Self {
        self.effort = level.min((EFFORT_LABELS.len() - 1) as u8);
        self
    }

    /// Starts with these favorites, in this order. An id the catalog does
    /// not hold is ignored, and so is a repeat.
    pub fn with_favorites(
        mut self,
        ids: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        for id in ids {
            let id = id.into();
            if find_model(&self.providers, &id).is_some() && !self.favorites.contains(&id) {
                self.favorites.push(id);
            }
        }
        self.refresh_rows("");
        self
    }

    fn provider_of(&self, model_id: &str) -> Option<&ModelProvider> {
        self.providers
            .iter()
            .find(|provider| provider.models.iter().any(|model| model.id == model_id))
    }

    /// The catalog, in order.
    pub fn providers(&self) -> &[ModelProvider] {
        &self.providers
    }

    /// Replaces the catalog. The choice and the favorites stay for models
    /// still in the catalog and are dropped for the others.
    pub fn set_providers(
        &mut self,
        providers: impl IntoIterator<Item = ModelProvider>,
        cx: &mut Context<Self>,
    ) {
        self.providers = providers.into_iter().collect();
        if let Some(id) = &self.selected
            && self.provider_of(id).is_none()
        {
            self.selected = None;
        }
        let providers = &self.providers;
        self.favorites
            .retain(|id| find_model(providers, id).is_some());
        if let ModelView::Provider(id) = &self.view
            && !self.providers.iter().any(|provider| &provider.id == id)
        {
            self.view = self
                .providers
                .first()
                .map(|provider| ModelView::Provider(provider.id.clone()))
                .unwrap_or(ModelView::Favorites);
        }
        self.rebuild(cx);
    }

    /// The id of the chosen model.
    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }

    /// The chosen model and its provider.
    pub fn selected_model(&self) -> Option<(&ModelProvider, &ModelEntry)> {
        let id = self.selected.as_ref()?;
        let provider = self.provider_of(id)?;
        let model = provider.models.iter().find(|model| &model.id == id)?;
        Some((provider, model))
    }

    /// Chooses the model with `id` and emits `Selected`. An unknown id, or
    /// the model already chosen, changes nothing. The rail moves to the
    /// model's provider.
    pub fn select(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        if self.selected.as_ref() == Some(&id) {
            return;
        }
        let Some(provider) = self.provider_of(&id) else {
            return;
        };
        let view = ModelView::Provider(provider.id.clone());
        self.selected = Some(id.clone());
        if self.view != view && self.view != ModelView::Favorites {
            self.view = view.clone();
            cx.emit(ModelPickerEvent::ViewChanged(view));
        }
        cx.emit(ModelPickerEvent::Selected(id));
        self.rebuild(cx);
    }

    /// The effort level of the chosen model, from `0` to `5`, or `None`
    /// when no model is chosen or the chosen model has no effort setting.
    pub fn effort(&self) -> Option<u8> {
        self.selected_model()
            .is_some_and(|(_, model)| model.effort)
            .then_some(self.effort)
    }

    /// The name of the effort level, such as "Medium".
    pub fn effort_label(&self) -> Option<&'static str> {
        self.effort().map(|level| EFFORT_LABELS[usize::from(level)])
    }

    /// Sets the effort to `level`, clamped to the last level, and emits
    /// `EffortChanged`.
    pub fn set_effort(&mut self, level: u8, cx: &mut Context<Self>) {
        let level = level.min((EFFORT_LABELS.len() - 1) as u8);
        if level == self.effort {
            return;
        }
        self.effort = level;
        cx.emit(ModelPickerEvent::EffortChanged(level));
        cx.notify();
    }

    /// The favorite models' ids, in the order they were starred.
    pub fn favorites(&self) -> &[SharedString] {
        &self.favorites
    }

    /// Whether the model with `id` is a favorite.
    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|favorite| favorite == id)
    }

    /// Stars or unstars the model with `id` and emits `FavoriteChanged`.
    /// An unknown id, or a model already in that state, changes nothing.
    pub fn set_favorite(
        &mut self,
        id: impl Into<SharedString>,
        favorite: bool,
        cx: &mut Context<Self>,
    ) {
        let id = id.into();
        if find_model(&self.providers, &id).is_none() || self.is_favorite(&id) == favorite {
            return;
        }
        if favorite {
            self.favorites.push(id.clone());
        } else {
            self.favorites.retain(|existing| existing != &id);
        }
        cx.emit(ModelPickerEvent::FavoriteChanged {
            model: id,
            favorite,
        });
        self.rebuild(cx);
    }

    /// Stars the model if it is not a favorite and unstars it if it is.
    pub fn toggle_favorite(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        let favorite = !self.is_favorite(&id);
        self.set_favorite(id, favorite, cx);
    }

    /// What the rail has selected.
    pub fn view(&self) -> &ModelView {
        &self.view
    }

    /// Shows `view` and clears the search so its models show. A provider
    /// the catalog does not hold changes nothing.
    pub fn set_view(&mut self, view: ModelView, window: &mut Window, cx: &mut Context<Self>) {
        if let ModelView::Provider(id) = &view
            && !self.providers.iter().any(|provider| &provider.id == id)
        {
            return;
        }
        self.set_query("", window, cx);
        if self.view != view {
            self.view = view.clone();
            self.legacy_open = false;
            cx.emit(ModelPickerEvent::ViewChanged(view));
        }
        self.rebuild(cx);
    }

    /// The text of the search.
    pub fn query(&self, cx: &App) -> SharedString {
        self.search.read(cx).value()
    }

    /// Replaces the text of the search.
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        let query = SharedString::from(query.to_string());
        self.search
            .update(cx, |search, cx| search.set_value(query, window, cx));
        self.rebuild(cx);
    }

    /// The models the list shows now, as `(provider, model)` pairs, in
    /// the order of the list.
    pub fn visible(&self) -> Vec<(&ModelProvider, &ModelEntry)> {
        self.rows
            .iter()
            .filter_map(|row| match row {
                Row::Model {
                    provider, model, ..
                } => Some((
                    &self.providers[*provider],
                    &self.providers[*provider].models[*model],
                )),
                Row::Legacy(_) => None,
            })
            .collect()
    }

    /// Whether the list shows the "Legacy models" header.
    pub fn has_legacy_group(&self) -> bool {
        self.rows.iter().any(|row| matches!(row, Row::Legacy(_)))
    }

    /// Whether the legacy group is open.
    pub fn is_legacy_open(&self) -> bool {
        self.legacy_open
    }

    /// Opens or closes the legacy group. It is closed until it is opened,
    /// and closes again when the view changes.
    pub fn set_legacy_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.legacy_open != open {
            self.legacy_open = open;
            self.rebuild(cx);
        }
    }

    /// The id of the model the keyboard or the pointer is on.
    pub fn highlighted(&self) -> Option<SharedString> {
        match self.rows.get(self.highlighted?)? {
            Row::Model {
                provider, model, ..
            } => Some(self.providers[*provider].models[*model].id.clone()),
            Row::Legacy(_) => None,
        }
    }

    /// Whether the panel is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the panel and emits `OpenChanged`. Closing clears
    /// the search.
    pub fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }
        self.open = open;
        if !open {
            self.set_query("", window, cx);
        } else {
            self.rebuild(cx);
        }
        cx.emit(ModelPickerEvent::OpenChanged(open));
        cx.notify();
    }

    /// Chooses the model on the `number`th row of the list, counting from
    /// `1` and only models, as Cmd+1 to Cmd+9 do, and closes the panel.
    pub fn select_number(&mut self, number: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.rows.iter().find_map(|row| match row {
            Row::Model {
                provider,
                model,
                number: Some(n),
            } if *n == number => Some(self.providers[*provider].models[*model].id.clone()),
            _ => None,
        });
        if let Some(id) = id {
            self.select(id, cx);
            self.set_open(false, window, cx);
        }
    }

    /// Recomputes the rows from the search text and the rest of the state.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        self.refresh_rows(&query);
        cx.notify();
    }

    fn refresh_rows(&mut self, query: &str) {
        self.rows = build_rows(
            &self.providers,
            &self.view,
            query,
            &self.favorites,
            self.legacy_open,
        );
        self.list.reset(self.rows.len());
        let chosen = self.selected.as_ref().and_then(|id| {
            self.rows.iter().position(|row| match row {
                Row::Model {
                    provider, model, ..
                } => &self.providers[*provider].models[*model].id == id,
                Row::Legacy(_) => false,
            })
        });
        self.highlighted = chosen.or_else(|| {
            self.rows
                .iter()
                .position(|row| matches!(row, Row::Model { .. }))
        });
        if let Some(row) = self.highlighted {
            self.list.scroll_to_reveal_item(row);
        }
    }

    /// Puts the highlight on `row`, from the pointer.
    fn hover_row(&mut self, row: usize, cx: &mut Context<Self>) {
        if self.highlighted != Some(row) && row < self.rows.len() {
            self.highlighted = Some(row);
            cx.notify();
        }
    }

    /// Moves the highlight one row up or down, wrapping at the ends, and
    /// scrolls it into view.
    fn move_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.rows.len() as isize;
        if count == 0 {
            return;
        }
        let row = match self.highlighted {
            Some(row) => (row as isize + delta).rem_euclid(count),
            None if delta > 0 => 0,
            None => count - 1,
        } as usize;
        self.highlight(row, cx);
    }

    fn highlight(&mut self, row: usize, cx: &mut Context<Self>) {
        self.highlighted = Some(row);
        self.list.scroll_to_reveal_item(row);
        cx.notify();
    }

    fn highlight_edge(&mut self, last: bool, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        self.highlight(if last { self.rows.len() - 1 } else { 0 }, cx);
    }

    /// Chooses what `row` stands for: a model is chosen and the panel
    /// closes, and the legacy header opens or closes its group.
    fn choose_row(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.highlighted = Some(row);
        match self.rows.get(row).copied() {
            Some(Row::Model {
                provider, model, ..
            }) => {
                let id = self.providers[provider].models[model].id.clone();
                self.select(id, cx);
                self.set_open(false, window, cx);
            }
            Some(Row::Legacy(_)) => {
                let open = !self.legacy_open;
                self.set_legacy_open(open, cx);
            }
            None => {}
        }
    }

    /// The menu that lists the effort levels, owned here so the effort menu
    /// and the picker's events stay in one place.
    pub(super) fn effort_menu_state(&self) -> &Entity<MenuState> {
        &self.effort_menu
    }

    /// Whether the keys that move within a query-less list apply: Home and
    /// End belong to the search field while text is typed.
    fn takes_edge_keys(&self, cx: &App) -> bool {
        self.query(cx).is_empty()
    }
}

type RailClick = Box<dyn Fn(&mut Window, &mut App)>;

type TriggerBuilder = Box<dyn FnOnce(bool, &ModelPickerState) -> gpui_kit::AnyElement>;

/// A picker for a model: a trigger pill that shows the chosen one, and a
/// panel with a rail, a search field, and the list of models.
///
/// The rail holds the favorites, a hairline, and one mark per provider; the
/// active entry is brightened and has an accent bar on the rail's edge. The
/// list shows each model on two lines, its name over its provider, with a
/// Cmd+1 to Cmd+9 shortcut on the first nine rows and a star that pins the
/// model to the favorites. A provider's legacy models fold into a group at
/// the top. A search matches model names and provider names across every
/// provider, and the rail is dimmed with no active entry while it has text.
/// The search takes the keyboard when the panel opens, and a letter or `/`
/// typed anywhere in the panel goes to it.
///
/// Up and Down move the highlight and wrap, Home and End jump, Enter and
/// Space choose the highlighted row, and Escape closes the panel. The
/// highlight follows the pointer only while it moves.
///
/// The effort of the chosen model is not in the panel: the
/// [`Composer`](crate::Composer) shows an [`EffortMenu`](crate::EffortMenu)
/// beside the trigger.
///
/// The picker works anywhere: [`trigger`](Self::trigger) replaces the
/// default pill, so a toolbar, a status bar, or a settings page can open
/// it.
///
/// ```no_run
/// use gpui_cn::{ModelEntry, ModelPicker, ModelPickerState, ModelProvider};
/// use gpui_kit::{AppContext as _, Context, Window};
///
/// fn picker(window: &mut Window, cx: &mut Context<()>) -> ModelPicker {
///     let state = cx.new(|cx| {
///         ModelPickerState::new(
///             [ModelProvider::new("codex", "Codex").models([
///                 ModelEntry::new("gpt-5.6-mini", "GPT-5.6 Mini").effort(true),
///             ])],
///             window,
///             cx,
///         )
///         .with_selected("gpt-5.6-mini")
///     });
///     ModelPicker::new("model", &state)
/// }
/// ```
#[derive(IntoElement)]
#[non_exhaustive]
pub struct ModelPicker {
    id: ElementId,
    state: Entity<ModelPickerState>,
    trigger: Option<TriggerBuilder>,
    align: Align,
    icon_only: bool,
    style: StyleRefinement,
}

impl ModelPicker {
    /// A picker on `state`, with a stable id.
    pub fn new(id: impl Into<ElementId>, state: &Entity<ModelPickerState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            trigger: None,
            align: Align::Start,
            icon_only: false,
            style: StyleRefinement::default(),
        }
    }

    /// Replaces the trigger. It shows as selected while the panel is open.
    pub fn trigger(mut self, trigger: impl Selectable + IntoElement + 'static) -> Self {
        self.trigger = Some(Box::new(move |open, _| {
            trigger.open(open).into_any_element()
        }));
        self
    }

    /// Which edge of the trigger the panel lines up with. The default is
    /// `Start`, as a popover's.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Shows the default trigger as the provider's mark alone, with the
    /// model's name in a tooltip and no chevron, as a very narrow toolbar
    /// does. With no model chosen there is no mark, and the option does
    /// nothing.
    pub fn icon_only(mut self, icon_only: bool) -> Self {
        self.icon_only = icon_only;
        self
    }

    /// What the default trigger needs of a row: its width with the name cut
    /// to the minimum, and its width as the provider's mark alone.
    pub(crate) fn need(&self, window: &Window, cx: &App) -> Need {
        let theme = cx.theme();
        let padding = theme.metrics.control_padding_sm;
        let rem = window.rem_size();
        let gap = rems(0.375).to_pixels(rem);
        let read = self.state.read(cx);
        let label = read
            .selected_model()
            .map_or(SharedString::from("Select model"), |(_, model)| {
                model.name.clone()
            });
        let label = collapse::text_width(&label, window)
            .min(collapse::min_label_width(window, collapse::MIN_LABEL_CHARS));
        Need::flexible(
            padding * 2. + rems(1.).to_pixels(rem) + gap + label + gap + rems(0.75).to_pixels(rem),
            read.selected_model().map(|_| theme.metrics.control_sm),
        )
    }
}

/// Styles the panel.
impl Styled for ModelPicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn child(parent: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(parent.clone().into(), name.into())
}

impl RenderOnce for ModelPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let need = self.need(window, cx);
        let id = self.id;
        let state = self.state;
        let (open, search_focus) = {
            let read = state.read(cx);
            (read.open, read.search.read(cx).focus_handle(cx))
        };
        let (width, height, text, radius) = {
            let theme = cx.theme();
            (
                theme.metrics.model_picker_width,
                theme.metrics.model_picker_height,
                theme.text_control,
                theme.radius_xl(),
            )
        };

        let trigger = match self.trigger {
            Some(build) => build(open, state.read(cx)),
            None => {
                let read = state.read(cx);
                let (label, icon) = read.selected_model().map_or_else(
                    || ("Select model".into(), None),
                    |(provider, model)| (model.name.clone(), Some(provider.icon.clone())),
                );
                let (ink, muted) = {
                    let theme = cx.theme();
                    (theme.foreground(), theme.muted_foreground())
                };
                if self.icon_only
                    && let Some(icon) = icon.clone()
                {
                    Button::new(child(&id, "trigger"))
                        .ghost()
                        .size(ButtonSize::Sm)
                        .icon(icon)
                        .accessibility_label(label.clone())
                        .tooltip(label)
                        .text_color(muted)
                        .selected(open)
                        .into_any_element()
                } else {
                    let keep = collapse::text_width(&label, window)
                        .min(collapse::min_label_width(window, collapse::MIN_LABEL_CHARS));
                    let chevron = Icon::from(IconName::ChevronDown)
                        .size_3()
                        .when(open, |icon| icon.rotate(radians(PI)));
                    Button::new(child(&id, "trigger"))
                        .ghost()
                        .size(ButtonSize::Sm)
                        .when_some(icon, |button, icon| button.icon(icon))
                        .child(
                            div()
                                .min_w(keep)
                                .flex_shrink(1.)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_color(ink)
                                .child(label),
                        )
                        .flex_shrink(1.)
                        .min_w(need.min)
                        .trailing_icon(chevron)
                        .text_color(muted)
                        .selected(open)
                        .into_any_element()
                }
            }
        };
        let content = {
            let (id, state) = (id.clone(), state.clone());
            move |window: &mut Window, cx: &mut App| panel(&id, &state, window, cx)
        };
        let on_open_change = {
            let state = state.clone();
            move |open: bool, window: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| state.set_open(open, window, cx));
            }
        };
        Popover::new(id)
            .open(open)
            .on_open_change(on_open_change)
            .align(self.align)
            .track_focus(&search_focus)
            .trigger(Slot { trigger, open })
            .content(content)
            .w(width)
            .h(height)
            .p_0()
            .text_size(text.size)
            .line_height(text.line_height)
            .rounded(radius)
            .overflow_hidden()
            .refine_style(&self.style)
    }
}

/// A built trigger, which the popover marks open through `Selectable`.
/// The picker already built it with the open flag it keeps itself.
#[derive(IntoElement)]
struct Slot {
    trigger: gpui_kit::AnyElement,
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

/// The panel: the rail, the search, and the list.
fn panel(
    id: &ElementId,
    state: &Entity<ModelPickerState>,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let theme = cx.theme();
    let look = MenuLook::of(theme, window.rem_size());
    let disabled_opacity = theme.disabled_opacity;
    let (rail_width, header_height) = (theme.metrics.model_picker_rail, theme.metrics.model_header);
    let row_height = theme.metrics.model_row;
    let pointer_cursors = Theme::global(cx).pointer_cursors;
    let read = state.read(cx);
    let query = read.query(cx);
    let searching = !query.trim().is_empty();
    let search = read.search.clone();
    let search_focus = read.search.read(cx).focus_handle(cx);
    let panel_focus = read.focus.clone();
    let view = read.view.clone();
    let list = read.list.clone();
    let empty = read.rows.is_empty();
    let empty_text = if view == ModelView::Favorites && !searching {
        "Star a model to pin it here"
    } else {
        "No models found"
    };
    let providers = read.providers.clone();

    let entry = |entry_id: ElementId,
                 icon: Icon,
                 label: SharedString,
                 active: bool,
                 on_click: RailClick| {
        let marked = active && !searching;
        div()
            .relative()
            .w_full()
            .py(look.padding / 2.)
            .px(look.padding)
            .child(
                Button::new(entry_id)
                    .ghost()
                    .size(ButtonSize::Lg)
                    .icon(icon)
                    .accessibility_label(label.clone())
                    .tooltip(label)
                    .w_full()
                    .text_color(if marked {
                        look.foreground
                    } else {
                        look.muted_foreground
                    })
                    .selected(marked)
                    .when(marked, |button| button.bg(look.accent))
                    .on_click(move |_, window, cx| on_click(window, cx)),
            )
            .into_any_element()
    };
    let mut marks = vec![entry(
        child(id, "favorites"),
        Icon::from(IconName::StarFill),
        "Favorites".into(),
        view == ModelView::Favorites,
        {
            let state = state.clone();
            Box::new(move |window, cx| {
                state.update(cx, |state, cx| {
                    state.set_view(ModelView::Favorites, window, cx)
                });
            })
        },
    )];
    marks.push(
        div()
            .w_full()
            .py(look.padding)
            .px(look.padding * 2.)
            .child(div().h_px().w_full().bg(look.separator))
            .into_any_element(),
    );
    marks.extend(providers.iter().map(|provider| {
        let provider_id = provider.id.clone();
        entry(
            ElementId::NamedChild(child(id, "provider").into(), provider.id.clone()),
            provider.icon.clone(),
            provider.name.clone(),
            view == ModelView::Provider(provider.id.clone()),
            {
                let state = state.clone();
                Box::new(move |window, cx| {
                    let view = ModelView::Provider(provider_id.clone());
                    state.update(cx, |state, cx| state.set_view(view, window, cx));
                })
            },
        )
    }));
    let rail = ScrollArea::new(child(id, "rail"))
        .size_full()
        .child(div().flex().flex_col().py(look.padding).children(marks));
    let rail = div()
        .id(child(id, "rail-box"))
        .test_support()
        .flex_shrink_0()
        .w(rail_width)
        .h_full()
        .border_r_1()
        .border_color(look.separator)
        .when(searching, |this| this.opacity(disabled_opacity))
        .child(rail);

    let body = if empty {
        div()
            .id(child(id, "empty"))
            .test_support()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(look.muted_foreground)
            .child(empty_text)
            .into_any_element()
    } else {
        let rows = Rows {
            id: id.clone(),
            state: state.clone(),
            look: look.clone(),
            pointer_cursors,
        };
        ScrollArea::list(
            child(id, "models"),
            &list,
            row_height,
            move |row, window, cx| rows.render(row, window, cx),
        )
        .size_full()
        .into_any_element()
    };
    // The list pane keeps the menu's inset on every side, so the fills of
    // rows, and the ring of a focused one, stay clear of the rail's
    // hairline and the panel's border. A slot holds a row with half the
    // gap above and below it, so the pane takes that half off its top and
    // bottom, and a larger step under the underline.
    let slot_gap = look.padding / 2.;
    let list_box = div()
        .id(child(id, "list"))
        .test_support()
        .flex_1()
        .min_h_0()
        .w_full()
        .px(look.padding)
        .pt(look.padding + slot_gap)
        .pb(look.padding - slot_gap)
        .child(body);

    let search_row = search_row(&search, &look, window, cx)
        .id(child(id, "search"))
        .test_support()
        .h(header_height)
        .w_full()
        .mx(look.padding);
    let underline = div()
        .id(child(id, "underline"))
        .test_support()
        .flex_shrink_0()
        .px(look.padding)
        .child(separator(&look));
    let main = v_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .child(div().flex_shrink_0().child(search_row).child(underline))
        .child(list_box);

    let (up, down, confirm, first, last) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
    );
    let (key_state, focus_target) = (state.clone(), search_focus.clone());
    h_flex()
        .id(child(id, "body"))
        .test_support()
        .size_full()
        .items_stretch()
        .key_context(CONTEXT)
        .track_focus(&panel_focus)
        .on_action(move |_: &SelectUp, _, cx| {
            up.update(cx, |state, cx| state.move_highlight(-1, cx));
        })
        .on_action(move |_: &SelectDown, _, cx| {
            down.update(cx, |state, cx| state.move_highlight(1, cx));
        })
        .on_action(move |_: &Confirm, window, cx| {
            confirm.update(cx, |state, cx| {
                if let Some(row) = state.highlighted {
                    state.choose_row(row, window, cx);
                }
            });
        })
        .on_action(move |_: &FirstRow, _, cx| {
            if first.read(cx).takes_edge_keys(cx) {
                first.update(cx, |state, cx| state.highlight_edge(false, cx));
            } else {
                cx.propagate();
            }
        })
        .on_action(move |_: &LastRow, _, cx| {
            if last.read(cx).takes_edge_keys(cx) {
                last.update(cx, |state, cx| state.highlight_edge(true, cx));
            } else {
                cx.propagate();
            }
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let keystroke = &event.keystroke;
            if keystroke.modifiers.secondary() {
                if let Ok(number) = keystroke.key.parse::<usize>()
                    && (1..=SHORTCUT_ROWS).contains(&number)
                {
                    cx.stop_propagation();
                    key_state.update(cx, |state, cx| state.select_number(number, window, cx));
                }
                return;
            }
            let in_search = focus_target.contains_focused(window, cx);
            if keystroke.key == "space" && !keystroke.modifiers.modified() {
                let take = key_state.read(cx).takes_edge_keys(cx);
                if take {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_state.update(cx, |state, cx| {
                        if let Some(row) = state.highlighted {
                            state.choose_row(row, window, cx);
                        }
                    });
                }
                return;
            }
            if !in_search
                && !keystroke.modifiers.modified()
                && let Some(text) = keystroke.key_char.as_ref()
                && !text.is_empty()
            {
                cx.stop_propagation();
                window.focus(&focus_target, cx);
                let search = key_state.read(cx).search.clone();
                let typed = format!("{}{text}", search.read(cx).value());
                search.update(cx, |search, cx| search.set_value(typed, window, cx));
            }
        })
        .child(rail)
        .child(main)
        .into_any_element()
}

/// What a row is drawn from, owned by the list's row builder, which
/// outlives the render that made it.
struct Rows {
    id: ElementId,
    state: Entity<ModelPickerState>,
    look: MenuLook,
    pointer_cursors: bool,
}

impl Rows {
    /// A row in its slot: the slot is the row's height and the row is a
    /// step shorter on both sides, so the fills of two rows never touch.
    fn render(&self, index: usize, window: &mut Window, cx: &mut App) -> gpui_kit::AnyElement {
        let row_height = cx.theme().metrics.model_row;
        div()
            .w_full()
            .h(row_height)
            .py_0p5()
            .child(self.row(index, window, cx))
            .into_any_element()
    }

    fn row(&self, index: usize, window: &mut Window, cx: &mut App) -> gpui_kit::AnyElement {
        let look = &self.look;
        let (row, highlighted, legacy_open) = {
            let state = self.state.read(cx);
            let Some(row) = state.rows.get(index).copied() else {
                return div().into_any_element();
            };
            (row, state.highlighted == Some(index), state.legacy_open)
        };
        let touch = look.touch;
        let hover_state = self.state.clone();
        let click_state = self.state.clone();
        let frame = |frame_id: ElementId| {
            row_frame(look, highlighted && !touch)
                .id(frame_id)
                .test_support()
                .h_full()
                .when(self.pointer_cursors, |this| this.cursor_pointer())
                .when(!touch, |this| {
                    this.on_mouse_move(move |_, _, cx| {
                        hover_state.update(cx, |state, cx| state.hover_row(index, cx));
                    })
                })
        };
        let name_text = cx.theme().text_control;
        let caption_text = cx.theme().base.typography.xs;
        match row {
            Row::Legacy(count) => {
                let progress = transition(
                    ElementId::NamedChild(child(&self.id, "legacy-fade").into(), "x".into()),
                    if legacy_open { 1. } else { 0. },
                    Theme::global(cx).motion.fast_transition(),
                    window,
                    cx,
                );
                frame(child(&self.id, "legacy"))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(name_text.size)
                                    .line_height(name_text.line_height)
                                    .text_color(look.foreground)
                                    .child("Legacy models"),
                            )
                            .child(
                                div()
                                    .text_size(caption_text.size)
                                    .line_height(caption_text.line_height)
                                    .text_color(look.muted_foreground)
                                    .child(crate::plural::counted(count, "model")),
                            ),
                    )
                    .child(
                        Icon::from(IconName::ChevronDown)
                            .size_4()
                            .flex_shrink_0()
                            .text_color(look.muted_foreground)
                            .rotate(radians(PI * progress)),
                    )
                    .on_click(move |_, window, cx| {
                        click_state.update(cx, |state, cx| state.choose_row(index, window, cx));
                    })
                    .into_any_element()
            }
            Row::Model {
                provider,
                model,
                number,
            } => {
                let (provider, model, chosen, favorite) = {
                    let state = self.state.read(cx);
                    let model = state.providers[provider].models[model].clone();
                    (
                        state.providers[provider].clone(),
                        model.clone(),
                        state.selected.as_ref() == Some(&model.id),
                        state.is_favorite(&model.id),
                    )
                };
                let star_state = self.state.clone();
                let model_id = model.id.clone();
                let star_id = model.id.clone();
                let fade = if model.legacy {
                    transition(
                        ElementId::NamedChild(child(&self.id, "legacy-fade").into(), "x".into()),
                        1.,
                        Theme::global(cx).motion.fast_transition(),
                        window,
                        cx,
                    )
                } else {
                    1.
                };
                frame(ElementId::NamedChild(
                    self.id.clone().into(),
                    model.id.clone(),
                ))
                .opacity(fade)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(name_text.size)
                                .line_height(name_text.line_height)
                                .text_color(look.foreground)
                                .child(model.name.clone()),
                        )
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .text_size(caption_text.size)
                                .line_height(caption_text.line_height)
                                .text_color(look.muted_foreground)
                                .child(provider.icon.clone().size_3().flex_shrink_0())
                                .child(provider.name.clone()),
                        ),
                )
                .when(chosen, |this| {
                    this.child(
                        Icon::from(IconName::Check)
                            .size_4()
                            .flex_shrink_0()
                            .text_color(look.foreground),
                    )
                })
                .when_some(number, |this, number| {
                    this.child(
                        div()
                            .id(ElementId::NamedChild(
                                child(&self.id, "shortcut").into(),
                                model.id.clone(),
                            ))
                            .test_support()
                            .flex_shrink_0()
                            .px_1p5()
                            .rounded(look.row_radius)
                            .bg(look.separator)
                            .text_size(caption_text.size)
                            .line_height(caption_text.line_height)
                            .text_color(look.muted_foreground)
                            .child(shortcut_text(number)),
                    )
                })
                .child(
                    Button::new(ElementId::NamedChild(
                        child(&self.id, "star").into(),
                        model.id.clone(),
                    ))
                    .ghost()
                    .size(ButtonSize::Default)
                    .icon(Icon::from(if favorite {
                        IconName::StarFill
                    } else {
                        IconName::Star
                    }))
                    .text_color(if favorite {
                        look.foreground
                    } else {
                        look.muted_foreground
                    })
                    .accessibility_label(if favorite {
                        "Remove from favorites"
                    } else {
                        "Add to favorites"
                    })
                    .on_click(move |_: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        star_state
                            .update(cx, |state, cx| state.toggle_favorite(star_id.clone(), cx));
                    }),
                )
                .on_click(move |_, window, cx| {
                    let _ = &model_id;
                    click_state.update(cx, |state, cx| state.choose_row(index, window, cx));
                })
                .into_any_element()
            }
        }
    }
}

/// The text of a row's number shortcut: the command key and the digit on
/// macOS, Ctrl and the digit elsewhere.
fn shortcut_text(number: usize) -> String {
    if cfg!(target_os = "macos") {
        format!("\u{2318}{number}")
    } else {
        format!("Ctrl+{number}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<ModelProvider> {
        vec![
            ModelProvider::new("acme", "Acme").models([
                ModelEntry::new("acme-1", "Alpha One"),
                ModelEntry::new("acme-old", "Alpha Old").legacy(true),
                ModelEntry::new("acme-2", "Alpha Two"),
            ]),
            ModelProvider::new("zed", "Zed Labs").models([
                ModelEntry::new("zed-1", "Beta"),
                ModelEntry::new("zed-2", "Alpha Z"),
            ]),
        ]
    }

    fn ids(rows: &[Row], providers: &[ModelProvider]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Legacy(count) => format!("legacy:{count}"),
                Row::Model {
                    provider, model, ..
                } => providers[*provider].models[*model].id.to_string(),
            })
            .collect()
    }

    fn rows(view: ModelView, query: &str, favorites: &[&str], open: bool) -> Vec<String> {
        let providers = catalog();
        let favorites: Vec<SharedString> = favorites.iter().map(|id| (*id).into()).collect();
        ids(
            &build_rows(&providers, &view, query, &favorites, open),
            &providers,
        )
    }

    #[test]
    fn a_provider_view_folds_its_legacy_models_under_a_header() {
        let acme = || ModelView::Provider("acme".into());
        assert_eq!(
            rows(acme(), "", &[], false),
            ["legacy:1", "acme-1", "acme-2"]
        );
        assert_eq!(
            rows(acme(), "", &[], true),
            ["legacy:1", "acme-old", "acme-1", "acme-2"]
        );
        assert_eq!(
            rows(ModelView::Provider("zed".into()), "", &[], true),
            ["zed-1", "zed-2"],
            "no legacy models, no header"
        );
    }

    #[test]
    fn a_query_searches_every_provider_flat_by_model_or_provider_name() {
        let acme = || ModelView::Provider("acme".into());
        assert_eq!(
            rows(acme(), "ALPHA", &[], false),
            ["acme-1", "acme-old", "acme-2", "zed-2"]
        );
        assert_eq!(rows(acme(), "beta", &[], false), ["zed-1"]);
        assert_eq!(
            rows(acme(), "zed", &[], false),
            ["zed-1", "zed-2"],
            "a provider name matches all its models"
        );
        assert!(rows(acme(), "nope", &[], false).is_empty());
    }

    #[test]
    fn the_favorites_view_lists_the_starred_models_in_the_order_starred() {
        assert_eq!(
            rows(
                ModelView::Favorites,
                "",
                &["zed-2", "acme-old", "gone"],
                false
            ),
            ["zed-2", "acme-old"]
        );
        assert!(rows(ModelView::Favorites, "", &[], false).is_empty());
    }

    #[test]
    fn only_the_first_nine_models_get_a_number() {
        let providers = vec![
            ModelProvider::new("p", "P")
                .models((0..12).map(|n| ModelEntry::new(format!("m{n}"), format!("Model {n}")))),
        ];
        let rows = build_rows(&providers, &ModelView::Provider("p".into()), "", &[], false);
        let numbers: Vec<_> = rows
            .iter()
            .map(|row| match row {
                Row::Model { number, .. } => *number,
                Row::Legacy(_) => None,
            })
            .collect();
        assert_eq!(numbers[0], Some(1));
        assert_eq!(numbers[8], Some(9));
        assert_eq!(numbers[9], None);
    }

    #[test]
    fn the_effort_labels_run_from_low_to_max() {
        assert_eq!(EFFORT_LABELS.first(), Some(&"Low"));
        assert_eq!(EFFORT_LABELS.last(), Some(&"Max"));
    }
}
