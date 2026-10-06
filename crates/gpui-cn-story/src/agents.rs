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

/// The models starred at the start: one of each provider.
pub const FAVORITES: [&str; 2] = ["gpt-5.6-terra", "sonnet-5.5"];

/// The mark of the Codex provider: a terminal.
pub const CODEX_ICON: IconName = IconName::SquareTerminal;

/// The mark of the Claude provider: an asterisk. No provider mark is a
/// star, a heart, a check, a chevron, or a search glyph, which are the
/// picker's own controls.
pub const CLAUDE_ICON: IconName = IconName::Asterisk;

/// The two providers: Codex and Claude. This is the one catalog every
/// composer and model picker sample in the story uses. Each provider has a
/// stand-in Lucide mark: the picker takes any icon, and an application
/// passes its own brand mark there.
pub fn catalog() -> Vec<ModelProvider> {
    vec![
        ModelProvider::new("codex", "Codex")
            .icon(CODEX_ICON)
            .models([
                ModelEntry::new("gpt-5.6-mini", "GPT-5.6 Mini").effort(true),
                ModelEntry::new("gpt-5.6-terra", "GPT-5.6 Terra").effort(true),
                ModelEntry::new("gpt-5.6-sol", "GPT-5.6 Sol").effort(true),
                ModelEntry::new("gpt-5.5", "GPT-5.5").effort(true),
                ModelEntry::new("gpt-5.5-mini", "GPT-5.5 Mini")
                    .effort(true)
                    .legacy(true),
                ModelEntry::new("gpt-5.4", "GPT-5.4")
                    .effort(true)
                    .legacy(true),
            ]),
        ModelProvider::new("claude", "Claude")
            .icon(CLAUDE_ICON)
            .models([
                ModelEntry::new("fable-5.1", "Fable 5.1").effort(true),
                ModelEntry::new("opus-5.5", "Opus 5.5").effort(true),
                ModelEntry::new("sonnet-5.5", "Sonnet 5.5").effort(true),
                ModelEntry::new("haiku-4.5", "Haiku 4.5").legacy(true),
            ]),
    ]
}

/// About thirty refs, as a repository lists them: the current branch, a
/// worktree, local branches, and remote ones, some with long names.
fn refs() -> Vec<StatusOption> {
    let mut refs = vec![
        StatusOption::new("main", "main").trailing("current"),
        StatusOption::new("feat/local-tts", "feat/local-tts").trailing("worktree"),
        StatusOption::new("feature/composer", "feature/composer"),
        StatusOption::new("fix/popover", "fix/popover"),
        StatusOption::new("release/0.4", "release/0.4"),
    ];
    for name in [
        "origin/main",
        "origin/renovate/mermaid-12.x",
        "origin/renovate/dotenv-18.x",
        "origin/renovate/dotenv-17.x",
        "origin/renovate/anthropic-ai-sdk-0.x",
        "origin/fix/psmode-current-evidence",
        "origin/fix/psmode-current-turn-evidence-x-evidence",
        "origin/feat/model-picker-favorites",
        "origin/feat/status-select-search-list",
        "origin/feat/file-picker-new-folder-support",
        "origin/docs/github-cli-attachments",
        "origin/docs/composer-guide",
        "origin/chore/deps-update-nucleo",
        "origin/chore/ci-cache",
        "origin/refactor/path-browser",
        "origin/refactor/progress-ring-into-progress",
        "origin/test/headless-picker-footers",
        "origin/release/0.3",
        "origin/release/0.2",
        "origin/hotfix/dialog-focus-restore",
        "origin/spike/command-dialog",
        "origin/wip/theme-color-page",
        "origin/wip/effort-menu",
        "origin/dependabot/cargo/smol-2",
    ] {
        refs.push(StatusOption::new(name, name).trailing("remote"));
    }
    refs
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
            .icon(IconName::LockOpen)
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
    cx.new(|cx| {
        ModelPickerState::new(catalog(), window, cx)
            .with_selected(DEFAULT_MODEL)
            .with_favorites(FAVORITES)
    })
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
    /// Sample options: twelve projects, devices Local and Cloud, and about
    /// thirty refs with kinds, so the search lists have something to
    /// search. The first of each is chosen.
    pub fn new<T: 'static>(cx: &mut Context<T>) -> Self {
        use gpui_kit::AppContext as _;
        let options = |names: &[&str]| -> Vec<StatusOption> {
            names
                .iter()
                .map(|name| StatusOption::new(name.to_string(), name.to_string()))
                .collect()
        };
        let projects = options(&[
            "sample-app",
            "northwind",
            "playground",
            "billing-service",
            "design-system",
            "docs-site",
            "mobile-client",
            "data-pipeline",
            "infra-config",
            "internal-tools",
            "marketing-site",
            "release-notes",
        ]);
        Self {
            project: cx.new(|cx| StatusSelectState::new(projects, cx)),
            device: cx.new(|cx| StatusSelectState::new(options(&["Local", "Cloud"]), cx)),
            branch: cx.new(|cx| StatusSelectState::new(refs(), cx)),
        }
    }

    /// A short list for each dropdown: three projects, two devices, and
    /// three branches, which shows the rule that picks a plain menu or a
    /// search list from the number of options.
    pub fn short<T: 'static>(cx: &mut Context<T>) -> Self {
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
                StatusSelectState::new(options(&["main", "feature/composer", "fix/popover"]), cx)
            }),
        }
    }

    /// Options with long names, to show a tab in a narrow column: a long
    /// project, a long device, and a long branch.
    pub fn long<T: 'static>(cx: &mut Context<T>) -> Self {
        use gpui_kit::AppContext as _;
        let one = |name: &str| vec![StatusOption::new(name.to_string(), name.to_string())];
        Self {
            project: cx.new(|cx| StatusSelectState::new(one("sample-application-monorepo"), cx)),
            device: cx.new(|cx| StatusSelectState::new(one("Local machine in the office"), cx)),
            branch: cx.new(|cx| {
                StatusSelectState::new(one("feature/composer-redesign-with-a-long-name"), cx)
            }),
        }
    }

    /// The tab: the three dropdowns, ids derived from `id`.
    pub fn tab(&self, id: &'static str) -> ComposerStatusTab {
        let child =
            |name: &'static str| ElementId::NamedChild(ElementId::from(id).into(), name.into());
        ComposerStatusTab::new(id)
            .select(
                StatusSelect::new(child("project"), &self.project)
                    .icon(IconName::Folder)
                    .placeholder("Search projects...")
                    .empty("No projects found"),
            )
            .select(StatusSelect::new(child("device"), &self.device).icon(IconName::Laptop))
            .select(
                StatusSelect::new(child("branch"), &self.branch)
                    .icon(IconName::GitBranch)
                    .placeholder("Search refs...")
                    .empty("No refs found"),
            )
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
