//! The example agent catalog and permission set.

use gpui_cn_story::agents::{catalog, permission_modes};

#[test]
fn the_catalog_has_three_providers_with_the_named_models() {
    let catalog = catalog();
    let names = |ix: usize| -> Vec<String> {
        catalog[ix]
            .models_of()
            .iter()
            .map(|m| m.name().to_string())
            .collect()
    };
    assert_eq!(catalog.len(), 3);
    assert_eq!(
        names(0),
        [
            "GPT-5.6 Mini",
            "GPT-5.6 Terra",
            "GPT-5.6 Sol",
            "GPT-5.5",
            "GPT-5.5 Mini",
            "GPT-5.4"
        ]
    );
    assert_eq!(
        names(1),
        ["Fable 5.1", "Opus 5.5", "Sonnet 5.5", "Haiku 4.5"]
    );
    assert_eq!(names(2), ["Pi Auto", "Pi Fast", "Pi Deep"]);
    let haiku = &catalog[1].models_of()[3];
    assert!(!haiku.supports_effort(), "Haiku has no effort");
    assert!(
        catalog[1].models_of()[..3]
            .iter()
            .all(|m| m.supports_effort())
    );
}

#[test]
fn the_permission_set_is_the_claude_code_style_one() {
    let ids: Vec<String> = permission_modes()
        .iter()
        .map(|m| m.id().to_string())
        .collect();
    assert_eq!(ids, ["default", "accept-edits", "plan", "bypass"]);
}
