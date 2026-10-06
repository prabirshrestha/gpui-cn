//! Example data for a coding-agent composer: a model catalog of three
//! providers and a permission set.
//!
//! It is only data. Nothing here is behavior of the library, and nothing
//! is a vendor mark: each provider carries a generic Lucide icon.

use gpui_cn::{
    ComposerState, ModelEntry, ModelPickerState, ModelProvider, PermissionMode, PermissionState,
    gpui_kit::assets::IconName,
};
use gpui_kit::{Context, Entity, Window};

/// The three providers: Codex, Claude, and Pi. The first model of each
/// is its default.
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
        ModelProvider::new("pi", "Pi")
            .icon(IconName::Network)
            .models([
                ModelEntry::new("pi-auto", "Pi Auto").effort(true),
                ModelEntry::new("pi-fast", "Pi Fast").effort(true),
                ModelEntry::new("pi-deep", "Pi Deep").effort(true),
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
            .icon(IconName::ShieldCheck),
    ]
}

/// A composer state with the whole catalog, the first model of the first
/// provider chosen, and the permission modes above.
pub fn composer<T: 'static>(window: &mut Window, cx: &mut Context<T>) -> Entity<ComposerState> {
    use gpui_kit::AppContext as _;
    let models =
        cx.new(|cx| ModelPickerState::new(catalog(), window, cx).with_selected("gpt-5.6-mini"));
    let permission = cx.new(|cx| PermissionState::new(cx).with_modes(permission_modes()));
    cx.new(|cx| {
        ComposerState::new(window, cx)
            .with_models(models, window, cx)
            .with_permission(permission, window, cx)
    })
}
