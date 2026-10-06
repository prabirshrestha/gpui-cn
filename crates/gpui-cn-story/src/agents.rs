//! Example data for a coding-agent composer: a model catalog of two
//! providers and a permission set.
//!
//! It is only data. Nothing here is behavior of the library, and nothing
//! is a vendor mark: each provider carries a generic Lucide icon.

use gpui_cn::{
    ComposerState, ComposerStatusTab, ModelEntry, ModelPickerState, ModelProvider, PermissionMode,
    PermissionState, PermissionTone, StatusOption, StatusSelect, StatusSelectState,
    gpui_kit::assets::IconName,
};
use gpui_kit::{App, Context, ElementId, Entity, Window};

/// The model every sample starts on.
pub const DEFAULT_MODEL: &str = "gpt-5.6-mini";

/// The two providers: Codex and Claude. This is the one catalog every
/// composer and model picker sample in the story uses.
pub fn catalog() -> Vec<ModelProvider> {
    vec![
        ModelProvider::new("codex", "Codex")
            .icon(IconName::SquareTerminal)
            .models([
                ModelEntry::new("gpt-5.6-mini", "GPT-5.6 Mini").effort(true),
                ModelEntry::new("gpt-5.6-terra", "GPT-5.6 Terra").effort(true),
                ModelEntry::new("gpt-5.6-sol", "GPT-5.6 Sol").effort(true),
                ModelEntry::new("gpt-5.5", "GPT-5.5").effort(true),
                ModelEntry::new("gpt-5.5-mini", "GPT-5.5 Mini").effort(true),
                ModelEntry::new("gpt-5.4", "GPT-5.4").effort(true),
            ]),
        ModelProvider::new("claude", "Claude")
            .icon(IconName::Star)
            .models([
                ModelEntry::new("fable-5.1", "Fable 5.1").effort(true),
                ModelEntry::new("opus-5.5", "Opus 5.5").effort(true),
                ModelEntry::new("sonnet-5.5", "Sonnet 5.5").effort(true),
                ModelEntry::new("haiku-4.5", "Haiku 4.5"),
            ]),
    ]
}

/// The permission modes of a Claude Code style agent. The first is the
/// default.
pub fn permission_modes() -> Vec<PermissionMode> {
    vec![
        PermissionMode::new("default", "Default")
            .description("Ask before editing files or running commands")
            .icon(IconName::Hand),
        PermissionMode::new("accept-edits", "Accept edits")
            .description("Edit files freely, ask before commands")
            .icon(IconName::Check),
        PermissionMode::new("plan", "Plan")
            .description("Read only: propose a plan and wait")
            .icon(IconName::ClipboardList),
        PermissionMode::new("bypass", "Bypass permissions")
            .description("Run everything without asking")
            .icon(IconName::ShieldAlert)
            .tone(PermissionTone::Warning),
    ]
}

/// A model picker state over the shared catalog, on the default model.
/// Each sample takes its own: two pickers on one state open together.
pub fn model_picker<T: 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
) -> Entity<ModelPickerState> {
    use gpui_kit::AppContext as _;
    cx.new(|cx| ModelPickerState::new(catalog(), window, cx).with_selected(DEFAULT_MODEL))
}

/// A composer state with the shared catalog on the default model, and the
/// permission modes above with the risky one chosen to show its tone.
pub fn composer<T: 'static>(window: &mut Window, cx: &mut Context<T>) -> Entity<ComposerState> {
    use gpui_kit::AppContext as _;
    let models = model_picker(window, cx);
    let permission = cx.new(|cx| {
        PermissionState::new(cx)
            .with_modes(permission_modes())
            .with_selected("bypass")
    });
    cx.new(|cx| {
        ComposerState::new(window, cx)
            .with_models(models, window, cx)
            .with_permission(permission, window, cx)
    })
}

/// The three dropdowns of a sample status tab: a project, a device, and a
/// branch, each with its own state.
pub struct SampleStatus {
    /// The project folder.
    pub project: Entity<StatusSelectState>,
    /// The device.
    pub device: Entity<StatusSelectState>,
    /// The git branch.
    pub branch: Entity<StatusSelectState>,
}

impl SampleStatus {
    /// Sample options: projects sample-app, northwind, and playground;
    /// devices Local and Cloud; branches feature/composer, main, and
    /// fix/popover. The first of each is chosen.
    pub fn new<T: 'static>(cx: &mut Context<T>) -> Self {
        use gpui_kit::AppContext as _;
        let options = |names: &[&str]| -> Vec<StatusOption> {
            names
                .iter()
                .map(|name| StatusOption::new(name.to_string(), name.to_string()))
                .collect()
        };
        Self {
            project: cx.new(|cx| {
                StatusSelectState::new(options(&["sample-app", "northwind", "playground"]), cx)
            }),
            device: cx.new(|cx| StatusSelectState::new(options(&["Local", "Cloud"]), cx)),
            branch: cx.new(|cx| {
                StatusSelectState::new(options(&["feature/composer", "main", "fix/popover"]), cx)
            }),
        }
    }

    /// The tab: the three dropdowns, ids derived from `id`.
    pub fn tab(&self, id: &'static str) -> ComposerStatusTab {
        let child =
            |name: &'static str| ElementId::NamedChild(ElementId::from(id).into(), name.into());
        ComposerStatusTab::new(id)
            .select(StatusSelect::new(child("project"), &self.project).icon(IconName::Folder))
            .select(StatusSelect::new(child("device"), &self.device).icon(IconName::Laptop))
            .select(StatusSelect::new(child("branch"), &self.branch).icon(IconName::GitBranch))
    }

    /// Forwards the three dropdowns' changes to `composer` as status
    /// events named project, device, and branch.
    pub fn watch(&self, composer: &Entity<ComposerState>, cx: &mut App) {
        composer.update(cx, |state, cx| {
            state.watch_status("project", &self.project, cx);
            state.watch_status("device", &self.device, cx);
            state.watch_status("branch", &self.branch, cx);
        });
    }
}
