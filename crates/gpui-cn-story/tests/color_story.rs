//! The Color page: its registry reads real, distinct colors that match the
//! values the theme tests assert, and a swatch copies its hex.

use gpui_cn::{ActiveTheme as _, ReduceMotion, Theme, ThemeMode, theme::to_hex};
use gpui_cn_story::{
    Gallery,
    stories::{COLOR_REGISTRY, ColorStory, grade},
};
use gpui_kit::{
    AppContext as _, ElementId, Entity, TestAppContext, WindowHandle, px, size,
    test::TestWindowExt as _,
};
use std::collections::HashSet;

fn setup(cx: &mut TestAppContext) -> (WindowHandle<gpui_kit::base::Root>, Entity<ColorStory>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery = None;
    let handle = cx.open_window(size(px(1300.), px(3000.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    let gallery: Entity<Gallery> = gallery.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        gallery.update(cx, |gallery, cx| gallery.select_story("Color", window, cx));
    })
    .unwrap();
    frames(&handle, cx);
    let story = gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<ColorStory>(cx))
        .expect("the color story is showing");
    (handle, story)
}

fn frames(handle: &WindowHandle<gpui_kit::base::Root>, cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.update_window((*handle).into(), |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn swatch(name: &str) -> ElementId {
    ElementId::NamedChild(ElementId::from("swatch").into(), name.to_string().into())
}

fn hexes(cx: &mut TestAppContext, mode: ThemeMode) -> Vec<(&'static str, String)> {
    cx.update(|cx| {
        Theme::change(mode, cx);
        let theme = cx.theme().clone();
        COLOR_REGISTRY
            .iter()
            .map(|token| (token.name, to_hex((token.get)(&theme))))
            .collect()
    })
}

#[gpui_kit::test]
fn the_registry_has_unique_names_and_real_colors(cx: &mut TestAppContext) {
    let (_, _) = setup(cx);
    let names: HashSet<_> = COLOR_REGISTRY.iter().map(|token| token.name).collect();
    assert_eq!(names.len(), COLOR_REGISTRY.len(), "a name repeats");
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        for (name, hex) in hexes(cx, mode) {
            assert!(
                hex.starts_with('#') && (hex.len() == 7 || hex.len() == 9),
                "{name} is {hex}"
            );
        }
    }
}

#[gpui_kit::test]
fn the_registry_matches_the_values_the_theme_tests_assert(cx: &mut TestAppContext) {
    let (_, _) = setup(cx);
    let expected = [
        ("background", "#181818", "#ffffff"),
        ("sidebar", "#222222", "#fdfdfd"),
        ("sidebar_border", "#343434", "#ebebeb"),
        ("field", "#2c2c2c", "#ffffff"),
        ("field_border", "#3b3b3b", "#e5e5e6"),
        ("popover", "#2d2d2d", "#ffffff"),
        ("popover_accent", "#3d3d3d", "#f6f6f7"),
        ("select_trigger", "#292929", "#ffffff"),
        ("select_trigger_border", "#3a3a3a", "#e5e5e6"),
        ("radio_border", "#3b3b3b", "#e5e5e6"),
    ];
    let dark = hexes(cx, ThemeMode::Dark);
    let light = hexes(cx, ThemeMode::Light);
    let find = |list: &[(&'static str, String)], name: &str| {
        list.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, hex)| hex.clone())
    };
    for (name, d, l) in expected {
        if let Some(hex) = find(&dark, name) {
            assert_eq!(hex, d, "{name} in dark");
            assert_eq!(find(&light, name).as_deref(), Some(l), "{name} in light");
        }
    }
}

#[gpui_kit::test]
fn a_swatch_copies_its_token_code_and_says_so_for_a_moment(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    let wanted = COLOR_REGISTRY
        .iter()
        .find(|t| t.name == "field")
        .unwrap()
        .snippet
        .to_string();
    assert_eq!(wanted, "cx.theme().field");
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(swatch("field"), cx)
    })
    .unwrap();
    frames(&handle, cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(wanted.as_str()));
    assert_eq!(
        story.read_with(cx, |story, _| story.copied()),
        Some("field")
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    frames(&handle, cx);
    assert_eq!(story.read_with(cx, |story, _| story.copied()), None);

    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("x".into()));
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    frames(&handle, cx);
    let again = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        again.as_deref(),
        Some(wanted.as_str()),
        "Enter copies the focused swatch"
    );
}

#[test]
fn every_snippet_reads_the_token_it_is_named_for() {
    for token in COLOR_REGISTRY {
        let code = token
            .snippet
            .strip_prefix("cx.theme().")
            .unwrap_or_else(|| panic!("{} does not start from the theme", token.name));
        let code = code.strip_suffix("()").unwrap_or(code);
        let last = code.rsplit('.').next().unwrap();
        if token.name != "card" {
            assert_eq!(last, token.name, "{}", token.snippet);
        } else {
            assert_eq!(code, "base.colors.surface");
        }
        assert!(
            !token.snippet.contains('#'),
            "{} carries a literal color",
            token.name
        );
    }
}

#[test]
fn ratios_grade_as_wcag_does() {
    assert_eq!(grade(15.2), "AAA");
    assert_eq!(grade(7.0), "AAA");
    assert_eq!(grade(4.5), "AA");
    assert_eq!(grade(3.2), "AA large");
    assert_eq!(grade(1.5), "Low");
}

fn search(
    handle: &WindowHandle<gpui_kit::base::Root>,
    story: &Entity<ColorStory>,
    text: &str,
    cx: &mut TestAppContext,
) -> Vec<&'static str> {
    cx.update_window((*handle).into(), |_, window, cx| {
        let input = story.read(cx).search().clone();
        input.update(cx, |input, cx| {
            input.set_value(text.to_string(), window, cx)
        });
    })
    .unwrap();
    frames(handle, cx);
    story.read_with(cx, |story, cx| {
        story.visible(cx).iter().map(|token| token.name).collect()
    })
}

#[gpui_kit::test]
fn typing_focus_leaves_only_the_focus_colors(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    let shown = search(&handle, &story, "focus", cx);
    assert!(shown.contains(&"focus_ring") && shown.contains(&"field_focus_border"));
    assert!(shown.len() <= 4, "{shown:?}");
    assert!(!shown.contains(&"background"));
}

#[gpui_kit::test]
fn a_group_title_shows_the_group_and_empty_groups_hide(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    let shown = search(&handle, &story, "surfaces", cx);
    assert!(shown.contains(&"background") && shown.contains(&"sidebar"));
    let groups: HashSet<_> = story.read_with(cx, |story, cx| {
        story.visible(cx).iter().map(|token| token.group).collect()
    });
    assert!(groups.contains(&gpui_cn_story::stories::ColorGroup::Surfaces));
    assert!(groups.len() < gpui_cn_story::stories::ColorGroup::ALL.len());
}

#[gpui_kit::test]
fn a_hex_query_finds_the_swatch_in_dark_and_light(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    for (mode, query, expected) in [
        (ThemeMode::Dark, "#2c2c2c", "field"),
        (ThemeMode::Dark, "799c", "field_focus_border"),
        (ThemeMode::Light, "339c", "field_focus_border"),
    ] {
        cx.update(|cx| Theme::change(mode, cx));
        frames(&handle, cx);
        let shown = search(&handle, &story, query, cx);
        assert!(shown.contains(&expected), "{query} in {mode:?}: {shown:?}");
    }
}

#[gpui_kit::test]
fn nonsense_shows_the_empty_state_and_clearing_restores_everything(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    assert!(search(&handle, &story, "qzxqzxqzx", cx).is_empty());
    cx.update_window(handle.into(), |_, window, _| {
        assert!(window.try_find("color-empty").is_some());
    })
    .unwrap();
    let all = search(&handle, &story, "", cx);
    assert_eq!(all.len(), COLOR_REGISTRY.len());
    cx.update_window(handle.into(), |_, window, _| {
        assert!(window.try_find("color-empty").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn enter_in_the_search_focuses_the_first_match(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    search(&handle, &story, "focus_ring", cx);
    cx.update_window(handle.into(), |_, window, cx| {
        let input = story.read(cx).search().clone();
        use gpui_kit::Focusable as _;
        input.focus_handle(cx).focus(window, cx);
        window.press("enter", cx);
    })
    .unwrap();
    frames(&handle, cx);
    cx.update(|cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("x".into())));
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    frames(&handle, cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        copied.as_deref(),
        Some("cx.theme().focus_ring()"),
        "the first match has focus"
    );
}
