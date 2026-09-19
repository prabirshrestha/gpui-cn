//! A sidebar beside the main content: resizable by dragging its edge,
//! collapsible with a trigger, animated between the two.
//!
//! The parts follow shadcn's sidebar. [`SidebarState`] owns whether the
//! sidebar is open, how wide it is, and how it collapses. [`SidebarLayout`]
//! places a [`Sidebar`] beside the content and owns the resize handle and
//! the collapse motion. Inside are [`SidebarGroup`]s of
//! [`SidebarMenuButton`] rows, with [`SidebarMenuSub`] for nested rows,
//! [`SidebarSeparator`] between sections, and [`SidebarMenuSkeleton`]
//! while rows load. [`SidebarTrigger`] opens and closes it.
//!
//! ```
//! use gpui_cn::{Sidebar, SidebarGroup, SidebarLayout, SidebarMenuButton, SidebarState, SidebarTrigger};
//! use gpui_kit::{Entity, IntoElement, ParentElement as _, base::Selectable as _, div};
//!
//! fn shell(state: &Entity<SidebarState>) -> impl IntoElement {
//!     SidebarLayout::new(state)
//!         .sidebar(
//!             Sidebar::new()
//!                 .header(SidebarTrigger::new("trigger", state))
//!                 .child(
//!                     SidebarGroup::new()
//!                         .label("Projects")
//!                         .child(SidebarMenuButton::new("inbox").label("Inbox").selected(true)),
//!                 ),
//!         )
//!         .child(div().child("Content"))
//! }
//! ```

use std::{ops::Range, rc::Rc, time::Duration};

use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Element, ElementId, Entity,
    EventEmitter, GlobalElementId, InspectorElementId, InteractiveElement as _, IntoElement,
    LayoutId, MouseButton, ParentElement, Pixels, Render, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    base::{
        Collapsible, Disableable, Placement, Selectable, StyledExt as _, TestSupportExt as _,
        Transition, h_flex, transition, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{ActiveTheme as _, Button, ButtonSize, Icon, Theme};

const PANEL_LEFT: &[u8] = include_bytes!("../icons/panel-left.svg");
const CHEVRON_RIGHT: &[u8] = include_bytes!("../icons/chevron-right.svg");
const CHEVRON_DOWN: &[u8] = include_bytes!("../icons/chevron-down.svg");

/// Which edge of the content the sidebar is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarSide {
    /// The left edge; the usual navigation rail.
    #[default]
    Left,
    /// The right edge; an inspector or a secondary panel.
    Right,
}

/// What closing the sidebar does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarCollapsible {
    /// Slides out completely and gives its width to the content.
    Offcanvas,
    /// Narrows to a rail of icons; rows drop their labels and show them as
    /// tooltips instead. The default.
    #[default]
    Icon,
    /// Cannot close; the trigger does nothing.
    None,
}

/// Emitted by [`SidebarState`] after it changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SidebarEvent {
    /// The sidebar was opened or closed; the payload is the new state.
    Toggled(bool),
    /// The sidebar width changed by a drag or a call, in pixels.
    Resized(Pixels),
}

/// Whether the sidebar is open, how wide it is, and how it collapses.
///
/// Shared by every page that shows the sidebar, so the width and the
/// open state survive navigation. Edit it through its methods so the
/// layout and the trigger observe the change.
pub struct SidebarState {
    open: bool,
    width: Pixels,
    width_range: Range<Pixels>,
    icon_width: Pixels,
    collapsible: SidebarCollapsible,
    resizing: bool,
}

impl EventEmitter<SidebarEvent> for SidebarState {}

impl SidebarState {
    /// An open sidebar at the theme's sidebar width that narrows to a rail
    /// of icons when closed.
    pub fn new(cx: &App) -> Self {
        let metrics = &cx.theme().metrics;
        Self {
            open: true,
            width: metrics.sidebar_width,
            width_range: metrics.sidebar_width_range.clone(),
            icon_width: metrics.icon_sidebar_width,
            collapsible: SidebarCollapsible::Icon,
            resizing: false,
        }
    }

    /// Starts closed.
    pub fn closed(mut self) -> Self {
        self.open = false;
        self
    }

    /// Starts at `width`, clamped to the range.
    pub fn with_width(mut self, width: Pixels) -> Self {
        self.width = clamp(width, &self.width_range);
        self
    }

    /// The widths a drag can reach. The current width is clamped into it.
    pub fn with_width_range(mut self, range: Range<Pixels>) -> Self {
        self.width_range = range;
        self.width = clamp(self.width, &self.width_range);
        self
    }

    /// How the sidebar closes.
    pub fn with_collapsible(mut self, collapsible: SidebarCollapsible) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// The width of the rail when collapsed to icons. The theme's
    /// `icon_sidebar_width` unless set; a window whose rail sits under the
    /// macOS window controls gives it the theme's `window_controls_inset`.
    pub fn with_icon_width(mut self, width: Pixels) -> Self {
        self.icon_width = width;
        self
    }

    /// The width of the rail when collapsed to icons.
    pub fn icon_width(&self) -> Pixels {
        self.icon_width
    }

    /// Whether the sidebar is open. A sidebar that cannot collapse is
    /// always open.
    pub fn is_open(&self) -> bool {
        self.open || self.collapsible == SidebarCollapsible::None
    }

    /// Whether the sidebar is showing only icons: closed, and collapsing
    /// to icons.
    pub fn is_icon_only(&self) -> bool {
        !self.is_open() && self.collapsible == SidebarCollapsible::Icon
    }

    /// The width the sidebar has when open.
    pub fn width(&self) -> Pixels {
        self.width
    }

    /// The widths a drag can reach.
    pub fn width_range(&self) -> &Range<Pixels> {
        &self.width_range
    }

    /// How the sidebar closes.
    pub fn collapsible(&self) -> SidebarCollapsible {
        self.collapsible
    }

    /// Whether a drag on the handle is in progress.
    pub fn is_resizing(&self) -> bool {
        self.resizing
    }

    /// The width the layout gives the sidebar right now, before motion.
    fn target_width(&self) -> Pixels {
        if self.is_open() {
            self.width
        } else if self.collapsible == SidebarCollapsible::Icon {
            self.icon_width
        } else {
            px(0.)
        }
    }

    /// Opens or closes the sidebar.
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }
        self.open = open;
        cx.emit(SidebarEvent::Toggled(open));
        cx.notify();
    }

    /// Closes an open sidebar and opens a closed one.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.set_open(!self.open, cx);
    }

    /// Sets the width, clamped to the range.
    pub fn set_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let width = clamp(width, &self.width_range);
        if self.width == width {
            return;
        }
        self.width = width;
        cx.emit(SidebarEvent::Resized(width));
        cx.notify();
    }

    /// Changes the width of the rail when collapsed to icons.
    pub fn set_icon_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.icon_width != width {
            self.icon_width = width;
            cx.notify();
        }
    }

    /// Changes how the sidebar closes.
    pub fn set_collapsible(&mut self, collapsible: SidebarCollapsible, cx: &mut Context<Self>) {
        if self.collapsible != collapsible {
            self.collapsible = collapsible;
            cx.notify();
        }
    }

    fn set_resizing(&mut self, resizing: bool, cx: &mut Context<Self>) {
        if self.resizing != resizing {
            self.resizing = resizing;
            cx.notify();
        }
    }
}

fn clamp(width: Pixels, range: &Range<Pixels>) -> Pixels {
    width.max(range.start).min(range.end)
}

/// The value carried by a drag of the resize handle.
struct SidebarResize;

/// What GPUI renders under the pointer during a resize: nothing.
struct NoDragPreview;

impl Render for NoDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// A [`Sidebar`] beside the main content.
///
/// The sidebar keeps the width in its [`SidebarState`] and slides to its
/// closed width when the state closes, over the theme's slow transition
/// (or at once under reduced motion). A handle on its inner edge resizes
/// it within the state's range while it is open. The remaining width is
/// the content's.
///
/// The layout fills its parent; give it a sized parent.
#[derive(IntoElement)]
pub struct SidebarLayout {
    state: Entity<SidebarState>,
    side: SidebarSide,
    sidebar: Option<Sidebar>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SidebarLayout {
    /// A layout driven by `state`, with the sidebar on the left.
    pub fn new(state: &Entity<SidebarState>) -> Self {
        Self {
            state: state.clone(),
            side: SidebarSide::Left,
            sidebar: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// The sidebar.
    pub fn sidebar(mut self, sidebar: Sidebar) -> Self {
        self.sidebar = Some(sidebar);
        self
    }

    /// Which edge the sidebar is on.
    pub fn side(mut self, side: SidebarSide) -> Self {
        self.side = side;
        self
    }
}

impl Styled for SidebarLayout {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for SidebarLayout {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SidebarLayout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (open, icon_only, width, icon_width, target, resizing) = {
            let state = self.state.read(cx);
            (
                state.is_open(),
                state.is_icon_only(),
                state.width,
                state.icon_width,
                state.target_width(),
                state.resizing,
            )
        };
        let handle_width = cx.theme().metrics.resize_handle;
        let id = self.state.entity_id().as_u64();
        let side = self.side;
        // The shown width is the animated value; the target is the state's.
        // A drag moves the target every frame, so it must not animate.
        let policy = if resizing {
            Transition::new(Duration::ZERO)
        } else {
            Theme::global(cx).motion.slow_transition()
        };
        let shown = px(transition(
            ElementId::NamedInteger("sidebar-width".into(), id),
            f32::from(target),
            policy,
            window,
            cx,
        ));
        let visible = shown > px(0.);
        // The rows take their closed or open form as soon as the change
        // starts, and the panel clips them while its width catches up: a
        // closing rail shows icons at once with the sidebar surface
        // trailing behind them, and an opening one reveals its labels as
        // it grows. Switching at the end would leave labels clipped for
        // the whole motion and then jump them into place.
        let icon_mode = icon_only;
        let inner_width = if icon_mode { icon_width } else { width };

        let state = self.state.clone();
        let state_for_down = self.state.clone();
        let state_for_up = self.state.clone();
        let state_for_up_out = self.state.clone();
        let panel = div()
            .id("sidebar-panel")
            .test_support()
            .flex_shrink_0()
            .h_full()
            .overflow_hidden()
            .w(shown)
            .when(side == SidebarSide::Right, |this| this.flex().justify_end())
            .when(visible, |this| {
                this.child(
                    div().h_full().w(inner_width).flex_shrink_0().children(
                        self.sidebar
                            .map(|sidebar| sidebar.side(side).icon_mode(icon_mode)),
                    ),
                )
            });
        let handle = open.then(|| {
            div()
                .id("sidebar-resize-handle")
                .test_support()
                .absolute()
                .top_0()
                .bottom_0()
                .map(|this| match side {
                    SidebarSide::Left => this.left(shown - handle_width / 2.),
                    SidebarSide::Right => this.right(shown - handle_width / 2.),
                })
                .w(handle_width)
                .cursor_col_resize()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    state_for_down.update(cx, |state, cx| state.set_resizing(true, cx));
                })
                .on_drag(SidebarResize, |_, _, _, cx| cx.new(|_| NoDragPreview))
        });
        let inset = div()
            .id("sidebar-inset")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .children(self.children);

        div()
            .id(ElementId::NamedInteger("sidebar-layout".into(), id))
            .flex()
            .items_stretch()
            .size_full()
            .relative()
            .overflow_hidden()
            .refine_style(&self.style)
            .on_drag_move::<SidebarResize>(move |event, _, cx| {
                let width = match side {
                    SidebarSide::Left => event.event.position.x - event.bounds.origin.x,
                    SidebarSide::Right => event.bounds.right() - event.event.position.x,
                };
                state.update(cx, |state, cx| state.set_width(width, cx));
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                state_for_up.update(cx, |state, cx| state.set_resizing(false, cx));
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                state_for_up_out.update(cx, |state, cx| state.set_resizing(false, cx));
            })
            .map(|this| match side {
                SidebarSide::Left => this.child(panel).children(handle).child(inset),
                SidebarSide::Right => this.child(inset).children(handle).child(panel),
            })
    }
}

/// The sidebar surface: a header that stays put, content that scrolls, and
/// a footer that stays put.
///
/// It paints the theme's sidebar tokens and puts its children under the
/// sidebar scope, so a [`Button`] inside it hovers and selects with the
/// sidebar's own surfaces instead of the window's, and rows know when the
/// sidebar is a rail of icons. Fill it with [`SidebarGroup`] and
/// [`SidebarMenuButton`].
#[derive(IntoElement)]
pub struct Sidebar {
    header: Vec<AnyElement>,
    children: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    style: StyleRefinement,
    side: SidebarSide,
    icon_mode: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl Sidebar {
    /// An empty sidebar.
    pub fn new() -> Self {
        Self {
            header: Vec::new(),
            children: Vec::new(),
            footer: Vec::new(),
            style: StyleRefinement::default(),
            side: SidebarSide::Left,
            icon_mode: false,
        }
    }

    /// Something above the scrolling content: a title bar, a workspace
    /// switcher, a search field.
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header.push(header.into_any_element());
        self
    }

    /// Something below the scrolling content: an account row, settings.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer.push(footer.into_any_element());
        self
    }

    fn side(mut self, side: SidebarSide) -> Self {
        self.side = side;
        self
    }

    fn icon_mode(mut self, icon_mode: bool) -> Self {
        self.icon_mode = icon_mode;
        self
    }
}

impl Styled for Sidebar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Sidebar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let has_footer = !self.footer.is_empty();
        SidebarScoped::new(
            self.icon_mode,
            v_flex()
                .size_full()
                .bg(theme.sidebar)
                .text_color(theme.foreground())
                .map(|this| match self.side {
                    SidebarSide::Left => this.border_r_1(),
                    SidebarSide::Right => this.border_l_1(),
                })
                .border_color(theme.sidebar_border)
                .refine_style(&self.style)
                .child(v_flex().flex_shrink_0().children(self.header))
                .child(
                    div()
                        .id("sidebar-content")
                        .test_support()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px_2()
                        .pb_2()
                        .children(self.children),
                )
                .when(has_footer, |this| {
                    this.child(v_flex().flex_shrink_0().px_2().pb_2().children(self.footer))
                }),
        )
    }
}

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// A run of rows under an optional label, with an optional action beside
/// the label, and optionally collapsible.
///
/// A collapsible group is stateless: the owner keeps whether it is open
/// and flips it in [`on_toggle`](Self::on_toggle), as shadcn's
/// `Collapsible` does. In a sidebar collapsed to icons the label and the
/// action are hidden and the rows always show.
#[derive(IntoElement)]
pub struct SidebarGroup {
    id: Option<ElementId>,
    label: Option<SharedString>,
    action: Option<AnyElement>,
    open: Option<bool>,
    on_toggle: Option<ToggleHandler>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Default for SidebarGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarGroup {
    /// An unlabeled group.
    pub fn new() -> Self {
        Self {
            id: None,
            label: None,
            action: None,
            open: None,
            on_toggle: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// A stable id for a collapsible group's toggle. Without one the label
    /// is the id.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// The quiet label above the rows.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A small control at the far end of the label row: an icon-only
    /// [`Button`] in the `Xs` or `Sm` size, such as "add".
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    /// Makes the label a toggle that shows the rows when `open`.
    pub fn collapsible(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Called with the new open state when the label is activated.
    pub fn on_toggle(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Styled for SidebarGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for SidebarGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SidebarGroup {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // The label row is a small row, and the group sits a small step
        // below what is above it, so a label reads as a heading for its
        // rows rather than as a row of its own.
        let (muted, row_height, text_size) = {
            let theme = cx.theme();
            (
                theme.sidebar_muted_foreground,
                theme.metrics.row_sm,
                theme.text_control.size,
            )
        };
        let icon_mode = sidebar_scope(cx).is_some_and(|scope| scope.icon_mode);
        let open = self.open.unwrap_or(true);
        let toggle_id = self
            .id
            .or_else(|| self.label.clone().map(ElementId::from))
            .map(|id| ElementId::NamedChild(id.into(), "toggle".into()));
        let on_toggle = self.on_toggle;
        v_flex()
            .pt_2()
            .refine_style(&self.style)
            .when(!icon_mode, |this| {
                this.when_some(self.label, |this, label| {
                    let row = h_flex()
                        .h(row_height)
                        .text_size(text_size)
                        .text_color(muted);
                    let text = div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label.clone());
                    this.child(match (self.open, toggle_id) {
                        (Some(open), Some(id)) => row
                            .child(
                                Button::new(id)
                                    .ghost()
                                    .flex_1()
                                    .min_w_0()
                                    .justify_start()
                                    .h(row_height)
                                    .px_2()
                                    .font_normal()
                                    .text_color(muted)
                                    .toggled(open)
                                    .accessibility_label(label)
                                    .child(text)
                                    .trailing_icon(Icon::from_bytes(if open {
                                        CHEVRON_DOWN
                                    } else {
                                        CHEVRON_RIGHT
                                    }))
                                    .on_click(move |_, window, cx| {
                                        if let Some(on_toggle) = &on_toggle {
                                            on_toggle(&!open, window, cx);
                                        }
                                    }),
                            )
                            .children(self.action.map(|action| div().pl_1().child(action))),
                        _ => row
                            .px_2()
                            .child(text)
                            .children(self.action.map(|action| div().pl_1().child(action))),
                    })
                })
            })
            .child(
                // Rows sit a hair apart so a hovered row beside the
                // selected one reads as two rows, not one taller one.
                Collapsible::new()
                    .open(open || icon_mode)
                    .content(v_flex().gap_0p5().children(self.children)),
            )
    }
}

/// The size tier of a [`SidebarMenuButton`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarMenuSize {
    /// 28px, 12px text.
    Sm,
    /// 30px, 13px text: the reference row.
    #[default]
    Default,
    /// 48px, for a row with an avatar or two lines.
    Lg,
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A row in the sidebar: an icon, a label, a badge, an action that shows
/// on hover, a selected state, and optionally nested rows it folds.
///
/// A [`Button`] in the ghost variant, full width, left aligned, at the row
/// height, with the foreground as its text. Everything the button offers
/// (tooltips, disabled, focus) applies. In a sidebar collapsed to icons
/// the row shows its icon alone and its label as a tooltip.
///
/// A row with a [`submenu`](Self::submenu) that is [`collapsible`](Self::collapsible)
/// carries a chevron and folds the nested rows; the owner keeps the open
/// state and flips it in [`on_toggle`](Self::on_toggle), as shadcn's
/// collapsible menu does.
#[derive(IntoElement)]
pub struct SidebarMenuButton {
    id: ElementId,
    button: Button,
    label: Option<SharedString>,
    badge: Option<SharedString>,
    action: Option<AnyElement>,
    sub: Option<SidebarMenuSub>,
    open: Option<bool>,
    on_toggle: Option<ToggleHandler>,
    on_click: Option<ClickHandler>,
    size: SidebarMenuSize,
    disabled: bool,
    has_tooltip: bool,
    has_icon: bool,
}

impl SidebarMenuButton {
    /// A row with a stable id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            button: Button::new(id.clone()).ghost(),
            id,
            label: None,
            badge: None,
            action: None,
            sub: None,
            open: None,
            on_toggle: None,
            on_click: None,
            size: SidebarMenuSize::Default,
            disabled: false,
            has_tooltip: false,
            has_icon: false,
        }
    }

    /// Rows nested under this one. Always shown unless the row is
    /// [`collapsible`](Self::collapsible).
    pub fn submenu(mut self, sub: SidebarMenuSub) -> Self {
        self.sub = Some(sub);
        self
    }

    /// Makes the row a toggle for its nested rows, shown when `open`, with
    /// a chevron at its far edge.
    pub fn collapsible(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Called with the new open state when a collapsible row is activated,
    /// before [`on_click`](Self::on_click).
    pub fn on_toggle(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// The visible text.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.button = self.button.icon(icon);
        self.has_icon = true;
        self
    }

    /// An icon after the label, pushed to the far edge.
    pub fn trailing_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.button = self.button.trailing_icon(icon);
        self
    }

    /// Quiet text at the far edge: a count, a shortcut.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// A control at the far edge that shows while the row is hovered: an
    /// icon-only [`Button`] in the `Sm` size, such as "more". It covers
    /// the badge while shown.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    /// The size tier.
    pub fn size(mut self, size: SidebarMenuSize) -> Self {
        self.size = size;
        self
    }

    /// A text tooltip.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.button = self.button.tooltip(text);
        self.has_tooltip = true;
        self
    }

    /// A text tooltip that prefers a side.
    pub fn tooltip_at(mut self, placement: Placement, text: impl Into<SharedString>) -> Self {
        self.button = self.button.tooltip_at(placement, text);
        self.has_tooltip = true;
        self
    }

    /// The activation handler for pointer, Enter, and Space.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Selectable for SidebarMenuButton {
    fn selected(mut self, selected: bool) -> Self {
        self.button = self.button.selected(selected);
        self
    }

    fn is_selected(&self) -> bool {
        self.button.is_selected()
    }
}

impl Disableable for SidebarMenuButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.button = self.button.disabled(disabled);
        self
    }
}

impl Styled for SidebarMenuButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.button.style()
    }
}

impl ParentElement for SidebarMenuButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.button.extend(elements);
    }
}

impl RenderOnce for SidebarMenuButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let foreground = theme.foreground();
        let muted = theme.sidebar_muted_foreground;
        let caption = theme.base.typography.xs.size;
        let icon_mode = sidebar_scope(cx).is_some_and(|scope| scope.icon_mode);
        let (height, text) = match self.size {
            SidebarMenuSize::Sm => (theme.metrics.row_sm, caption),
            SidebarMenuSize::Default => (theme.metrics.row, theme.text_control.size),
            SidebarMenuSize::Lg => (theme.metrics.row_lg, theme.text_control.size),
        };
        let row_id = ElementId::NamedChild(self.id.clone().into(), "row".into());
        let group_name: SharedString = format!("sidebar-row-{:?}", self.id).into();
        let open = self.open;
        let on_toggle = self.on_toggle;
        let on_click = self.on_click;
        // The ghost variant rests with muted text; a row is a destination,
        // so it reads in the foreground. A disabled row keeps the ghost's
        // disabled text.
        let mut button = self
            .button
            .w_full()
            .h(height)
            .px_2()
            .font_normal()
            .text_size(text)
            .when(!self.disabled, |this| this.text_color(foreground))
            .when(on_toggle.is_some() || on_click.is_some(), |this| {
                this.on_click(move |event, window, cx| {
                    if let (Some(on_toggle), Some(open)) = (&on_toggle, open) {
                        on_toggle(&!open, window, cx);
                    }
                    if let Some(on_click) = &on_click {
                        on_click(event, window, cx);
                    }
                })
            });
        if icon_mode {
            // An icon alone, centered, named by its label. A row without
            // an icon shows the label's first letter, so it is not blank.
            button = button.justify_center();
            if let Some(label) = &self.label {
                button = button.accessibility_label(label.clone());
                if !self.has_tooltip {
                    button = button.tooltip_at(Placement::Right, label.clone());
                }
                if !self.has_icon {
                    let initial: SharedString = label
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default()
                        .into();
                    button = button.child(div().text_size(text).child(initial));
                }
            }
            return v_flex().child(div().id(row_id).child(button));
        }
        button = button.justify_start();
        if let Some(label) = self.label {
            button = button.label(label);
        }
        if let Some(open) = open {
            button = button.trailing_icon(Icon::from_bytes(if open {
                CHEVRON_DOWN
            } else {
                CHEVRON_RIGHT
            }));
        }
        if let Some(badge) = self.badge {
            button = button.child(
                div()
                    .flex_1()
                    .flex()
                    .justify_end()
                    .pl_2()
                    .text_size(caption)
                    .text_color(muted)
                    .child(badge),
            );
        }
        let row = div()
            .id(row_id)
            .relative()
            .group(group_name.clone())
            .child(button)
            .when_some(self.action, |this, action| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_1()
                        .flex()
                        .items_center()
                        .opacity(0.)
                        .group_hover(group_name, |this| this.opacity(1.))
                        .child(action),
                )
            });
        v_flex()
            .gap_0p5()
            .child(row)
            .when_some(self.sub, |this, sub| {
                this.child(Collapsible::new().open(open.unwrap_or(true)).content(sub))
            })
    }
}

/// Rows nested under a row: indented, with a hairline down their left.
/// Hidden in a sidebar collapsed to icons.
#[derive(IntoElement)]
pub struct SidebarMenuSub {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Default for SidebarMenuSub {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarMenuSub {
    /// An empty run of nested rows.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for SidebarMenuSub {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for SidebarMenuSub {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SidebarMenuSub {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let border = cx.theme().sidebar_border;
        let icon_mode = sidebar_scope(cx).is_some_and(|scope| scope.icon_mode);
        v_flex()
            .ml_4()
            .pl_1p5()
            .border_l_1()
            .border_color(border)
            .refine_style(&self.style)
            .when(!icon_mode, |this| this.children(self.children))
    }
}

/// A hairline between sections of the sidebar.
#[derive(IntoElement)]
pub struct SidebarSeparator {
    style: StyleRefinement,
}

impl Default for SidebarSeparator {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarSeparator {
    /// A separator with the row inset.
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for SidebarSeparator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SidebarSeparator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .mx_2()
            .my_2()
            .h(px(1.))
            .bg(cx.theme().sidebar_border)
            .refine_style(&self.style)
    }
}

/// A placeholder row while rows load: a muted icon and a muted bar of a
/// stable random width.
#[derive(IntoElement)]
pub struct SidebarMenuSkeleton {
    id: ElementId,
    show_icon: bool,
}

impl SidebarMenuSkeleton {
    /// A skeleton row. The id decides the bar's width, so a list of
    /// skeletons looks like a list of different labels.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            show_icon: true,
        }
    }

    /// Whether the row leaves room for an icon. On by default.
    pub fn show_icon(mut self, show_icon: bool) -> Self {
        self.show_icon = show_icon;
        self
    }
}

impl RenderOnce for SidebarMenuSkeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (fill, row_height, bar_height) = {
            let theme = cx.theme();
            (
                theme.sidebar_accent,
                theme.metrics.row,
                theme.base.typography.xs.size,
            )
        };
        let icon_mode = sidebar_scope(cx).is_some_and(|scope| scope.icon_mode);
        // A stable pseudo-random width between 50% and 90%, from the id.
        let seed = format!("{:?}", self.id).bytes().fold(7u32, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(byte as u32)
        });
        let width = 0.5 + (seed % 41) as f32 / 100.;
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_1p5()
            .h(row_height)
            .px_2()
            .when(icon_mode, |this| this.justify_center())
            .when(self.show_icon, |this| {
                this.child(div().size_4().flex_shrink_0().rounded_sm().bg(fill))
            })
            .when(!icon_mode, |this| {
                this.child(
                    div()
                        .h(bar_height)
                        .w(gpui_kit::relative(width))
                        .rounded_sm()
                        .bg(fill),
                )
            })
    }
}

/// The button that opens and closes a sidebar.
///
/// An icon-only ghost [`Button`] with the panel glyph. It reads as "Hide
/// sidebar" while open and "Show sidebar" while closed, and is disabled
/// for a sidebar that cannot collapse.
#[derive(IntoElement)]
pub struct SidebarTrigger {
    id: ElementId,
    state: Entity<SidebarState>,
    tooltip: bool,
    size: ButtonSize,
}

impl SidebarTrigger {
    /// A trigger for `state`.
    pub fn new(id: impl Into<ElementId>, state: &Entity<SidebarState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            tooltip: true,
            size: ButtonSize::Default,
        }
    }

    /// Whether the trigger shows its name as a tooltip. On by default.
    pub fn tooltip(mut self, tooltip: bool) -> Self {
        self.tooltip = tooltip;
        self
    }

    /// The button size tier.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for SidebarTrigger {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (open, fixed) = {
            let state = self.state.read(cx);
            (
                state.is_open(),
                state.collapsible == SidebarCollapsible::None,
            )
        };
        let name = if open { "Hide sidebar" } else { "Show sidebar" };
        let state = self.state;
        Button::new(self.id)
            .ghost()
            .size(self.size)
            .icon(Icon::from_bytes(PANEL_LEFT))
            .accessibility_label(name)
            .when(self.tooltip, |this| this.tooltip(name))
            .disabled(fixed)
            .on_click(move |_, _, cx| state.update(cx, |state, cx| state.toggle(cx)))
    }
}

/// What a render inside a [`Sidebar`] knows about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SidebarScopeInfo {
    /// The sidebar is a rail of icons.
    pub icon_mode: bool,
}

/// The sidebars the render is inside, innermost last. Components read it
/// to pick the sidebar's surfaces, as shadcn remaps its CSS variables
/// inside `Sidebar`.
#[derive(Default)]
struct SidebarScope {
    stack: Vec<SidebarScopeInfo>,
}

impl gpui_kit::Global for SidebarScope {}

/// The innermost sidebar the element being rendered is inside, if any.
pub(crate) fn sidebar_scope(cx: &App) -> Option<SidebarScopeInfo> {
    cx.try_global::<SidebarScope>()?.stack.last().copied()
}

/// Whether the element being rendered is inside a [`Sidebar`].
pub(crate) fn in_sidebar(cx: &App) -> bool {
    sidebar_scope(cx).is_some()
}

fn with_sidebar_scope<R>(info: SidebarScopeInfo, cx: &mut App, f: impl FnOnce(&mut App) -> R) -> R {
    cx.default_global::<SidebarScope>().stack.push(info);
    let result = f(cx);
    cx.default_global::<SidebarScope>().stack.pop();
    result
}

/// An element that renders its child inside the sidebar scope.
///
/// A deferred child (any `RenderOnce`) renders during layout, so wrapping
/// the three phases is enough for every descendant to see the scope.
struct SidebarScoped {
    info: SidebarScopeInfo,
    child: AnyElement,
}

impl SidebarScoped {
    fn new(icon_mode: bool, child: impl IntoElement) -> Self {
        Self {
            info: SidebarScopeInfo { icon_mode },
            child: child.into_any_element(),
        }
    }
}

impl IntoElement for SidebarScoped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SidebarScoped {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id =
            with_sidebar_scope(self.info, cx, |cx| self.child.request_layout(window, cx));
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        with_sidebar_scope(self.info, cx, |cx| {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        with_sidebar_scope(self.info, cx, |cx| self.child.paint(window, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::TestAppContext;

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
    }

    #[gpui_kit::test]
    fn state_clamps_width_and_reports_changes(cx: &mut TestAppContext) {
        init(cx);
        let state = cx.new(|cx| SidebarState::new(cx).with_width_range(px(200.)..px(400.)));
        let events = Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update({
            let events = events.clone();
            let state = state.clone();
            move |cx| {
                cx.subscribe(&state, move |_, event: &SidebarEvent, _| {
                    events.borrow_mut().push(*event);
                })
                .detach();
            }
        });
        state.update(cx, |state, cx| {
            assert_eq!(state.width(), px(300.));
            state.set_width(px(50.), cx);
            assert_eq!(state.width(), px(200.));
            state.set_width(px(900.), cx);
            assert_eq!(state.width(), px(400.));
            state.set_width(px(400.), cx);
            state.toggle(cx);
            assert!(!state.is_open());
            state.set_open(false, cx);
            state.set_open(true, cx);
        });
        assert_eq!(
            &*events.borrow(),
            &[
                SidebarEvent::Resized(px(200.)),
                SidebarEvent::Resized(px(400.)),
                SidebarEvent::Toggled(false),
                SidebarEvent::Toggled(true),
            ]
        );
    }

    #[gpui_kit::test]
    fn collapse_modes_decide_the_closed_width(cx: &mut TestAppContext) {
        init(cx);
        let state = cx.new(|cx| SidebarState::new(cx).closed());
        state.update(cx, |state, cx| {
            assert_eq!(state.target_width(), px(48.), "a rail by default");
            assert!(state.is_icon_only());
            state.set_collapsible(SidebarCollapsible::Offcanvas, cx);
            assert_eq!(state.target_width(), px(0.));
            assert!(!state.is_icon_only());
            state.set_collapsible(SidebarCollapsible::Icon, cx);
            assert_eq!(state.target_width(), px(48.));
            state.set_collapsible(SidebarCollapsible::None, cx);
            assert!(state.is_open(), "a fixed sidebar is always open");
            assert_eq!(state.target_width(), px(300.));
            assert!(!state.is_icon_only());
        });
    }

    #[gpui_kit::test]
    fn the_scope_nests_and_reports_the_innermost(cx: &mut TestAppContext) {
        cx.update(|cx| {
            assert!(!in_sidebar(cx));
            let rail = SidebarScopeInfo { icon_mode: true };
            let full = SidebarScopeInfo { icon_mode: false };
            with_sidebar_scope(full, cx, |cx| {
                assert_eq!(sidebar_scope(cx), Some(full));
                with_sidebar_scope(rail, cx, |cx| assert_eq!(sidebar_scope(cx), Some(rail)));
                assert_eq!(sidebar_scope(cx), Some(full));
            });
            assert!(!in_sidebar(cx));
        });
    }
}
