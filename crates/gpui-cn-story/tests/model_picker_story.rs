//! The Model picker story page: every example owns its state, opens by its
//! own trigger, and the controlled example resets.

use gpui_cn::{ReduceMotion, Theme};
use gpui_cn_story::{Gallery, agents, stories::ModelPickerStory};
use gpui_kit::{
    AppContext as _, ElementId, Entity, SharedString, TestAppContext, WindowHandle, px, size,
    test::TestWindowExt as _,
};

fn setup(
    cx: &mut TestAppContext,
) -> (WindowHandle<gpui_kit::base::Root>, Entity<ModelPickerStory>) {
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
        gallery.update(cx, |gallery, cx| {
            gallery.select_story("Model picker", window, cx)
        });
    })
    .unwrap();
    frames(&handle, cx);
    let story = gallery
        .read_with(cx, |gallery, cx| {
            gallery.current_story::<ModelPickerStory>(cx)
        })
        .expect("the model picker story is showing");
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

fn named(parent: &str, name: &str) -> ElementId {
    ElementId::NamedChild(
        ElementId::from(SharedString::from(parent.to_string())).into(),
        SharedString::from(name.to_string()),
    )
}

#[gpui_kit::test]
fn every_example_owns_a_state_with_a_model_chosen(cx: &mut TestAppContext) {
    let (_, story) = setup(cx);
    let pickers = story.read_with(cx, |story, _| story.pickers());
    let mut ids: Vec<_> = pickers
        .iter()
        .map(|(_, state, _)| state.entity_id())
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), pickers.len(), "two examples share one state");
    let shared: Vec<String> = agents::catalog()
        .iter()
        .flat_map(|p| p.models_of().iter().map(|m| m.name().to_string()))
        .collect();
    for (name, state, custom) in pickers {
        let chosen = state
            .read_with(cx, |state, _| {
                state.selected_model().map(|(_, m)| m.name().to_string())
            })
            .unwrap_or_else(|| panic!("{name} has no model chosen"));
        if !custom {
            assert!(shared.contains(&chosen), "{name} shows {chosen}");
        }
    }
}

#[gpui_kit::test]
fn each_picker_opens_alone_by_its_own_trigger(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    let all: Vec<(&str, ElementId)> = vec![
        ("models", named("models", "trigger")),
        ("trigger-only", ElementId::from("trigger-only-button")),
        ("favorites", named("favorites", "trigger")),
        ("no-favorites", named("no-favorites", "trigger")),
        ("single", named("single", "trigger")),
        ("no-effort", named("no-effort", "trigger")),
        ("search", named("search", "trigger")),
        ("controlled", named("controlled", "trigger")),
    ];
    let _ = story;
    for (index, (_, trigger)) in all.iter().enumerate() {
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(trigger.clone(), cx);
        })
        .unwrap();
        frames(&handle, cx);
        frames(&handle, cx);
        cx.update_window(handle.into(), |_, window, cx| {
            for (at, (name, _)) in all.iter().enumerate() {
                let shown = window.try_find(named(name, "body")).is_some();
                assert_eq!(shown, at == index, "picker {name} while {index} is open");
            }
            window.press("escape", cx);
        })
        .unwrap();
        frames(&handle, cx);
    }
}

#[gpui_kit::test]
fn the_favorites_examples_start_full_and_empty(cx: &mut TestAppContext) {
    let (_, story) = setup(cx);
    let pickers = story.read_with(cx, |story, _| story.pickers());
    let get = |name: &str| {
        pickers
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, state, _)| state.clone())
            .unwrap()
    };
    assert_eq!(
        get("favorites").read_with(cx, |state, _| state.favorites().len()),
        2
    );
    assert_eq!(
        get("favorites").read_with(cx, |state, _| state.visible().len()),
        2
    );
    assert!(get("no-favorites").read_with(cx, |state, _| state.favorites().is_empty()));
    assert_eq!(
        get("no-favorites").read_with(cx, |state, _| state.visible().len()),
        0
    );
}

#[gpui_kit::test]
fn reset_restores_the_controlled_picker(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    let state = story.read_with(cx, |story, _| story.controlled().clone());
    state.update(cx, |state, cx| {
        state.select("sonnet-5.5", cx);
        state.set_effort(4, cx);
        state.toggle_favorite("opus-5.5", cx);
    });
    frames(&handle, cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("controlled-reset", cx);
    })
    .unwrap();
    frames(&handle, cx);
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.selected().map(|s| s.as_ref()),
            Some(agents::DEFAULT_MODEL)
        );
        assert_eq!(state.effort(), Some(1));
        let favorites: Vec<_> = state.favorites().iter().map(|f| f.to_string()).collect();
        assert_eq!(favorites, agents::FAVORITES);
    });
}

#[gpui_kit::test]
fn the_search_button_opens_a_picker_with_the_query_typed(cx: &mut TestAppContext) {
    let (handle, story) = setup(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("search-open", cx);
    })
    .unwrap();
    frames(&handle, cx);
    let state = story
        .read_with(cx, |story, _| {
            story
                .pickers()
                .into_iter()
                .find(|(name, _, _)| *name == "search")
                .map(|(_, state, _)| state)
        })
        .unwrap();
    assert!(state.read_with(cx, |state, _| state.is_open()));
    assert_eq!(
        state.read_with(cx, |state, cx| state.query(cx).to_string()),
        "opus"
    );
}
