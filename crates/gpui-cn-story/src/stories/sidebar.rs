use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, Sidebar, SidebarCollapsible, SidebarGroup, SidebarLayout,
    SidebarMenuButton, SidebarMenuSize, SidebarMenuSkeleton, SidebarMenuSub, SidebarSeparator,
    SidebarSide, SidebarState, SidebarTrigger, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, ElementId, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Window, base::Disableable as _, base::Selectable as _, div,
    prelude::FluentBuilder as _, px,
};

use crate::{Story, frame, note, page, section, segmented_with};

/// A sidebar layout in a frame with every part: groups with actions and
/// collapsing, rows with badges and hover actions, nested rows, a
/// separator, skeleton rows, and the three collapse modes on either side.
pub struct SidebarStory {
    state: Entity<SidebarState>,
    side: SidebarSide,
    selected: &'static str,
    labels_open: bool,
    projects_open: bool,
    /// Whether the nested rows show their guide line.
    guide: bool,
    /// The recent rows, once they have "loaded"; skeleton rows until then.
    recent: Option<&'static [(&'static str, IconName)]>,
}

/// What the loading group shows once it has loaded, a few seconds in.
const RECENT: [(&str, IconName); 3] = [
    ("Q3 planning", IconName::FileText),
    ("Launch checklist", IconName::Calendar),
    ("Design review", IconName::Bell),
];

impl Story for SidebarStory {
    fn title() -> &'static str {
        "Sidebar"
    }

    fn icon() -> IconName {
        IconName::PanelLeft
    }

    fn description() -> &'static str {
        "A resizable, collapsible navigation rail beside the main content."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx: &mut Context<Self>| {
            let state = cx.new(|cx| {
                SidebarState::new(cx)
                    .with_width(px(240.))
                    .with_width_range(px(180.)..px(360.))
            });
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(4))
                    .await;
                this.update(cx, |this, cx| {
                    this.recent = Some(&RECENT);
                    cx.notify();
                })
                .ok();
            })
            .detach();
            Self {
                state,
                side: SidebarSide::Left,
                selected: "inbox",
                labels_open: true,
                projects_open: true,
                guide: true,
                recent: None,
            }
        })
        .into()
    }
}

const ROWS: [(&str, &str, IconName, Option<&str>); 4] = [
    ("inbox", "Inbox", IconName::Inbox, Some("12")),
    ("starred", "Starred", IconName::Star, None),
    ("projects", "Projects", IconName::Folder, Some("3")),
    ("archive", "Archive", IconName::HardDrive, None),
];

impl Render for SidebarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground();
        let title_bar = cx.theme().metrics.title_bar;
        let (open, icon_only, width, collapsible) = {
            let state = self.state.read(cx);
            (
                state.is_open(),
                state.is_icon_only(),
                state.width(),
                state.collapsible(),
            )
        };
        let side = self.side;
        let guide = self.guide;
        let state = self.state.clone();
        let labels_open = self.labels_open;
        let projects_open = self.projects_open;
        page([
            section(
                "Layout",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(note(
                        "Drag the edge to resize between 180px and 360px. The trigger closes \
                         the sidebar the way the collapse mode says: off the canvas, to a rail \
                         of icons whose labels become tooltips, or not at all. Hover a row for \
                         its action; Projects folds its nested rows (with or without the guide \
                         line down their left) and Labels folds as a \
                         group.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_x_5()
                            .gap_y_2()
                            .child(labeled(
                                "Collapse",
                                segmented_with(
                                    "collapsible",
                                    [
                                        ("Offcanvas", collapsible == SidebarCollapsible::Offcanvas),
                                        ("Icon", collapsible == SidebarCollapsible::Icon),
                                        ("None", collapsible == SidebarCollapsible::None),
                                    ],
                                    move |ix, _, cx| {
                                        let mode = [
                                            SidebarCollapsible::Offcanvas,
                                            SidebarCollapsible::Icon,
                                            SidebarCollapsible::None,
                                        ][ix];
                                        state.update(cx, |state, cx| {
                                            state.set_collapsible(mode, cx)
                                        });
                                    },
                                ),
                            ))
                            .child(labeled(
                                "Side",
                                segmented_with(
                                    "side",
                                    [
                                        ("Left", side == SidebarSide::Left),
                                        ("Right", side == SidebarSide::Right),
                                    ],
                                    {
                                        let story = cx.entity().downgrade();
                                        move |ix, _, cx| {
                                            story
                                                .update(cx, |this, cx| {
                                                    this.side =
                                                        [SidebarSide::Left, SidebarSide::Right][ix];
                                                    cx.notify();
                                                })
                                                .ok();
                                        }
                                    },
                                ),
                            ))
                            .child(labeled(
                                "Guide line",
                                segmented_with(
                                    "guide",
                                    [("On", guide), ("Off", !guide)],
                                    {
                                        let story = cx.entity().downgrade();
                                        move |ix, _, cx| {
                                            story
                                                .update(cx, |this, cx| {
                                                    this.guide = ix == 0;
                                                    cx.notify();
                                                })
                                                .ok();
                                        }
                                    },
                                ),
                            )),
                    )
                    .child(
                        frame(px(460.), cx).relative().child(
                            SidebarLayout::new(&self.state)
                                .side(side)
                                .sidebar(
                                    Sidebar::new()
                                        // An empty strip: the trigger is a
                                        // layer over the frame's corner, so
                                        // it stays put while the sidebar
                                        // slides under it.
                                        .header(div().h(title_bar))
                                        .child(
                                            SidebarGroup::new()
                                                .label("Mail")
                                                .action(
                                                    Button::new("story-compose")
                                                        .ghost()
                                                        .size(ButtonSize::Sm)
                                                        .icon(IconName::Plus)
                                                        .accessibility_label("Compose")
                                                        .tooltip("Compose"),
                                                )
                                                .children(ROWS.into_iter().map(
                                                    |(id, label, icon, badge)| {
                                                        SidebarMenuButton::new(id)
                                                            .icon(icon)
                                                            .label(label)
                                                            .when_some(badge, |this, badge| {
                                                                this.badge(badge)
                                                            })
                                                            .action(
                                                                Button::new(ElementId::NamedChild(
                                                                    ElementId::from(id).into(),
                                                                    "more".into(),
                                                                ))
                                                                    .ghost()
                                                                    .size(ButtonSize::Sm)
                                                                    .icon(IconName::Ellipsis)
                                                                    .accessibility_label("More")
                                                                    .tooltip("More"),
                                                            )
                                                            .selected(self.selected == id)
                                                            .when(id == "projects", |this| {
                                                                this.collapsible(projects_open)
                                                                    .on_toggle(cx.listener(
                                                                        |this, open, _, cx| {
                                                                            this.projects_open =
                                                                                *open;
                                                                            cx.notify();
                                                                        },
                                                                    ))
                                                                    .submenu(
                                                                        SidebarMenuSub::new()
                                                                            .guide(guide)
                                                                            .child(
                                                                                SidebarMenuButton::new(
                                                                                    "sub-today",
                                                                                )
                                                                                .label("Today")
                                                                                .size(
                                                                                    SidebarMenuSize::Sm,
                                                                                ),
                                                                            )
                                                                            .child(
                                                                                SidebarMenuButton::new(
                                                                                    "sub-week",
                                                                                )
                                                                                .label("This week")
                                                                                .size(
                                                                                    SidebarMenuSize::Sm,
                                                                                ),
                                                                            ),
                                                                    )
                                                            })
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    this.selected = id;
                                                                    cx.notify();
                                                                },
                                                            ))
                                                    },
                                                )),
                                        )
                                        .child(
                                            SidebarGroup::new()
                                                .id("labels")
                                                .label("Labels")
                                                .collapsible(labels_open)
                                                .on_toggle(cx.listener(|this, open, _, cx| {
                                                    this.labels_open = *open;
                                                    cx.notify();
                                                }))
                                                .child(
                                                    SidebarMenuButton::new("label-work")
                                                        .icon(IconName::Globe)
                                                        .label("Work"),
                                                )
                                                .child(
                                                    SidebarMenuButton::new("label-long")
                                                        .icon(IconName::FileText)
                                                        .label("A label long enough to be truncated with an ellipsis"),
                                                )
                                                .child(
                                                    SidebarMenuButton::new("label-disabled")
                                                        .icon(IconName::Info)
                                                        .label("Disabled")
                                                        .disabled(true),
                                                ),
                                        )
                                        .child(SidebarSeparator::new())
                                        .child(
                                            SidebarGroup::new()
                                                .label("Recent")
                                                .map(|group| match self.recent {
                                                    Some(recent) => group.children(
                                                        recent.iter().map(|(label, icon)| {
                                                            SidebarMenuButton::new(*label)
                                                                .icon(*icon)
                                                                .label(*label)
                                                        }),
                                                    ),
                                                    None => group.children((0..3).map(|ix| {
                                                        SidebarMenuSkeleton::new(("skeleton", ix as usize))
                                                    })),
                                                }),
                                        )
                                        .footer(
                                            SidebarMenuButton::new("story-settings")
                                                .icon(IconName::Settings)
                                                .label("Settings"),
                                        ),
                                )
                                .child(div().h(title_bar).flex_shrink_0())
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .items_center()
                                        .justify_center()
                                        .gap_1()
                                        .child(div().text_sm().child(format!(
                                            "Selected: {}",
                                            self.selected
                                        )))
                                        .child(div().text_xs().text_color(muted).child(
                                            format!(
                                                "Sidebar {} at {}px",
                                                if icon_only {
                                                    "as icons"
                                                } else if open {
                                                    "open"
                                                } else {
                                                    "closed"
                                                },
                                                f32::from(width)
                                            ),
                                        )),
                                ),
                        )
                        .child(
                            // The trigger stays in the frame's top-left
                            // corner, centered in a rail's width, whatever
                            // the sidebar does.
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .h(title_bar)
                                .w(cx.theme().metrics.icon_sidebar_width)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(SidebarTrigger::new("story-trigger", &self.state)),
                        ),
                    ),
            )
            .into_any_element(),
        ])
    }
}

fn labeled(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(div().text_sm().child(label))
        .child(control)
}
