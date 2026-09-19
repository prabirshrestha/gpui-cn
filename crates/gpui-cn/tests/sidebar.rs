//! UI integration tests for the sidebar: real input through a headless window.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui_cn::{
    ReduceMotion, Root, Sidebar, SidebarGroup, SidebarLayout, SidebarMenuButton, SidebarState,
    SidebarTrigger, Theme,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, TestAppContext, Window, WindowHandle, base::Selectable as _,
    base::TestSupportExt as _, div, point, px, size, test::TestWindowExt as _,
};

struct Harness {
    sidebar: Entity<SidebarState>,
    clicks: Rc<Cell<usize>>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.clicks.clone();
        div().size_full().child(
            SidebarLayout::new(&self.sidebar)
                .sidebar(
                    Sidebar::new()
                        .header(
                            div()
                                .h(px(46.))
                                .child(SidebarTrigger::new("trigger", &self.sidebar)),
                        )
                        .child(
                            SidebarGroup::new()
                                .label("Projects")
                                .child(
                                    SidebarMenuButton::new("inbox")
                                        .label("Inbox")
                                        .selected(true)
                                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                                )
                                .child(SidebarMenuButton::new("drafts").label("Drafts")),
                        ),
                )
                .child(div().id("content").test_support().size_full()),
        )
    }
}

struct Setup {
    handle: WindowHandle<Root>,
    sidebar: Entity<SidebarState>,
    clicks: Rc<Cell<usize>>,
}

fn setup(cx: &mut TestAppContext, reduce_motion: ReduceMotion) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = reduce_motion);
    });
    // These tests cover the off-canvas mode; the rail has its own below.
    let sidebar = cx.new(|cx| {
        SidebarState::new(cx)
            .with_width_range(px(200.)..px(400.))
            .with_collapsible(gpui_cn::SidebarCollapsible::Offcanvas)
    });
    let clicks = Rc::new(Cell::new(0));
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let harness = cx.new(|_| Harness {
            sidebar: sidebar.clone(),
            clicks: clicks.clone(),
        });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    Setup {
        handle,
        sidebar,
        clicks,
    }
}

fn panel_width(setup: &Setup, cx: &mut TestAppContext) -> gpui_kit::Pixels {
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.find("sidebar-panel").bounds().size.width
    })
    .unwrap()
}

#[gpui_kit::test]
fn the_sidebar_takes_its_width_and_the_content_takes_the_rest(cx: &mut TestAppContext) {
    let setup = setup(cx, ReduceMotion::On);
    assert_eq!(panel_width(&setup, cx), px(300.));
    cx.update_window(setup.handle.into(), |_, window, _| {
        let content = window.find("content").bounds();
        assert_eq!(content.origin.x, px(300.));
        assert_eq!(content.size.width, px(500.));
        // `selected` is visual in base and sets no accessibility flag; the
        // fill is proven in the button's unit tests.
        assert!(window.find("inbox").visible());
        assert_eq!(window.find("inbox").bounds().size.height, px(30.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_trigger_collapses_the_sidebar_and_hides_its_rows(cx: &mut TestAppContext) {
    let setup = setup(cx, ReduceMotion::On);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert_eq!(window.find("trigger").label(), Some("Hide sidebar"));
        window.click("trigger", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(!setup.sidebar.read_with(cx, |state, _| state.is_open()));
    assert_eq!(panel_width(&setup, cx), px(0.));
    cx.update_window(setup.handle.into(), |_, window, _| {
        assert!(
            window.try_find("inbox").is_none(),
            "a collapsed sidebar renders nothing, so nothing in it can take focus"
        );
        assert!(window.try_find("trigger").is_none());
        assert_eq!(window.find("content").bounds().origin.x, px(0.));
    })
    .unwrap();

    setup
        .sidebar
        .update(cx, |state, cx| state.set_open(true, cx));
    assert_eq!(panel_width(&setup, cx), px(300.));
    cx.update_window(setup.handle.into(), |_, window, _| {
        assert_eq!(window.find("trigger").label(), Some("Hide sidebar"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn collapsing_animates_the_width_unless_motion_is_reduced(cx: &mut TestAppContext) {
    let setup = setup(cx, ReduceMotion::Off);
    setup
        .sidebar
        .update(cx, |state, cx| state.set_open(false, cx));
    let width = panel_width(&setup, cx);
    assert!(
        width > px(0.) && width <= px(300.),
        "starts sliding: {width:?}"
    );
    cx.executor().advance_clock(Duration::from_millis(100));
    let mid = panel_width(&setup, cx);
    assert!(mid < width, "keeps sliding: {mid:?} < {width:?}");
    assert!(mid > px(0.));
    cx.executor().advance_clock(Duration::from_millis(400));
    assert_eq!(panel_width(&setup, cx), px(0.));
}

#[gpui_kit::test]
fn dragging_the_edge_resizes_within_the_range(cx: &mut TestAppContext) {
    let setup = setup(cx, ReduceMotion::On);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        let handle = window.find("sidebar-resize-handle").bounds();
        assert_eq!(handle.origin.x, px(296.));
        assert_eq!(handle.size.width, px(8.));
        window.drag(point(px(300.), px(300.)), point(px(350.), px(300.)), cx);
    })
    .unwrap();
    assert_eq!(
        setup.sidebar.read_with(cx, |state, _| state.width()),
        px(350.)
    );
    assert_eq!(panel_width(&setup, cx), px(350.));

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.drag(point(px(350.), px(300.)), point(px(50.), px(300.)), cx);
    })
    .unwrap();
    assert_eq!(
        setup.sidebar.read_with(cx, |state, _| state.width()),
        px(200.)
    );
    assert_eq!(panel_width(&setup, cx), px(200.));
    assert!(!setup.sidebar.read_with(cx, |state, _| state.is_resizing()));

    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.drag(point(px(200.), px(300.)), point(px(700.), px(300.)), cx);
    })
    .unwrap();
    assert_eq!(
        setup.sidebar.read_with(cx, |state, _| state.width()),
        px(400.)
    );
}

#[gpui_kit::test]
fn a_row_clicks_and_is_reachable_by_keyboard(cx: &mut TestAppContext) {
    let setup = setup(cx, ReduceMotion::On);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click("inbox", cx);
        window.render_frame(cx);
        assert_eq!(setup.clicks.get(), 1);
        window.press("tab", cx);
        window.render_frame(cx);
        assert_eq!(window.find("trigger").focused(), Some(true));
        window.press("tab", cx);
        window.render_frame(cx);
        assert_eq!(window.find("inbox").focused(), Some(true));
    })
    .unwrap();
}

/// A sidebar with every part: a collapsible group with an action, rows
/// with a badge and a hover action, nested rows, a separator, a skeleton.
struct Parts {
    sidebar: Entity<SidebarState>,
    group_open: bool,
    row_open: bool,
    action_clicks: Rc<Cell<usize>>,
    row_action_clicks: Rc<Cell<usize>>,
}

impl Render for Parts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let action_clicks = self.action_clicks.clone();
        let row_action_clicks = self.row_action_clicks.clone();
        div().size_full().child(
            SidebarLayout::new(&self.sidebar)
                .side(gpui_cn::SidebarSide::Right)
                .sidebar(
                    Sidebar::new()
                        .child(
                            SidebarGroup::new()
                                .id("mail")
                                .label("Mail")
                                .collapsible(self.group_open)
                                .on_toggle(cx.listener(|this, open, _, cx| {
                                    this.group_open = *open;
                                    cx.notify();
                                }))
                                .action(
                                    gpui_cn::Button::new("add")
                                        .ghost()
                                        .size(gpui_cn::ButtonSize::Sm)
                                        .label("Add")
                                        .on_click(move |_, _, _| {
                                            action_clicks.set(action_clicks.get() + 1)
                                        }),
                                )
                                .child(
                                    SidebarMenuButton::new("inbox")
                                        .label("Inbox")
                                        .badge("12")
                                        .action(
                                            gpui_cn::Button::new("inbox-more")
                                                .ghost()
                                                .size(gpui_cn::ButtonSize::Sm)
                                                .label("More")
                                                .on_click(move |_, _, _| {
                                                    row_action_clicks
                                                        .set(row_action_clicks.get() + 1)
                                                }),
                                        ),
                                )
                                .child(
                                    SidebarMenuButton::new("projects")
                                        .label("Projects")
                                        .collapsible(self.row_open)
                                        .on_toggle(cx.listener(|this, open, _, cx| {
                                            this.row_open = *open;
                                            cx.notify();
                                        }))
                                        .submenu(gpui_cn::SidebarMenuSub::new().child(
                                            SidebarMenuButton::new("nested").label("Nested"),
                                        )),
                                ),
                        )
                        .child(gpui_cn::SidebarSeparator::new())
                        .child(gpui_cn::SidebarMenuSkeleton::new("skeleton")),
                )
                .child(div().id("content").test_support().size_full()),
        )
    }
}

fn setup_parts(
    cx: &mut TestAppContext,
    collapsible: gpui_cn::SidebarCollapsible,
) -> (WindowHandle<Root>, Entity<SidebarState>, Entity<Parts>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let sidebar = cx.new(|cx| SidebarState::new(cx).with_collapsible(collapsible));
    let mut parts = None;
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let view = cx.new(|_| Parts {
            sidebar: sidebar.clone(),
            group_open: true,
            row_open: true,
            action_clicks: Rc::new(Cell::new(0)),
            row_action_clicks: Rc::new(Cell::new(0)),
        });
        parts = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    (handle, sidebar, parts.unwrap())
}

#[gpui_kit::test]
fn a_right_sidebar_sits_at_the_far_edge_with_its_handle_inside(cx: &mut TestAppContext) {
    let (handle, sidebar, _) = setup_parts(cx, gpui_cn::SidebarCollapsible::Offcanvas);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("sidebar-panel").bounds().origin.x, px(500.));
        assert_eq!(window.find("content").bounds().size.width, px(500.));
        let handle = window.find("sidebar-resize-handle").bounds();
        assert_eq!(handle.origin.x, px(496.));
        window.drag(point(px(500.), px(300.)), point(px(450.), px(300.)), cx);
    })
    .unwrap();
    assert_eq!(sidebar.read_with(cx, |state, _| state.width()), px(350.));
}

#[gpui_kit::test]
fn a_collapsible_group_hides_its_rows_and_keeps_its_action(cx: &mut TestAppContext) {
    let (handle, _, parts) = setup_parts(cx, gpui_cn::SidebarCollapsible::Offcanvas);
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("inbox").is_some());
        assert!(window.try_find("nested").is_some());
        window.click(
            gpui_kit::ElementId::NamedChild(
                gpui_kit::ElementId::from("mail").into(),
                "toggle".into(),
            ),
            cx,
        );
        window.render_frame(cx);
        assert!(window.try_find("inbox").is_none(), "rows fold away");
        assert!(window.try_find("nested").is_none());
        window.click("add", cx);
        window.render_frame(cx);
    })
    .unwrap();
    parts.read_with(cx, |parts, _| {
        assert!(!parts.group_open);
        assert_eq!(parts.action_clicks.get(), 1);
    });
}

#[gpui_kit::test]
fn a_row_action_is_reachable_and_fires_on_its_own(cx: &mut TestAppContext) {
    let (handle, _, parts) = setup_parts(cx, gpui_cn::SidebarCollapsible::Offcanvas);
    cx.update_window(handle.into(), |_, window, cx| {
        window.hover("inbox", cx);
        window.render_frame(cx);
        window.click("inbox-more", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        parts.read_with(cx, |parts, _| parts.row_action_clicks.get()),
        1
    );
}

#[gpui_kit::test]
fn an_icon_rail_shows_icons_alone_named_by_their_labels(cx: &mut TestAppContext) {
    let (handle, sidebar, _) = setup_parts(cx, gpui_cn::SidebarCollapsible::Icon);
    sidebar.update(cx, |state, cx| state.set_open(false, cx));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("sidebar-panel").bounds().size.width, px(48.));
        let inbox = window.find("inbox");
        assert_eq!(inbox.label(), Some("Inbox"), "the label is the name");
        assert!(inbox.bounds().size.width < px(40.), "an icon-sized row");
        assert!(
            window.try_find("nested").is_none(),
            "nested rows are hidden"
        );
        assert!(
            window.try_find("sidebar-resize-handle").is_none(),
            "a rail is not resizable"
        );
    })
    .unwrap();
    assert!(sidebar.read_with(cx, |state, _| state.is_icon_only()));
}

#[gpui_kit::test]
fn a_collapsible_row_folds_its_nested_rows(cx: &mut TestAppContext) {
    let (handle, _, parts) = setup_parts(cx, gpui_cn::SidebarCollapsible::Offcanvas);
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("nested").is_some());
        window.click("projects", cx);
        window.render_frame(cx);
        assert!(window.try_find("nested").is_none(), "the nested rows fold");
        window.click("projects", cx);
        window.render_frame(cx);
        assert!(window.try_find("nested").is_some());
    })
    .unwrap();
    assert!(parts.read_with(cx, |parts, _| parts.row_open));
}

#[gpui_kit::test]
fn an_icon_rail_shows_an_initial_for_a_row_without_an_icon(cx: &mut TestAppContext) {
    let (handle, sidebar, _) = setup_parts(cx, gpui_cn::SidebarCollapsible::Icon);
    sidebar.update(cx, |state, cx| {
        state.set_icon_width(px(88.), cx);
        state.set_open(false, cx);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("sidebar-panel").bounds().size.width, px(88.));
        // "Inbox" has no icon in this harness, so its rail form is an "I".
        let inbox = window.find("inbox");
        assert_eq!(inbox.label(), Some("Inbox"));
        assert!(inbox.bounds().size.width > px(0.));
    })
    .unwrap();
    assert_eq!(
        sidebar.read_with(cx, |state, _| state.icon_width()),
        px(88.)
    );
}

#[gpui_kit::test]
fn rows_take_their_rail_form_as_soon_as_the_rail_starts_closing(cx: &mut TestAppContext) {
    let (handle, sidebar, _) = setup_parts(cx, gpui_cn::SidebarCollapsible::Icon);
    cx.update(|cx| Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::Off));
    sidebar.update(cx, |state, cx| state.set_open(false, cx));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let panel = window.find("sidebar-panel").bounds().size.width;
        assert!(panel > px(48.), "still sliding: {panel:?}");
        let inbox = window.find("inbox").bounds();
        assert!(inbox.size.width < px(40.), "already an icon: {inbox:?}");
        assert!(
            window.try_find("nested").is_none(),
            "labels and nested rows gone at once"
        );
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("sidebar-panel").bounds().size.width, px(48.));
    })
    .unwrap();
}
