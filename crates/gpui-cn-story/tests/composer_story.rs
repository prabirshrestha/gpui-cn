//! The Composer story page: every picker and menu opens once, anchored to
//! its own trigger, and every sample owns its state.

use gpui_cn::{ReduceMotion, Theme};
use gpui_cn_story::{Gallery, agents, stories::ComposerStory};
use gpui_kit::{
    AppContext as _, ElementId, Entity, TestAppContext, WindowHandle, px, size,
    test::TestWindowExt as _,
};

fn named(parent: &str, name: &str) -> ElementId {
    ElementId::NamedChild(
        ElementId::from(SharedString::from(parent.to_string())).into(),
        SharedString::from(name.to_string()),
    )
}

use gpui_kit::SharedString;

fn setup(cx: &mut TestAppContext) -> (WindowHandle<gpui_kit::base::Root>, Entity<Gallery>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery = None;
    let handle = cx.open_window(size(px(1300.), px(4200.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    let gallery = gallery.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        gallery.update(cx, |gallery, cx| {
            gallery.select_story("Composer", window, cx)
        });
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    (handle, gallery)
}

fn frames(handle: &WindowHandle<gpui_kit::base::Root>, cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.update_window((*handle).into(), |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
}

/// The standalone and composer pickers, as (trigger id, panel id).
fn pickers() -> Vec<(ElementId, ElementId)> {
    let mut found = Vec::new();
    for composer in [
        "composer-agent",
        "composer-basic",
        "composer-rich",
        "composer-custom",
    ] {
        let models = named(composer, "models");
        found.push((
            ElementId::NamedChild(models.clone().into(), "trigger".into()),
            ElementId::NamedChild(models.into(), "panel".into()),
        ));
    }
    found
}

/// The standalone and composer permission menus, as (trigger id, panel id).
fn menus() -> Vec<(ElementId, ElementId)> {
    let mut found = vec![
        (named("permission", "trigger"), named("permission", "menu")),
        (
            named("custom-permission", "trigger"),
            named("custom-permission", "menu"),
        ),
    ];
    for composer in [
        "composer-agent",
        "composer-basic",
        "composer-rich",
        "composer-custom",
    ] {
        let menu = named(composer, "permission");
        found.push((
            ElementId::NamedChild(menu.clone().into(), "trigger".into()),
            ElementId::NamedChild(menu.into(), "menu".into()),
        ));
    }
    found
}

fn open_one(
    handle: &WindowHandle<gpui_kit::base::Root>,
    cx: &mut TestAppContext,
    all: &[(ElementId, ElementId)],
    index: usize,
) {
    cx.update_window((*handle).into(), |_, window, cx| {
        window.click(all[index].0.clone(), cx);
    })
    .unwrap();
    frames(handle, cx);
    frames(handle, cx);
    cx.update_window((*handle).into(), |_, window, cx| {
        for (at, (trigger, panel)) in all.iter().enumerate() {
            let shown = window.try_find(panel.clone()).is_some();
            assert_eq!(
                shown,
                at == index,
                "panel {at} {panel:?} while {index} is open"
            );
            if at == index {
                let trigger = window.find(trigger.clone()).bounds();
                let panel = window.find(panel.clone()).bounds();
                let near = (panel.left() - trigger.left()).abs() < px(400.)
                    && (panel.top() - trigger.bottom()).abs() < px(700.);
                assert!(near, "the panel sits by its trigger: {trigger:?} {panel:?}");
            }
        }
        window.press("escape", cx);
    })
    .unwrap();
    frames(handle, cx);
    cx.update_window((*handle).into(), |_, window, _| {
        for (_, panel) in all {
            assert!(window.try_find(panel.clone()).is_none(), "closed by Escape");
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn each_model_picker_opens_alone_by_its_trigger(cx: &mut TestAppContext) {
    let (handle, _) = setup(cx);
    let all = pickers();
    for index in 0..all.len() {
        open_one(&handle, cx, &all, index);
    }
}

#[gpui_kit::test]
fn each_permission_menu_opens_alone_by_its_trigger(cx: &mut TestAppContext) {
    let (handle, _) = setup(cx);
    let all = menus();
    for index in 0..all.len() {
        open_one(&handle, cx, &all, index);
    }
}

#[gpui_kit::test]
fn every_sample_has_a_model_chosen_from_its_catalog(cx: &mut TestAppContext) {
    let (_, gallery) = setup(cx);
    let story = gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<ComposerStory>(cx))
        .expect("the composer story is showing");
    let shared: Vec<String> = agents::catalog()
        .iter()
        .flat_map(|p| p.models_of().iter().map(|m| m.name().to_string()))
        .collect();
    let samples = story.read_with(cx, |story, cx| story.composers(cx));
    for (name, picker, custom) in samples {
        let chosen = picker.read_with(cx, |state, _| {
            state
                .selected_model()
                .map(|(_, model)| model.name().to_string())
        });
        let chosen = chosen.unwrap_or_else(|| panic!("{name} has no model chosen"));
        if !custom {
            assert!(shared.contains(&chosen), "{name} shows {chosen}");
        }
    }
    let ids: Vec<_> = story.read_with(cx, |story, cx| {
        story
            .composers(cx)
            .into_iter()
            .map(|(_, picker, _)| picker.entity_id())
            .collect()
    });
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        ids.len(),
        "two samples share one picker state"
    );
}

fn progress_of(
    story: &Entity<ComposerStory>,
    id: &str,
    cx: &mut TestAppContext,
) -> Option<Option<f32>> {
    story.read_with(cx, |story, _| {
        story
            .attachments()
            .iter()
            .find(|a| a.id() == id)
            .map(|a| a.progress_percent())
    })
}

fn tick(cx: &mut TestAppContext, millis: u64) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(millis));
    cx.run_until_parked();
}

fn story_of(gallery: &Entity<Gallery>, cx: &mut TestAppContext) -> Entity<ComposerStory> {
    gallery
        .read_with(cx, |gallery, cx| gallery.current_story::<ComposerStory>(cx))
        .expect("the composer story is showing")
}

#[gpui_kit::test]
fn the_upload_demo_runs_the_queue_in_order_and_ends_idle(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    let story = story_of(&gallery, cx);
    assert!(
        story.read_with(cx, |story, _| story.is_uploading()),
        "it starts by itself"
    );
    let mut last_sheet = 55.;
    let mut landed: Vec<(&str, usize)> = Vec::new();
    for step in 0..200 {
        tick(cx, 100);
        for id in ["sheet", "deck", "report"] {
            if !landed.iter().any(|(done, _)| *done == id)
                && progress_of(&story, id, cx) == Some(None)
            {
                landed.push((id, step));
            }
        }
        if let Some(Some(p)) = progress_of(&story, "sheet", cx) {
            assert!(p >= last_sheet, "progress only rises");
            last_sheet = p;
        }
        let deck = progress_of(&story, "deck", cx).flatten();
        if progress_of(&story, "sheet", cx) != Some(None) {
            assert!(deck.unwrap_or(0.) == 0., "one file uploads at a time");
        }
        if !story.read_with(cx, |story, _| story.is_uploading()) {
            break;
        }
    }
    assert_eq!(
        landed.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        ["sheet", "deck", "report"],
        "the files land in order"
    );
    assert!(
        landed.windows(2).all(|pair| pair[0].1 < pair[1].1),
        "each file lands on a later tick than the one before: {landed:?}"
    );
    assert!(
        !story.read_with(cx, |story, _| story.is_uploading()),
        "the task ended"
    );
    tick(cx, 2000);
    frames(&handle, cx);
    frames(&handle, cx);
    cx.update_window(handle.into(), |_, window, _| {
        let tile = ElementId::NamedChild(ElementId::from("files").into(), "sheet".into());
        assert!(
            window
                .try_find(ElementId::NamedChild(tile.clone().into(), "dismiss".into()))
                .is_some(),
            "a landed file shows its dismiss button"
        );
        assert!(
            window
                .try_find(ElementId::NamedChild(tile.into(), "percent".into()))
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn start_replays_and_reset_stops_and_restores_the_list(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    let story = story_of(&gallery, cx);
    tick(cx, 300);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("reset", cx);
    })
    .unwrap();
    assert!(
        !story.read_with(cx, |story, _| story.is_uploading()),
        "reset stops it"
    );
    assert_eq!(progress_of(&story, "sheet", cx), Some(Some(55.)));
    assert_eq!(progress_of(&story, "deck", cx), Some(Some(0.)));
    assert_eq!(progress_of(&story, "report", cx), Some(Some(0.)));
    assert_eq!(
        progress_of(&story, "photo", cx),
        Some(None),
        "the image has landed"
    );
    tick(cx, 2000);
    assert_eq!(
        progress_of(&story, "sheet", cx),
        Some(Some(55.)),
        "no ticks after a reset"
    );
    frames(&handle, cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("start-upload", cx);
    })
    .unwrap();
    assert!(story.read_with(cx, |story, _| story.is_uploading()));
    tick(cx, 500);
    let p = progress_of(&story, "sheet", cx)
        .flatten()
        .expect("still uploading");
    assert!(p > 55., "it runs again: {p}");
}
