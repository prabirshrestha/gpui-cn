//! The gpui-cn gallery shell: a sidebar of stories beside the story shown,
//! under a title bar the application draws itself, with a settings page a
//! navigation stack pushes over it. Built from gpui-cn components only, so
//! the gallery is also the first consumer of the library.

pub mod settings;
pub mod stories;

use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, NavButtons, NavMotion, NavStack, NavStackState, Sidebar,
    SidebarGroup, SidebarLayout, SidebarMenuButton, SidebarState, SidebarTrigger, TitleBar,
    gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Context, ElementId, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, actions,
    base::{Selectable as _, StyledExt as _},
    div,
    prelude::FluentBuilder as _,
    px,
};

use settings::SettingsPage;

/// The width a story or settings column grows to before it stops, so
/// prose and cards keep a readable measure in a wide window.
pub const PAGE_WIDTH: gpui_kit::Pixels = px(760.);
/// The window the gallery opens at.
pub const WINDOW_SIZE: gpui_kit::Size<gpui_kit::Pixels> = gpui_kit::size(px(1100.), px(760.));
/// The smallest and largest UI font size the settings page offers.
pub const UI_FONT_SIZE_RANGE: std::ops::Range<gpui_kit::Pixels> = px(10.)..px(28.);

actions!(
    gallery,
    [
        /// Opens the settings page.
        OpenSettings,
        /// Shows or hides the sidebar.
        ToggleSidebar,
        /// Goes back one page.
        NavigateBack,
        /// Goes forward one page.
        NavigateForward,
    ]
);

/// One page of the gallery.
pub trait Story: 'static {
    /// The name in the story list.
    fn title() -> &'static str;
    /// The icon beside the name, and the whole row when the sidebar is a
    /// rail.
    fn icon() -> IconName;
    /// One sentence under the title.
    fn description() -> &'static str;
    /// Builds the page.
    fn view(window: &mut Window, cx: &mut App) -> AnyView;
}

/// A registered story.
#[derive(Clone)]
pub struct StoryEntry {
    title: &'static str,
    icon: IconName,
    description: &'static str,
    build: fn(&mut Window, &mut App) -> AnyView,
}

impl StoryEntry {
    fn of<S: Story>() -> Self {
        Self {
            title: S::title(),
            icon: S::icon(),
            description: S::description(),
            build: S::view,
        }
    }
}

/// Every story, in display order.
pub fn stories() -> Vec<StoryEntry> {
    vec![
        StoryEntry::of::<stories::TypographyStory>(),
        StoryEntry::of::<stories::SpacingStory>(),
        StoryEntry::of::<stories::ButtonStory>(),
        StoryEntry::of::<stories::SidebarStory>(),
        StoryEntry::of::<stories::NavStackStory>(),
        StoryEntry::of::<stories::TitleBarStory>(),
    ]
}

/// The gallery window content: a navigation stack whose root is the
/// components page and whose only other page is settings.
pub struct Gallery {
    stack: Entity<NavStackState>,
    sidebar: Entity<SidebarState>,
    components: Entity<ComponentsPage>,
    settings: Entity<SettingsPage>,
}

impl Gallery {
    /// A gallery showing the first story.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // On macOS the rail sits under the window controls, so it is as
        // wide as the room they take.
        let sidebar = cx.new(|cx| {
            let state = SidebarState::new(cx);
            if cfg!(target_os = "macos") {
                state.with_icon_width(cx.theme().metrics.window_controls_inset)
            } else {
                state
            }
        });
        let stack = cx.new(|_| NavStackState::new());
        let components = cx.new(|cx| ComponentsPage::new(&sidebar, &stack, window, cx));
        let settings = cx.new(|cx| SettingsPage::new(&sidebar, &stack, cx));
        stack.update(cx, |stack, cx| {
            stack.push(components.clone(), NavMotion::Immediate, cx);
        });
        cx.observe(&stack, |_, _, cx| cx.notify()).detach();
        Self {
            stack,
            sidebar,
            components,
            settings,
        }
    }

    /// Shows the story with `title` on the components page. Unknown
    /// titles change nothing.
    pub fn select_story(&mut self, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.components.update(cx, |page, cx| {
            if let Some(ix) = page.entries.iter().position(|entry| entry.title == title) {
                page.select(ix, window, cx);
            }
        });
    }

    /// The navigation stack.
    pub fn stack(&self) -> &Entity<NavStackState> {
        &self.stack
    }

    /// The sidebar state shared by every page.
    pub fn sidebar(&self) -> &Entity<SidebarState> {
        &self.sidebar
    }

    /// Pushes the settings page, unless it is already showing.
    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        let settings: AnyView = self.settings.clone().into();
        self.stack.update(cx, |stack, cx| {
            if stack.current() == Some(&settings) {
                return;
            }
            stack.push(settings, NavMotion::Animated, cx);
        });
    }

    /// Shows or hides the sidebar.
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |sidebar, cx| sidebar.toggle(cx));
    }

    /// Goes back one page.
    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        self.stack.update(cx, |stack, cx| {
            stack.pop(NavMotion::Animated, cx);
        });
    }

    /// Goes forward one page.
    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.stack.update(cx, |stack, cx| {
            stack.forward(NavMotion::Animated, cx);
        });
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        NavStack::new(&self.stack).size_full()
    }
}

/// The root page: the story list in the sidebar, the story beside it.
pub struct ComponentsPage {
    entries: Vec<StoryEntry>,
    selected: usize,
    page: AnyView,
    sidebar: Entity<SidebarState>,
    stack: Entity<NavStackState>,
}

impl ComponentsPage {
    fn new(
        sidebar: &Entity<SidebarState>,
        stack: &Entity<NavStackState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let entries = stories();
        let page = (entries[0].build)(window, cx);
        cx.observe(sidebar, |_, _, cx| cx.notify()).detach();
        cx.observe(stack, |_, _, cx| cx.notify()).detach();
        Self {
            entries,
            selected: 0,
            page,
            sidebar: sidebar.clone(),
            stack: stack.clone(),
        }
    }

    fn select(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix == self.selected || ix >= self.entries.len() {
            return;
        }
        self.selected = ix;
        self.page = (self.entries[ix].build)(window, cx);
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> Sidebar {
        let (heading, title_bar) = {
            let theme = cx.theme();
            (theme.text_heading, theme.metrics.title_bar)
        };
        let open = self.sidebar.read(cx).is_open();
        Sidebar::new()
            .header(sidebar_title_bar(
                open,
                title_bar,
                &self.sidebar,
                &self.stack,
            ))
            .when(open, |this| {
                this.header(
                    div()
                        .flex()
                        .items_center()
                        .h(heading.line_height * 1.6)
                        .px_4()
                        .text_size(heading.size)
                        .font_semibold()
                        .child("gpui-cn"),
                )
            })
            .child(SidebarGroup::new().label("Components").children(
                self.entries.iter().enumerate().map(|(ix, entry)| {
                    SidebarMenuButton::new(ElementId::from(("story", ix)))
                        .icon(entry.icon)
                        .label(entry.title)
                        .selected(ix == self.selected)
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.select(ix, window, cx)),
                        )
                }),
            ))
            .footer(
                SidebarMenuButton::new("open-settings")
                    .icon(IconName::Settings)
                    .label("Settings")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx)),
            )
    }
}

impl Render for ComponentsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, muted) = {
            let theme = cx.theme();
            (theme.border(), theme.muted_foreground())
        };
        let (open, icon_only) = {
            let state = self.sidebar.read(cx);
            (state.is_open(), state.is_icon_only())
        };
        let entry = self.entries[self.selected].clone();
        SidebarLayout::new(&self.sidebar)
            .sidebar(self.render_sidebar(cx))
            .child(
                TitleBar::new()
                    .inset(!open && !icon_only)
                    // The rail's own strip holds the window controls, so
                    // the trigger lives here whenever the sidebar is closed.
                    .when(!open, |this| {
                        this.child(SidebarTrigger::new("inset-trigger", &self.sidebar))
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("repository")
                            .ghost()
                            .icon(IconName::Github)
                            .accessibility_label("Repository")
                            .tooltip("Open the repository")
                            .on_click(|_, _, cx| {
                                cx.open_url("https://github.com/prabirshrestha/gpui-cn")
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .px_6()
                    .pb_4()
                    .gap_1()
                    .border_b_1()
                    .border_color(border)
                    .child(div().text_lg().font_medium().child(entry.title))
                    .child(div().text_sm().text_color(muted).child(entry.description)),
            )
            .child(
                div()
                    .id("page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .child(self.page.clone()),
            )
    }
}

/// The sidebar's title bar: the trigger and the navigation arrows while
/// the sidebar is open, and an empty strip of the same height while it is
/// a rail of icons, where the window controls take the room.
pub fn sidebar_title_bar(
    open: bool,
    title_bar: gpui_kit::Pixels,
    sidebar: &Entity<SidebarState>,
    stack: &Entity<NavStackState>,
) -> AnyElement {
    if open {
        TitleBar::new()
            .child(SidebarTrigger::new("trigger", sidebar))
            .child(NavButtons::new("nav", stack))
            .into_any_element()
    } else {
        div().h(title_bar).flex_shrink_0().into_any_element()
    }
}

/// A row of pill buttons where the selected one carries the soft fill.
pub fn segmented<const N: usize>(
    id: &'static str,
    options: [(&'static str, bool); N],
    on_select: fn(usize, &mut App),
) -> impl IntoElement {
    segmented_with(id, options, move |ix, _, cx| on_select(ix, cx))
}

/// [`segmented`] with a closure that also gets the window.
pub fn segmented_with<const N: usize>(
    id: &'static str,
    options: [(&'static str, bool); N],
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    segmented_from(
        id,
        options
            .into_iter()
            .map(|(label, selected)| (SharedString::from(label), selected))
            .collect(),
        on_select,
    )
}

/// [`segmented_with`] over options decided at runtime.
pub fn segmented_from(
    id: &'static str,
    options: Vec<(SharedString, bool)>,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = std::rc::Rc::new(on_select);
    div()
        .flex()
        .items_center()
        .gap_1()
        .children(
            options
                .into_iter()
                .enumerate()
                .map(move |(ix, (label, selected))| {
                    let on_select = on_select.clone();
                    Button::new(ElementId::from((id, ix)))
                        .ghost()
                        .size(ButtonSize::Sm)
                        .rounded_full()
                        .label(label)
                        .toggled(selected)
                        .selected(selected)
                        .on_click(move |_, window, cx| on_select(ix, window, cx))
                }),
        )
}

/// A titled block inside a story.
pub fn section(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_sm().font_medium().child(title))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(content),
        )
}

/// A story page: sections stacked with room between them.
pub fn page(sections: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    div().flex().flex_col().gap_8().children(sections)
}

/// A bordered box a story shows a layout in, so a component that fills
/// its parent has a parent to fill.
pub fn frame(height: gpui_kit::Pixels, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let theme = cx.theme();
    div()
        .id("frame")
        .w_full()
        .max_w(PAGE_WIDTH)
        .h(height)
        .rounded(theme.radius_lg())
        .border_1()
        .border_color(theme.border())
        .bg(theme.background())
        .overflow_hidden()
}

/// A short explanation under a section title.
pub fn note(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground())
        .max_w(PAGE_WIDTH)
        .child(text.into())
}
