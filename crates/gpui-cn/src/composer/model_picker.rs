use std::f32::consts::PI;

use gpui_kit::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, RenderOnce,
    SharedString, StyleRefinement, Styled, Subscription, Window,
    assets::IconName,
    base::{
        Align, Radio as BaseRadio, Selectable, StyledExt as _, TestSupportExt as _,
        actions::Cancel,
        input::{InputEvent, InputState},
        slider::{SliderEvent, SliderState},
    },
    div,
    prelude::FluentBuilder as _,
    px, radians,
};

use crate::{
    ActiveTheme as _, Button, ButtonSize, Icon, Input, Popover, RadioMark, ScrollArea, Slider,
    menu::{MenuLook, search_style},
};

/// The names of the six effort levels, from the lowest to the highest.
///
/// The levels are the stops of the effort slider: a model that supports
/// effort runs at one of them, `Faster` at the first and `Smarter` at the
/// last.
pub const EFFORT_LABELS: [&str; 6] = ["Low", "Medium", "High", "Extra high", "Ultra", "Max"];

/// The effort a model starts at when it is first chosen: `Medium`.
const DEFAULT_EFFORT: u8 = 1;

/// A model a [`ModelPicker`] offers.
///
/// The id comes from the domain and is unique across the whole catalog,
/// every provider included.
///
/// ```
/// use gpui_cn::ModelEntry;
///
/// let model = ModelEntry::new("mini", "GPT Mini").effort(true);
/// assert!(model.supports_effort());
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ModelEntry {
    id: SharedString,
    name: SharedString,
    effort: bool,
}

impl ModelEntry {
    /// A model with a stable id and the name it shows. It has no effort
    /// setting.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            effort: false,
        }
    }

    /// Whether the model has an effort setting. A model that does shows an
    /// effort chip while it is the chosen one.
    pub fn effort(mut self, supported: bool) -> Self {
        self.effort = supported;
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
/// let provider = ModelProvider::new("acme", "Acme")
///     .icon(IconName::Bot)
///     .models([ModelEntry::new("acme-1", "Acme One")]);
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

/// What a [`ModelPickerState`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelPickerEvent {
    /// A model was chosen. The payload is its id.
    Selected(SharedString),
    /// The effort level changed, from `0` to `5`.
    EffortChanged(u8),
    /// The rail's provider changed. The payload is its id.
    ProviderChanged(SharedString),
    /// The panel opened or closed.
    OpenChanged(bool),
}

/// The catalog, the chosen model and its effort, the rail's provider, the
/// quick search, and the open panel of a [`ModelPicker`].
///
/// Owned by the view that shows the picker, which observes it so a change
/// renders. The choice is kept by model id, so a catalog that changes
/// under it keeps the choice when the model is still there.
pub struct ModelPickerState {
    providers: Vec<ModelProvider>,
    selected: Option<SharedString>,
    effort: u8,
    active: SharedString,
    search: Entity<InputState>,
    slider: Entity<SliderState>,
    open: bool,
    effort_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ModelPickerEvent> for ModelPickerState {}

/// The quick search's focus, which holds the keyboard while the panel is
/// open.
impl gpui_kit::Focusable for ModelPickerState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }
}

/// The models that show for a query, as `(provider, model)` indices: an
/// empty query shows the active provider's models, and a query shows every
/// model of every provider whose name or whose provider's name contains
/// it, without regard to case.
fn visible_models(providers: &[ModelProvider], active: &str, query: &str) -> Vec<(usize, usize)> {
    let query = query.trim().to_lowercase();
    providers
        .iter()
        .enumerate()
        .filter(|(_, provider)| query.is_empty() && provider.id == active || !query.is_empty())
        .flat_map(|(p, provider)| {
            let provider_matches = provider.name.to_lowercase().contains(&query);
            let query = query.clone();
            provider
                .models
                .iter()
                .enumerate()
                .filter(move |(_, model)| {
                    query.is_empty()
                        || provider_matches
                        || model.name.to_lowercase().contains(&query)
                })
                .map(move |(m, _)| (p, m))
        })
        .collect()
}

impl ModelPickerState {
    /// A picker over `providers` with nothing chosen. The rail starts on
    /// the first provider.
    pub fn new(
        providers: impl IntoIterator<Item = ModelProvider>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let providers: Vec<_> = providers.into_iter().collect();
        let style = search_style(cx.theme());
        let search = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Quick Search");
            input.set_editor_style(style);
            input
        });
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max((EFFORT_LABELS.len() - 1) as f32)
                .step(1.)
                .default_value(f32::from(DEFAULT_EFFORT))
        });
        let subscriptions = vec![
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&slider, |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Change(value) = event {
                    this.set_effort(value.end().round() as u8, cx);
                }
            }),
        ];
        Self {
            active: providers
                .first()
                .map(|provider| provider.id.clone())
                .unwrap_or_default(),
            providers,
            selected: None,
            effort: DEFAULT_EFFORT,
            search,
            slider,
            open: false,
            effort_open: false,
            _subscriptions: subscriptions,
        }
    }

    /// Chooses the model with `id`, if the catalog has it. The rail moves
    /// to its provider.
    pub fn with_selected(mut self, id: impl Into<SharedString>) -> Self {
        let id = id.into();
        if let Some(provider) = self.provider_of(&id) {
            self.active = provider.id.clone();
            self.selected = Some(id);
        }
        self
    }

    /// Starts the effort at `level`, clamped to the last level.
    pub fn with_effort(mut self, level: u8) -> Self {
        self.effort = level.min((EFFORT_LABELS.len() - 1) as u8);
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

    /// Replaces the catalog. The choice stays when its model is still in
    /// the catalog and is cleared when it is not.
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
        if !self
            .providers
            .iter()
            .any(|provider| provider.id == self.active)
        {
            self.active = self
                .providers
                .first()
                .map(|provider| provider.id.clone())
                .unwrap_or_default();
        }
        cx.notify();
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
        let provider = provider.id.clone();
        self.selected = Some(id.clone());
        self.effort_open = false;
        if self.active != provider {
            self.active = provider.clone();
            cx.emit(ModelPickerEvent::ProviderChanged(provider));
        }
        cx.emit(ModelPickerEvent::Selected(id));
        cx.notify();
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

    /// The id of the provider the rail shows.
    pub fn active_provider(&self) -> &SharedString {
        &self.active
    }

    /// Shows the models of the provider with `id`, and clears the quick
    /// search so they show.
    pub fn set_active_provider(
        &mut self,
        id: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = id.into();
        if !self.providers.iter().any(|provider| provider.id == id) {
            return;
        }
        self.set_query("", window, cx);
        if self.active != id {
            self.active = id.clone();
            cx.emit(ModelPickerEvent::ProviderChanged(id));
        }
        cx.notify();
    }

    /// The text of the quick search.
    pub fn query(&self, cx: &App) -> SharedString {
        self.search.read(cx).value()
    }

    /// Replaces the text of the quick search.
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        let query = SharedString::from(query.to_string());
        self.search
            .update(cx, |search, cx| search.set_value(query, window, cx));
        cx.notify();
    }

    /// The models that show now, as `(provider, model)` pairs.
    pub fn visible(&self, cx: &App) -> Vec<(&ModelProvider, &ModelEntry)> {
        visible_models(&self.providers, &self.active, &self.query(cx))
            .into_iter()
            .map(|(p, m)| (&self.providers[p], &self.providers[p].models[m]))
            .collect()
    }

    /// Whether the panel is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the panel and emits `OpenChanged`. Closing clears
    /// the quick search and closes the effort card.
    pub fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }
        self.open = open;
        if !open {
            self.effort_open = false;
            self.set_query("", window, cx);
        }
        cx.emit(ModelPickerEvent::OpenChanged(open));
        cx.notify();
    }

    /// Whether the effort card is open.
    pub fn is_effort_open(&self) -> bool {
        self.effort_open
    }

    /// Opens or closes the effort card.
    pub fn set_effort_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.effort_open != open {
            self.effort_open = open;
            cx.notify();
        }
    }
}

type TriggerBuilder = Box<dyn FnOnce(bool, &ModelPickerState) -> gpui_kit::AnyElement>;

/// A picker for a model: a trigger that shows the chosen one, and a panel
/// with a rail of providers, a quick search, and the models, each with a
/// radio mark. The chosen model shows an effort chip, which opens a card
/// under its row with a slider from `Faster` to `Smarter`. The card is part
/// of the panel, not a second popover, so a press on it is never a press
/// outside the panel.
///
/// A quick search matches model names and provider names across every
/// provider. Press `/` while the panel is open to focus it.
///
/// The picker works anywhere: [`trigger`](Self::trigger) replaces the
/// default button, so a toolbar, a status bar, or a settings page can open
/// it.
///
/// ```no_run
/// use gpui_cn::{ModelEntry, ModelPicker, ModelPickerState, ModelProvider};
/// use gpui_kit::{AppContext as _, Context, Window};
///
/// fn picker(window: &mut Window, cx: &mut Context<()>) -> ModelPicker {
///     let state = cx.new(|cx| {
///         ModelPickerState::new(
///             [ModelProvider::new("acme", "Acme").models([
///                 ModelEntry::new("acme-1", "Acme One").effort(true),
///             ])],
///             window,
///             cx,
///         )
///         .with_selected("acme-1")
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id;
        let state = self.state;
        let (open, search_focus) = {
            let read = state.read(cx);
            (read.open, read.search.read(cx).focus_handle(cx))
        };
        let width = cx.theme().metrics.model_picker_width;

        let trigger = match self.trigger {
            Some(build) => build(open, state.read(cx)),
            None => {
                let read = state.read(cx);
                let label = read
                    .selected_model()
                    .map_or("Select model".into(), |(_, model)| model.name.clone());
                let chevron =
                    Icon::from(IconName::ChevronDown).when(open, |icon| icon.rotate(radians(PI)));
                Button::new(child(&id, "trigger"))
                    .ghost()
                    .size(ButtonSize::Sm)
                    .label(label)
                    .trailing_icon(chevron)
                    .selected(open)
                    .into_any_element()
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

/// The panel: the rail, the search and the models, and the effort card.
fn panel(
    id: &ElementId,
    state: &Entity<ModelPickerState>,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let look = MenuLook::of(cx.theme(), window.rem_size());
    let (rail_width, row_height, list_height, card_height) = {
        let metrics = &cx.theme().metrics;
        (
            metrics.model_picker_rail,
            metrics.model_row,
            metrics.model_list_height,
            metrics.effort_card_height,
        )
    };
    let touch = cx.theme().touch;
    let read = state.read(cx);
    let query = read.query(cx);
    let search = read.search.clone();
    let slider = read.slider.clone();
    let effort_open = read.effort_open;
    let selected = read.selected.clone();
    let effort = read.effort();
    let active = read.active.clone();
    let providers = read.providers.clone();
    let rows: Vec<(ModelProvider, ModelEntry)> = visible_models(&providers, &active, &query)
        .into_iter()
        .map(|(p, m)| (providers[p].clone(), providers[p].models[m].clone()))
        .collect();
    let search_focus = search.read(cx).focus_handle(cx);

    if let Some(level) = effort {
        let current = slider.read(cx).value().end();
        if (current - f32::from(level)).abs() > f32::EPSILON {
            slider.update(cx, |slider, cx| {
                slider.set_value(f32::from(level), window, cx)
            });
        }
    }

    let rail = ScrollArea::new(child(id, "rail"))
        .w(rail_width)
        .h_full()
        .flex_shrink_0()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .p_1()
                .children(providers.iter().map(|provider| {
                    let (state, provider_id) = (state.clone(), provider.id.clone());
                    Button::new(ElementId::NamedChild(
                        child(id, "provider").into(),
                        provider.id.clone(),
                    ))
                    .ghost()
                    .size(ButtonSize::Lg)
                    .icon(provider.icon.clone())
                    .accessibility_label(provider.name.clone())
                    .tooltip(provider.name.clone())
                    .selected(provider.id == active && query.trim().is_empty())
                    .when(provider.id == active && query.trim().is_empty(), |button| {
                        button.bg(look.accent)
                    })
                    .on_click(move |_, window, cx| {
                        let provider_id = provider_id.clone();
                        state.update(cx, |state, cx| {
                            state.set_active_provider(provider_id, window, cx)
                        });
                    })
                })),
        );

    let effort_card = effort_open && effort.is_some();
    let model_rows = rows.iter().flat_map(|(provider, model)| {
        let checked = selected.as_ref() == Some(&model.id);
        let select = {
            let (state, model_id) = (state.clone(), model.id.clone());
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                let model_id = model_id.clone();
                state.update(cx, |state, cx| {
                    state.select(model_id, cx);
                    state.set_open(false, window, cx);
                });
            }
        };
        let chip = (checked && model.effort).then(|| {
            let toggle = state.clone();
            Button::new(child(id, "effort"))
                .size(ButtonSize::Xs)
                .rounded_full()
                .label(
                    effort
                        .map(|level| EFFORT_LABELS[usize::from(level)])
                        .unwrap_or_default(),
                )
                .trailing_icon(
                    Icon::from(IconName::ChevronRight)
                        .size_3()
                        .when(effort_open, |icon| icon.rotate(radians(PI / 2.))),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    toggle.update(cx, |state, cx| {
                        let open = !state.effort_open;
                        state.set_effort_open(open, cx);
                    });
                })
        });
        let row = BaseRadio::new(ElementId::NamedChild(id.clone().into(), model.id.clone()))
            .checked(checked)
            .accessibility_label(format!("{} ({})", model.name, provider.name))
            .on_change(move |_, event, window, cx| select(event, window, cx))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .w_full()
            .h(row_height)
            .px(look.row_padding)
            .rounded(look.row_radius)
            .cursor_pointer()
            .when(checked, |this| this.bg(look.accent))
            .when(!touch, |this| this.hover(|style| style.bg(look.accent)))
            .child(
                provider
                    .icon
                    .clone()
                    .size_4()
                    .flex_shrink_0()
                    .text_color(look.muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(model.name.clone()),
            )
            .children(chip)
            .child(RadioMark::new(checked));
        let card = (checked && model.effort && effort_card)
            .then(|| effort_card_view(id, &slider, effort.unwrap_or_default(), card_height, &look));
        [Some(row.into_any_element()), card]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    });

    let list: gpui_kit::AnyElement = if rows.is_empty() {
        div()
            .h(row_height)
            .flex()
            .items_center()
            .justify_center()
            .text_color(look.muted_foreground)
            .child("No models found")
            .into_any_element()
    } else {
        let extra = if effort_card
            && rows
                .iter()
                .any(|(_, model)| selected.as_ref() == Some(&model.id))
        {
            card_height
        } else {
            px(0.)
        };
        let height = (row_height * rows.len() as f32 + extra).min(list_height);
        ScrollArea::new(child(id, "models"))
            .w_full()
            .h(height)
            .child(div().flex().flex_col().children(model_rows))
            .into_any_element()
    };

    let main = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(
            div()
                .h(row_height)
                .px(look.row_padding)
                .flex()
                .items_center()
                .text_color(look.muted_foreground)
                .child("Models"),
        )
        .child(
            div().px_1().child(
                Input::new(&search)
                    .id(child(id, "search"))
                    .accessibility_label("Quick Search")
                    .prefix(
                        Icon::from(IconName::Search)
                            .size_4()
                            .text_color(look.muted_foreground),
                    )
                    .cleanable(true),
            ),
        )
        .child(div().p_1().child(list));

    let key_state = state.clone();
    div()
        .relative()
        .flex()
        .w_full()
        .items_stretch()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "/" && !search_focus.is_focused(window) {
                cx.stop_propagation();
                window.focus(&search_focus, cx);
            }
        })
        .on_action(move |_: &Cancel, _, cx| {
            if key_state.read(cx).effort_open {
                key_state.update(cx, |state, cx| state.set_effort_open(false, cx));
            } else {
                cx.propagate();
            }
        })
        .child(rail)
        .child(main)
        .into_any_element()
}

/// The effort card, which opens under the chosen model's row: the level's
/// name, a slider from the first level to the last, and what its ends mean.
fn effort_card_view(
    id: &ElementId,
    slider: &Entity<SliderState>,
    level: u8,
    height: gpui_kit::Pixels,
    look: &MenuLook,
) -> gpui_kit::AnyElement {
    div()
        .id(child(id, "effort-card"))
        .test_support()
        .flex_shrink_0()
        .h(height)
        .mx_1()
        .mb_1()
        .px_3()
        .flex()
        .flex_col()
        .justify_center()
        .gap_1()
        .rounded(look.row_radius)
        .bg(look.accent)
        .child(
            div()
                .flex()
                .justify_between()
                .child(div().text_color(look.muted_foreground).child("Effort"))
                .child(div().child(EFFORT_LABELS[usize::from(level)])),
        )
        .child(
            Slider::new(slider)
                .id(child(id, "effort-slider"))
                .accessibility_label("Effort")
                .stops(EFFORT_LABELS.len()),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .text_color(look.muted_foreground)
                .child("Faster")
                .child("Smarter"),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<ModelProvider> {
        vec![
            ModelProvider::new("acme", "Acme").models([
                ModelEntry::new("acme-1", "Alpha One"),
                ModelEntry::new("acme-2", "Alpha Two"),
            ]),
            ModelProvider::new("zed", "Zed Labs").models([
                ModelEntry::new("zed-1", "Beta"),
                ModelEntry::new("zed-2", "Alpha Z"),
            ]),
        ]
    }

    #[test]
    fn an_empty_query_shows_the_active_provider_only() {
        assert_eq!(visible_models(&catalog(), "zed", ""), [(1, 0), (1, 1)]);
        assert_eq!(visible_models(&catalog(), "acme", "  "), [(0, 0), (0, 1)]);
    }

    #[test]
    fn a_query_searches_every_provider_by_model_or_provider_name() {
        assert_eq!(
            visible_models(&catalog(), "acme", "ALPHA"),
            [(0, 0), (0, 1), (1, 1)]
        );
        assert_eq!(visible_models(&catalog(), "acme", "beta"), [(1, 0)]);
        assert_eq!(
            visible_models(&catalog(), "acme", "zed"),
            [(1, 0), (1, 1)],
            "a provider name matches all its models"
        );
        assert!(visible_models(&catalog(), "acme", "nope").is_empty());
    }

    #[test]
    fn the_effort_labels_run_from_low_to_max() {
        assert_eq!(EFFORT_LABELS.first(), Some(&"Low"));
        assert_eq!(EFFORT_LABELS.last(), Some(&"Max"));
    }
}
