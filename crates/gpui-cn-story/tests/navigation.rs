//! The gallery's navigation: stories and settings are pages on one stack.

use gpui_cn::{NavMotion, ReduceMotion, Theme};
use gpui_cn_story::Gallery;
use gpui_kit::{
    AppContext as _, Entity, TestAppContext, WindowHandle, px, size, test::TestWindowExt as _,
};

fn setup(cx: &mut TestAppContext) -> (WindowHandle<gpui_kit::base::Root>, Entity<Gallery>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery = None;
    let handle = cx.open_window(size(px(1100.), px(760.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    (handle, gallery.unwrap())
}

fn index_of(title: &str) -> usize {
    gpui_cn_story::stories()
        .iter()
        .position(|story| story.title() == title)
        .expect("a story with that title")
}

fn click(
    handle: &WindowHandle<gpui_kit::base::Root>,
    cx: &mut TestAppContext,
    id: gpui_kit::ElementId,
) {
    cx.update_window((*handle).into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window((*handle).into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn state(gallery: &Entity<Gallery>, cx: &mut TestAppContext) -> (Option<usize>, usize, usize) {
    gallery.read_with(cx, |gallery, cx| {
        let stack = gallery.stack().read(cx);
        (
            gallery.current_story_index(cx),
            stack.depth(),
            stack.forward_views().len(),
        )
    })
}

#[gpui_kit::test]
fn selecting_stories_builds_a_history_that_back_and_forward_walk(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    assert_eq!(state(&gallery, cx), (Some(0), 1, 0));

    // Click two stories in the sidebar. A row's push is deferred past its
    // click, so each click gets its own update.
    click(&handle, cx, gpui_cn_story::story_row("Menu"));
    assert_eq!(state(&gallery, cx), (Some(index_of("Menu")), 2, 0));
    click(&handle, cx, gpui_cn_story::story_row("Badge"));
    assert_eq!(state(&gallery, cx), (Some(index_of("Badge")), 3, 0));

    // Selecting the story already showing pushes nothing.
    cx.update_window(handle.into(), |_, window, cx| {
        gallery.update(cx, |gallery, cx| {
            gallery.open_story(index_of("Badge"), NavMotion::Animated, window, cx)
        });
    })
    .unwrap();
    assert_eq!(state(&gallery, cx), (Some(index_of("Badge")), 3, 0));

    // Back walks the stories, forward returns.
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.go_back(cx)));
    assert_eq!(state(&gallery, cx), (Some(index_of("Menu")), 2, 1));
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.go_back(cx)));
    assert_eq!(state(&gallery, cx), (Some(0), 1, 2));
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.go_back(cx)));
    assert_eq!(state(&gallery, cx), (Some(0), 1, 2), "the root stays");
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.go_forward(cx)));
    assert_eq!(state(&gallery, cx), (Some(index_of("Menu")), 2, 1));

    // Settings is a page like the others; a new push drops the forward
    // history.
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.open_settings(cx)));
    assert_eq!(state(&gallery, cx), (None, 3, 0));
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.go_back(cx)));
    assert_eq!(state(&gallery, cx), (Some(index_of("Menu")), 2, 1));
}

#[gpui_kit::test]
fn the_title_bar_arrows_drive_the_history(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    let arrow = |name: &'static str| {
        gpui_kit::ElementId::NamedChild(gpui_kit::ElementId::from("nav").into(), name.into())
    };
    click(&handle, cx, gpui_cn_story::story_row("Command"));
    assert_eq!(state(&gallery, cx), (Some(index_of("Command")), 2, 0));
    click(&handle, cx, arrow("back"));
    assert_eq!(state(&gallery, cx), (Some(0), 1, 1));
    click(&handle, cx, arrow("forward"));
    assert_eq!(state(&gallery, cx), (Some(index_of("Command")), 2, 0));
}

#[gpui_kit::test]
fn the_arrows_stay_in_the_title_bar_while_the_sidebar_is_closed(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    click(&handle, cx, gpui_cn_story::story_row("Command"));
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx)));
    let arrow = |name: &'static str| {
        gpui_kit::ElementId::NamedChild(gpui_kit::ElementId::from("nav").into(), name.into())
    };
    click(&handle, cx, arrow("back"));
    assert_eq!(state(&gallery, cx), (Some(0), 1, 1));
    click(&handle, cx, arrow("forward"));
    assert_eq!(state(&gallery, cx), (Some(index_of("Command")), 2, 0));
}

#[gpui_kit::test]
fn the_settings_row_opens_settings_without_a_host_wiring_the_action(cx: &mut TestAppContext) {
    // The gallery registers its own action handlers, so the row works in
    // the desktop shell, on a phone, and here with no menu or shortcut.
    let (handle, gallery) = setup(cx);
    click(&handle, cx, gpui_kit::ElementId::from("open-settings"));
    assert_eq!(state(&gallery, cx), (None, 2, 0));
    cx.update_window(handle.into(), |_, window, _| {
        assert!(window.try_find("back-to-app").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_shell_sidebar_slides_closed_over_the_slow_transition(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    cx.update(|cx| Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::Off));
    let panel_width = |cx: &mut TestAppContext| {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.find("sidebar-panel").bounds().size.width
        })
        .unwrap()
    };
    assert_eq!(panel_width(cx), px(300.));
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx)));
    let start = panel_width(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    let mid = panel_width(cx);
    assert!(
        mid > px(0.) && mid < start,
        "sliding, not snapping: {start:?} then {mid:?}"
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(400));
    assert_eq!(panel_width(cx), px(0.));
}

#[gpui_kit::test]
fn the_trigger_and_the_arrows_never_move(cx: &mut TestAppContext) {
    let (handle, gallery) = setup(cx);
    cx.update(|cx| Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::Off));
    let places = |cx: &mut TestAppContext| {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.render_frame(cx);
            let back = gpui_kit::ElementId::NamedChild(
                gpui_kit::ElementId::from("nav").into(),
                "back".into(),
            );
            (
                window.find("trigger").bounds().origin,
                window.find(back).bounds().origin,
            )
        })
        .unwrap()
    };
    let at_rest = places(cx);
    // Off the canvas: mid-slide and closed.
    cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx)));
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(60));
    assert_eq!(places(cx), at_rest, "mid-slide");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    assert_eq!(places(cx), at_rest, "closed");
    // A rail.
    gallery.update(cx, |gallery, cx| {
        gallery.sidebar().update(cx, |state, cx| {
            state.set_collapsible(gpui_cn::SidebarCollapsible::Icon, cx)
        });
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    assert_eq!(places(cx), at_rest, "rail");
    // The trigger still works from there.
    cx.update_window(handle.into(), |_, window, cx| window.click("trigger", cx))
        .unwrap();
    assert!(gallery.read_with(cx, |gallery, cx| gallery.sidebar().read(cx).is_open()));
}

#[gpui_kit::test]
fn picking_a_story_keeps_the_sidebar_scrolled_where_it_was(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let mut gallery = None;
    // Short enough that the story list scrolls.
    let handle = cx.open_window(size(px(1100.), px(500.)), |window, cx| {
        let view = cx.new(|cx| Gallery::new(window, cx));
        gallery = Some(view.clone());
        gpui_kit::base::Root::new(view, window, cx)
    });
    let gallery = gallery.unwrap();
    let row = gpui_cn_story::story_row("Avatar");
    let scrolled = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let before = window.find(row.clone()).bounds().top();
            window.scroll(
                "sidebar-content",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-150.))),
                cx,
            );
            window.render_frame(cx);
            let after = window.find(row.clone()).bounds().top();
            assert!(after < before, "the sidebar scrolled");
            after
        })
        .unwrap();
    click(&handle, cx, row.clone());
    assert_eq!(state(&gallery, cx).0, Some(index_of("Avatar")));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(row.clone()).bounds().top(),
            scrolled,
            "the story list stays where it was scrolled"
        );
    })
    .unwrap();
}
