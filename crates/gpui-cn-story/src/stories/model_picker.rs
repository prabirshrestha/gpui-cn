use gpui_cn::{
    Button, ButtonSize, EffortMenu, ModelEntry, ModelPicker, ModelPickerEvent, ModelPickerState,
    ModelProvider, ModelView, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div,
};

use crate::{Story, agents, note, page, section};

/// The model picker on its own: the shared catalog, a trigger of the
/// application's own, the favorites view full and empty, catalogs of other
/// shapes, a search, and a picker an outside button reads and resets.
pub struct ModelPickerStory {
    catalog: Entity<ModelPickerState>,
    trigger_only: Entity<ModelPickerState>,
    favorites: Entity<ModelPickerState>,
    no_favorites: Entity<ModelPickerState>,
    single: Entity<ModelPickerState>,
    no_effort: Entity<ModelPickerState>,
    search: Entity<ModelPickerState>,
    controlled: Entity<ModelPickerState>,
    last_event: SharedString,
}

impl ModelPickerStory {
    /// A catalog of one provider, which the trigger-only and the single
    /// provider examples use.
    fn single_catalog() -> Vec<ModelProvider> {
        vec![
            ModelProvider::new("local", "On this machine")
                .icon(IconName::HardDrive)
                .models([
                    ModelEntry::new("tiny", "Tiny 1B"),
                    ModelEntry::new("small", "Small 8B").effort(true),
                    ModelEntry::new("large", "Large 70B").effort(true),
                ]),
        ]
    }

    /// A catalog whose models have no effort setting.
    fn plain_catalog() -> Vec<ModelProvider> {
        vec![
            ModelProvider::new("notes", "Notes")
                .icon(IconName::BookOpen)
                .models([
                    ModelEntry::new("draft", "Draft"),
                    ModelEntry::new("review", "Review"),
                    ModelEntry::new("final", "Final").legacy(true),
                ]),
            ModelProvider::new("code", "Code")
                .icon(IconName::SquareTerminal)
                .models([
                    ModelEntry::new("lint", "Lint"),
                    ModelEntry::new("fmt", "Format"),
                ]),
        ]
    }

    /// Every standalone picker example by name, and whether its catalog is
    /// the labelled custom one.
    pub fn pickers(&self) -> Vec<(&'static str, Entity<ModelPickerState>, bool)> {
        vec![
            ("catalog", self.catalog.clone(), false),
            ("trigger-only", self.trigger_only.clone(), true),
            ("favorites", self.favorites.clone(), false),
            ("no-favorites", self.no_favorites.clone(), false),
            ("single", self.single.clone(), true),
            ("no-effort", self.no_effort.clone(), true),
            ("search", self.search.clone(), false),
            ("controlled", self.controlled.clone(), false),
        ]
    }

    /// The picker an outside button reads and resets.
    pub fn controlled(&self) -> &Entity<ModelPickerState> {
        &self.controlled
    }

    /// The last event the first example reported.
    pub fn last_event(&self) -> &str {
        &self.last_event
    }

    /// Puts the controlled picker back on the model, the favorites, and the
    /// effort it started with.
    pub fn reset_controlled(&self, cx: &mut App) {
        reset(&self.controlled, cx);
    }

    fn describe(event: &ModelPickerEvent) -> SharedString {
        match event {
            ModelPickerEvent::Selected(id) => format!("Selected {id}"),
            ModelPickerEvent::EffortChanged(level) => format!("EffortChanged {level}"),
            ModelPickerEvent::ViewChanged(ModelView::Favorites) => {
                "ViewChanged Favorites".to_string()
            }
            ModelPickerEvent::ViewChanged(ModelView::Provider(id)) => {
                format!("ViewChanged {id}")
            }
            ModelPickerEvent::OpenChanged(open) => format!("OpenChanged {open}"),
            ModelPickerEvent::FavoriteChanged { model, favorite } => {
                format!("FavoriteChanged {model} {favorite}")
            }
        }
        .into()
    }
}

fn reset(state: &Entity<ModelPickerState>, cx: &mut App) {
    state.update(cx, |state, cx| {
        let favorites = state.favorites().to_vec();
        for id in favorites {
            state.set_favorite(id, false, cx);
        }
        for id in agents::FAVORITES {
            state.set_favorite(id, true, cx);
        }
        state.select(agents::DEFAULT_MODEL, cx);
        state.set_effort(1, cx);
    });
}

fn chosen(state: &Entity<ModelPickerState>, cx: &App) -> String {
    let read = state.read(cx);
    match read.selected_model() {
        None => "none".to_string(),
        Some((_, model)) => match read.effort_label() {
            Some(effort) => format!("{} ({effort})", model.name()),
            None => model.name().to_string(),
        },
    }
}

impl Story for ModelPickerStory {
    fn title() -> &'static str {
        "Model picker"
    }

    fn icon() -> IconName {
        IconName::Bot
    }

    fn description() -> &'static str {
        "A rail of favorites and providers, a search, and two-line rows with number \
         shortcuts and stars, as a popover on any trigger."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let catalog = agents::model_picker(window, cx);
            let trigger_only = cx.new(|cx| {
                ModelPickerState::new(Self::single_catalog(), window, cx).with_selected("small")
            });
            let favorites = cx.new(|cx| {
                ModelPickerState::new(agents::catalog(), window, cx)
                    .with_selected(agents::DEFAULT_MODEL)
                    .with_favorites(agents::FAVORITES)
                    .with_view(ModelView::Favorites)
            });
            let no_favorites = cx.new(|cx| {
                ModelPickerState::new(agents::catalog(), window, cx)
                    .with_selected(agents::DEFAULT_MODEL)
                    .with_view(ModelView::Favorites)
            });
            let single = cx.new(|cx| {
                ModelPickerState::new(Self::single_catalog(), window, cx).with_selected("tiny")
            });
            let no_effort = cx.new(|cx| {
                ModelPickerState::new(Self::plain_catalog(), window, cx).with_selected("draft")
            });
            let story = Self {
                catalog: catalog.clone(),
                trigger_only,
                favorites,
                no_favorites,
                single,
                no_effort,
                search: agents::model_picker(window, cx),
                controlled: agents::model_picker(window, cx),
                last_event: "Nothing yet. Open a picker.".into(),
            };
            for (_, state, _) in story.pickers() {
                cx.observe(&state, |_, _, cx| cx.notify()).detach();
            }
            cx.subscribe(
                &catalog,
                |this: &mut Self, _, event: &ModelPickerEvent, cx| {
                    this.last_event = Self::describe(event);
                    cx.notify();
                },
            )
            .detach();
            story
        })
        .into()
    }
}

impl Render for ModelPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row = || div().flex().items_center().gap_4();
        let controlled = self.controlled.clone();
        let search = self.search.clone();
        let favorite_names = |state: &Entity<ModelPickerState>| {
            let favorites = state.read(cx).favorites().to_vec();
            if favorites.is_empty() {
                "none".to_string()
            } else {
                favorites
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        let summary = format!(
            "Model: {}. Favorites: {}.",
            chosen(&self.controlled, cx),
            favorite_names(&self.controlled)
        );
        page([
            section(
                "Catalog",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The example catalog: Codex and Claude, with two favorites and some \
                         legacy models folded at the top of their lists. A search matches \
                         every provider. Press / to focus it, Cmd+1 to Cmd+9 (Ctrl elsewhere) \
                         choose a row, and a star pins a model to the Favorites entry of the \
                         rail. The effort of the chosen model is a menu beside the trigger.",
                        cx,
                    ))
                    .child(
                        row()
                            .child(ModelPicker::new("models", &self.catalog))
                            .child(EffortMenu::new("models-effort", &self.catalog))
                            .child(note(format!("Model: {}", chosen(&self.catalog, cx)), cx)),
                    )
                    .child(note(format!("Last event: {}", self.last_event), cx)),
            )
            .into_any_element(),
            section(
                "Trigger only",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Any button can open the picker: pass it as the trigger, and the \
                         picker marks it open while the panel shows.",
                        cx,
                    ))
                    .child(
                        row()
                            .child(
                                ModelPicker::new("trigger-only", &self.trigger_only).trigger(
                                    Button::new("trigger-only-button")
                                        .outline()
                                        .size(ButtonSize::Sm)
                                        .icon(IconName::Cpu)
                                        .label("Choose a local model"),
                                ),
                            )
                            .child(note(
                                format!("Model: {}", chosen(&self.trigger_only, cx)),
                                cx,
                            )),
                    ),
            )
            .into_any_element(),
            section(
                "Favorites",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Both pickers open on the Favorites entry. The first starts with two \
                         models starred. The second starts with none and says how to add one. \
                         The application keeps the favorites: it saves them when the picker \
                         reports FavoriteChanged.",
                        cx,
                    ))
                    .child(
                        row()
                            .child(ModelPicker::new("favorites", &self.favorites))
                            .child(ModelPicker::new("no-favorites", &self.no_favorites)),
                    ),
            )
            .into_any_element(),
            section(
                "Other catalogs",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The first catalog has one provider, so its rail holds the favorites and that \
                         provider. The second has models with no effort setting, so no effort \
                         menu shows beside it.",
                        cx,
                    ))
                    .child(
                        row()
                            .child(ModelPicker::new("single", &self.single))
                            .child(EffortMenu::new("single-effort", &self.single))
                            .child(note(format!("Model: {}", chosen(&self.single, cx)), cx)),
                    )
                    .child(
                        row()
                            .child(ModelPicker::new("no-effort", &self.no_effort))
                            .child(EffortMenu::new("no-effort-effort", &self.no_effort))
                            .child(note(format!("Model: {}", chosen(&self.no_effort, cx)), cx)),
                    ),
            )
            .into_any_element(),
            section(
                "Search",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The search matches model and provider names across every provider, \
                         and the rail is dimmed while it has text. This button opens a picker \
                         with the search already typed.",
                        cx,
                    ))
                    .child(
                        row().child(ModelPicker::new("search", &self.search)).child(
                            Button::new("search-open")
                                .outline()
                                .size(ButtonSize::Sm)
                                .label("Search for opus")
                                .on_click(move |_, window, cx| {
                                    search.update(cx, |state, cx| {
                                        state.set_open(true, window, cx);
                                        state.set_query("opus", window, cx);
                                    });
                                }),
                        ),
                    ),
            )
            .into_any_element(),
            section(
                "Controlled",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The picker's state is plain data the application can read and set: \
                         the text below reads the chosen model, its effort, and the \
                         favorites, and Reset puts them back.",
                        cx,
                    ))
                    .child(
                        row()
                            .child(ModelPicker::new("controlled", &self.controlled))
                            .child(EffortMenu::new("controlled-effort", &self.controlled))
                            .child(
                                Button::new("controlled-reset")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .label("Reset")
                                    .on_click(move |_, _, cx| reset(&controlled, cx)),
                            ),
                    )
                    .child(note(summary, cx)),
            )
            .into_any_element(),
            section(
                "Events",
                note(
                    "ModelPickerState reports Selected, EffortChanged, ViewChanged, \
                     OpenChanged, and FavoriteChanged. Selection and favorites are kept by \
                     model id, so a catalog that changes keeps the ones still in it.",
                    cx,
                ),
            )
            .into_any_element(),
        ])
    }
}
