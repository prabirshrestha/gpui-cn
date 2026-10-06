use gpui_kit::{
    App, ElementId, Entity, IntoElement, RenderOnce, StyleRefinement, Styled, Window,
    assets::IconName,
    base::{Align, StyledExt as _},
};

use crate::{
    ActiveTheme as _, Button, ButtonSize, DropdownMenu, EFFORT_LABELS, Icon, MenuEntry, MenuItem,
    ModelPickerState,
};

/// The effort of the chosen model as a small dropdown: the level's name,
/// such as "Medium", and a chevron. Its menu lists the six
/// [`EFFORT_LABELS`] with a check on the current one, and choosing a level
/// sets it on the [`ModelPickerState`], which reports
/// [`ModelPickerEvent::EffortChanged`](crate::ModelPickerEvent::EffortChanged).
///
/// The menu is a [`DropdownMenu`]. It draws nothing while the chosen model
/// has no effort setting, so a toolbar can always include it; use
/// [`ModelPickerState::effort`] to know whether it shows.
///
/// ```no_run
/// use gpui_cn::{EffortMenu, ModelPickerState};
/// use gpui_kit::Entity;
///
/// fn effort(models: &Entity<ModelPickerState>) -> EffortMenu {
///     EffortMenu::new("effort", models)
/// }
/// ```
#[derive(IntoElement)]
#[non_exhaustive]
pub struct EffortMenu {
    id: ElementId,
    state: Entity<ModelPickerState>,
    style: StyleRefinement,
}

impl EffortMenu {
    /// An effort menu on `state`, with a stable id.
    pub fn new(id: impl Into<ElementId>, state: &Entity<ModelPickerState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for EffortMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EffortMenu {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (level, menu) = {
            let state = self.state.read(cx);
            (state.effort(), state.effort_menu_state().clone())
        };
        let Some(level) = level else {
            return gpui_kit::div().into_any_element();
        };
        let muted = cx.theme().composer_muted;
        let trigger = Button::new(ElementId::NamedChild(
            self.id.clone().into(),
            "trigger".into(),
        ))
        .ghost()
        .size(ButtonSize::Sm)
        .accessibility_label("Effort")
        .text_color(muted)
        .label(EFFORT_LABELS[usize::from(level)])
        .trailing_icon(Icon::from(IconName::ChevronDown).size_3());
        let state = self.state.clone();
        DropdownMenu::new(self.id, &menu)
            .align(Align::End)
            .refine_style(&self.style)
            .trigger(trigger)
            .items(move |_, cx| {
                let current = state.read(cx).effort();
                EFFORT_LABELS
                    .iter()
                    .enumerate()
                    .map(|(index, label)| {
                        MenuEntry::from(
                            MenuItem::new(index.to_string(), *label)
                                .checked(current == Some(index as u8)),
                        )
                    })
                    .collect()
            })
            .into_any_element()
    }
}
