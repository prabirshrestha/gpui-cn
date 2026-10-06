//! The search box at the top of the gallery's sidebar filters the story
//! list by title.

use gpui_cn::{ReduceMotion, Theme};
use gpui_cn_story::{Gallery, stories};
use gpui_kit::{
    AppContext as _, ElementId, Entity, TestAppContext, WindowHandle, px, size,
    test::TestWindowExt as _,
};

fn setup(cx: &mut TestAppContext) -> (WindowHandle<gpui_kit::base::Root>, Entity<Gallery>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery = None;
    let handle = cx.open_window(size(px(1100.), px(900.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, gallery.unwrap())
}

fn row(ix: usize) -> ElementId {
    gpui_cn_story::story_row(stories()[ix].title())
}

fn index_of(title: &str) -> usize {
    stories()
        .iter()
        .position(|story| story.title() == title)
        .expect("a story with that title")
}

/// The indexes of the story rows in the sidebar now.
fn shown(handle: &WindowHandle<gpui_kit::base::Root>, cx: &mut TestAppContext) -> Vec<usize> {
    cx.update_window((*handle).into(), |_, window, cx| {
        window.render_frame(cx);
        (0..stories().len())
            .filter(|&ix| window.try_find(row(ix)).is_some())
            .collect()
    })
    .unwrap()
}

fn type_query(handle: &WindowHandle<gpui_kit::base::Root>, cx: &mut TestAppContext, text: &str) {
    cx.update_window((*handle).into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("story-filter", cx);
        window.press("cmd-a", cx);
        window.input(text, cx);
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn every_story_shows_until_a_query_is_typed(cx: &mut TestAppContext) {
    let (handle, _) = setup(cx);
    assert_eq!(shown(&handle, cx), (0..stories().len()).collect::<Vec<_>>());
}

#[gpui_kit::test]
fn typing_keeps_the_rows_whose_titles_contain_the_query(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    type_query(&handle, cx, "SWITCH");
    assert_eq!(shown(&handle, cx), [index_of("Switch")]);
    type_query(&handle, cx, "sel");
    let rows = shown(&handle, cx);
    assert!(rows.contains(&index_of("Select")));
    assert!(!rows.contains(&index_of("Switch")));
    assert_eq!(
        rows,
        gallery.read_with(cx, |gallery, cx| gallery.visible_stories(cx))
    );
}

#[gpui_kit::test]
fn nothing_matching_shows_an_empty_state_and_no_group(cx: &mut TestAppContext) {
    let (handle, _) = setup(cx);
    cx.update_window(handle.into(), |_, window, _| {
        assert!(window.try_find("story-empty").is_none());
    })
    .unwrap();
    type_query(&handle, cx, "zzzz");
    assert!(shown(&handle, cx).is_empty());
    cx.update_window(handle.into(), |_, window, _| {
        assert!(window.try_find("story-empty").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_showing_story_stays_selected_by_its_index_while_the_list_narrows(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    let select = index_of("Select");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(row(select), cx);
        window.render_frame(cx);
    })
    .unwrap();
    type_query(&handle, cx, "e");
    assert!(shown(&handle, cx).contains(&select), "the row keeps its id");
    type_query(&handle, cx, "scroll");
    assert!(!shown(&handle, cx).contains(&select));
    assert_eq!(
        gallery.read_with(cx, |gallery, cx| gallery.current_story_index(cx)),
        Some(select),
        "filtering never changes the page"
    );
    type_query(&handle, cx, "selec");
    assert!(
        shown(&handle, cx).contains(&select),
        "the row is back, same id"
    );
    assert_eq!(
        gallery.read_with(cx, |gallery, cx| gallery.current_story_index(cx)),
        Some(select)
    );
}

#[gpui_kit::test]
fn a_row_of_the_filtered_list_opens_its_story(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    type_query(&handle, cx, "tabs");
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(row(index_of("Tabs")), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        gallery.read_with(cx, |gallery, cx| gallery.current_story_index(cx)),
        Some(index_of("Tabs"))
    );
}

#[gpui_kit::test]
fn enter_opens_the_first_match(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    type_query(&handle, cx, "progress");
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        gallery.read_with(cx, |gallery, cx| gallery.current_story_index(cx)),
        Some(index_of("Progress"))
    );
}

#[gpui_kit::test]
fn escape_clears_the_query_and_brings_every_row_back(cx: &mut TestAppContext) {
    let (handle, _) = setup(cx);
    type_query(&handle, cx, "tag");
    assert!(shown(&handle, cx).len() < stories().len());
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("escape", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(shown(&handle, cx).len(), stories().len());
}

#[gpui_kit::test]
fn the_header_keeps_its_clearance_and_its_edge_shows_only_when_scrolled(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let handle = cx.open_window(size(px(1100.), px(500.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gpui_kit::base::Root::new(view, window, cx)
    });
    let token = px(8.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        let title = window.find("story-title").bounds();
        let search = window.find("story-filter").bounds();
        let content = window.find("sidebar-content").bounds();
        let above = f32::from(search.top() - title.bottom());
        let below = f32::from(content.top() - search.bottom());
        assert!((above - 8.).abs() <= 1., "above the box: {above}");
        assert!((below - 8.).abs() <= 1., "below the box: {below}");
        assert!(content.top() >= search.bottom() + token);
        assert!(
            window.try_find("sidebar-header-edge").is_none(),
            "no edge at rest"
        );
        window.scroll(
            "sidebar-content",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-90.))),
            cx,
        );
        window.render_frame(cx);
        window.render_frame(cx);
        let content = window.find("sidebar-content").bounds();
        let search = window.find("story-filter").bounds();
        assert!(
            content.top() >= search.bottom() + token,
            "the clip edge keeps its clearance"
        );
        assert!(
            window.try_find("sidebar-header-edge").is_some(),
            "an edge once scrolled"
        );
        window.scroll(
            "sidebar-content",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(500.))),
            cx,
        );
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(
            window.try_find("sidebar-header-edge").is_none(),
            "gone again at the top"
        );
    })
    .unwrap();
}
