//! The gpui-cn gallery shell: a sidebar of stories beside the story shown,
//! under a title bar the application draws itself, with a settings page a
//! navigation stack pushes over it. Built from gpui-cn components only, so
//! the gallery is also the first consumer of the library.

pub mod agents;
pub mod settings;
pub mod stories;

use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, CommandDialog, CommandEntry, CommandEvent, CommandGroup,
    CommandItem, CommandState, Icon, Input, InputEvent, InputState, NavButtons, NavMotion,
    NavStack, NavStackState, ScrollArea, Sidebar, SidebarCollapsible, SidebarGroup, SidebarLayout,
    SidebarMenuButton, SidebarState, SidebarTrigger, TitleBar, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, AsyncApp, Context, ElementId,
    Entity, InteractiveElement as _, IntoElement, ParentElement as _, PlatformInput, Render,
    ScrollDelta, ScrollWheelEvent, SharedString, Styled as _, Window, actions,
    base::{Selectable as _, StyledExt as _, TestSupportExt as _},
    div,
    prelude::FluentBuilder as _,
    px,
};

pub use settings::{SettingsPage, SettingsSection};

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
        /// Opens the palette that lists every story.
        OpenCommandPalette,
        /// Shows or hides the performance HUD (with the `fps` feature).
        TogglePerformanceHud,
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

/// The section of the sidebar a story is listed under. Every story is in
/// exactly one, named where it is registered in [`stories`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorySection {
    /// Buttons, commands, and menus.
    Actions,
    /// The composer and the pickers for AI chat surfaces.
    Ai,
    /// Avatars, badges, progress, and the like.
    FeedbackAndDisplay,
    /// Spacing, typography, theme, and scrolling.
    Foundations,
    /// Fields and controls that take a value.
    Inputs,
    /// Moving between pages and panels.
    Navigation,
    /// Dialogs and popovers.
    Overlays,
}

impl StorySection {
    /// Every section, in the order the sidebar lists them: Foundations
    /// first, then the others alphabetically by title. A section's rank is
    /// its place here, so a new section is placed by adding it to this table.
    pub const ALL: [StorySection; 7] = [
        Self::Foundations,
        Self::Actions,
        Self::Ai,
        Self::FeedbackAndDisplay,
        Self::Inputs,
        Self::Navigation,
        Self::Overlays,
    ];

    /// Where the section is listed: its place in [`ALL`](Self::ALL).
    pub fn rank(self) -> usize {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(Self::ALL.len())
    }

    /// The label of the section's group in the sidebar.
    pub fn title(self) -> &'static str {
        match self {
            Self::Actions => "Actions",
            Self::Ai => "AI",
            Self::FeedbackAndDisplay => "Feedback and display",
            Self::Foundations => "Foundations",
            Self::Inputs => "Inputs",
            Self::Navigation => "Navigation",
            Self::Overlays => "Overlays",
        }
    }
}

/// The id of a story's row in the sidebar. It comes from the title, so a
/// row keeps its id when stories are added or sections change.
pub fn story_row(title: &str) -> ElementId {
    ElementId::NamedChild(
        ElementId::from("story").into(),
        SharedString::from(title.to_string()),
    )
}

/// A registered story.
#[derive(Clone)]
pub struct StoryEntry {
    title: &'static str,
    section: StorySection,
    icon: IconName,
    description: &'static str,
    build: fn(&mut Window, &mut App) -> AnyView,
}

impl StoryEntry {
    fn of<S: Story>(section: StorySection) -> Self {
        Self {
            title: S::title(),
            section,
            icon: S::icon(),
            description: S::description(),
            build: S::view,
        }
    }

    /// The story's title, which names it in the sidebar.
    pub fn title(&self) -> &'static str {
        self.title
    }

    /// The section the story is listed under.
    pub fn section(&self) -> StorySection {
        self.section
    }
}

/// Every story, in display order: by section rank, then by story title,
/// without regard to case. The order is computed here, so a new story
/// lands in the right place and only needs its section.
pub fn stories() -> Vec<StoryEntry> {
    let mut entries = vec![
        StoryEntry::of::<stories::TypographyStory>(StorySection::Foundations),
        StoryEntry::of::<stories::SpacingStory>(StorySection::Foundations),
        StoryEntry::of::<stories::ButtonStory>(StorySection::Actions),
        StoryEntry::of::<stories::SwitchStory>(StorySection::Inputs),
        StoryEntry::of::<stories::SliderStory>(StorySection::Inputs),
        StoryEntry::of::<stories::RadioStory>(StorySection::Inputs),
        StoryEntry::of::<stories::InputStory>(StorySection::Inputs),
        StoryEntry::of::<stories::TextareaStory>(StorySection::Inputs),
        StoryEntry::of::<stories::SelectStory>(StorySection::Inputs),
        StoryEntry::of::<stories::MenuStory>(StorySection::Actions),
        StoryEntry::of::<stories::CommandStory>(StorySection::Actions),
        StoryEntry::of::<stories::PopoverStory>(StorySection::Overlays),
        StoryEntry::of::<stories::DialogStory>(StorySection::Overlays),
        StoryEntry::of::<stories::FilePickerStory>(StorySection::Overlays),
        StoryEntry::of::<stories::FolderPickerStory>(StorySection::Overlays),
        StoryEntry::of::<stories::ComposerStory>(StorySection::Ai),
        StoryEntry::of::<stories::ModelPickerStory>(StorySection::Ai),
        StoryEntry::of::<stories::AvatarStory>(StorySection::FeedbackAndDisplay),
        StoryEntry::of::<stories::BadgeStory>(StorySection::FeedbackAndDisplay),
        StoryEntry::of::<stories::TagStory>(StorySection::FeedbackAndDisplay),
        StoryEntry::of::<stories::SkeletonStory>(StorySection::FeedbackAndDisplay),
        StoryEntry::of::<stories::SpinnerStory>(StorySection::FeedbackAndDisplay),
        StoryEntry::of::<stories::ProgressStory>(StorySection::FeedbackAndDisplay),
        StoryEntry::of::<stories::ThemeModePickerStory>(StorySection::Foundations),
        StoryEntry::of::<stories::SidebarStory>(StorySection::Navigation),
        StoryEntry::of::<stories::NavStackStory>(StorySection::Navigation),
        StoryEntry::of::<stories::ScrollAreaStory>(StorySection::Navigation),
        StoryEntry::of::<stories::TitleBarStory>(StorySection::Navigation),
        StoryEntry::of::<stories::TabsStory>(StorySection::Navigation),
    ];
    entries.sort_by_key(|entry| (entry.section.rank(), entry.title.to_lowercase()));
    entries
}

/// The commands of the story palette: a group per section, in section
/// order, with the stories of each in display order.
fn palette_entries(entries: &[StoryEntry]) -> Vec<CommandEntry> {
    StorySection::ALL
        .iter()
        .map(|section| {
            CommandGroup::new()
                .heading(section.title())
                .items(
                    entries
                        .iter()
                        .filter(|entry| entry.section == *section)
                        .map(|entry| CommandItem::new(entry.title, entry.title).icon(entry.icon)),
                )
                .into()
        })
        .collect()
}

/// The gallery window content: a navigation stack of pages, one per story
/// shown plus settings, so back and forward walk the stories visited the
/// way the reference app walks its chats.
pub struct Gallery {
    stack: Entity<NavStackState>,
    sidebar: Entity<SidebarState>,
    entries: Vec<StoryEntry>,
    /// Each story's view, built the first time it is opened and kept so
    /// its state survives leaving and coming back.
    story_views: Vec<Option<AnyView>>,
    settings: Entity<SettingsPage>,
    /// The text that filters the story list in the sidebar.
    filter: Entity<InputState>,
    /// The palette that lists every story, opened with the keyboard or
    /// the menu.
    palette: Entity<CommandState>,
    palette_open: bool,
    show_hud: bool,
}

impl Gallery {
    /// A gallery showing the first story.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // The shell hides its sidebar completely, as the reference
        // app does. The trigger moves to the content's title bar.
        // Settings can switch to the icon rail, which on macOS is as wide
        // as the room the window controls take, since it sits under them.
        let sidebar = cx.new(|cx| {
            let state = SidebarState::new(cx).with_collapsible(SidebarCollapsible::Offcanvas);
            if cfg!(target_os = "macos") {
                state.with_icon_width(cx.theme().metrics.window_controls_inset)
            } else {
                state
            }
        });
        let stack = cx.new(|_| NavStackState::new());
        let settings = cx.new(|cx| SettingsPage::new(&sidebar, &stack, cx));
        let entries = stories();
        let filter = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search components")
                .clean_on_escape()
        });
        cx.subscribe_in(
            &filter,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => {
                    if let Some(&first) = this.visible_stories(cx).first() {
                        this.open_story(first, NavMotion::Animated, window, cx);
                    }
                }
                _ => {}
            },
        )
        .detach();
        let palette = cx.new(|cx| {
            CommandState::new("Search components...", window, cx)
                .with_entries(palette_entries(&entries))
        });
        cx.observe(&palette, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &palette,
            window,
            |this, _, event: &CommandEvent, window, cx| {
                if let CommandEvent::Confirmed(title) = event {
                    this.palette_open = false;
                    this.select_story(title, window, cx);
                    cx.notify();
                }
            },
        )
        .detach();
        cx.observe(&stack, |_, _, cx| cx.notify()).detach();
        cx.observe(&sidebar, |_, _, cx| cx.notify()).detach();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        // The actions reach the gallery through application-level
        // handlers, so they work wherever focus is and from every host:
        // the menu and the shortcuts on a desktop, a row in the sidebar on
        // a phone.
        let gallery = cx.entity().downgrade();
        App::on_action(cx, {
            let gallery = gallery.clone();
            move |_: &OpenSettings, cx| {
                let _ = gallery.update(cx, |gallery, cx| gallery.open_settings(cx));
            }
        });
        App::on_action(cx, {
            let gallery = gallery.clone();
            move |_: &ToggleSidebar, cx| {
                let _ = gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx));
            }
        });
        App::on_action(cx, {
            let gallery = gallery.clone();
            move |_: &NavigateBack, cx| {
                let _ = gallery.update(cx, |gallery, cx| gallery.go_back(cx));
            }
        });
        App::on_action(cx, {
            let gallery = gallery.clone();
            move |_: &NavigateForward, cx| {
                let _ = gallery.update(cx, |gallery, cx| gallery.go_forward(cx));
            }
        });
        App::on_action(cx, {
            let gallery = gallery.clone();
            move |_: &OpenCommandPalette, cx| {
                let _ = gallery.update(cx, |gallery, cx| gallery.open_palette(cx));
            }
        });
        App::on_action(cx, move |_: &TogglePerformanceHud, cx| {
            let _ = gallery.update(cx, |gallery, cx| {
                gallery.show_hud = !gallery.show_hud;
                cx.notify();
            });
        });
        let mut gallery = Self {
            stack,
            sidebar,
            story_views: vec![None; entries.len()],
            entries,
            settings,
            filter,
            palette,
            palette_open: false,
            show_hud: false,
        };
        gallery.open_story(0, NavMotion::Immediate, window, cx);
        gallery
    }

    /// Pushes a page showing the story at `ix`, unless that story is
    /// already showing. Out-of-range indexes change nothing.
    pub fn open_story(
        &mut self,
        ix: usize,
        motion: NavMotion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if ix >= self.entries.len() || self.current_story_index(cx) == Some(ix) {
            return;
        }
        let view = self.story_views[ix]
            .get_or_insert_with(|| (self.entries[ix].build)(window, cx))
            .clone();
        let entry = self.entries[ix].clone();
        let page = cx.new(|cx| ComponentsPage::new(ix, view, entry, &self.sidebar, cx));
        self.stack.update(cx, |stack, cx| {
            stack.push(page, motion, cx);
        });
        self.close_sheet(cx);
    }

    /// On a phone the sidebar is a sheet over the page, so picking a page
    /// closes it.
    fn close_sheet(&mut self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |sidebar, cx| {
            if sidebar.is_sheet() {
                sidebar.set_open(false, cx);
            }
        });
    }

    /// Opens the palette that lists every story.
    pub fn open_palette(&mut self, cx: &mut Context<Self>) {
        self.palette_open = true;
        cx.notify();
    }

    /// Whether the story palette is open.
    pub fn palette_open(&self) -> bool {
        self.palette_open
    }

    /// The state of the story palette.
    pub fn palette(&self) -> &Entity<CommandState> {
        &self.palette
    }

    /// Pushes the story with `title`. Unknown titles change nothing.
    pub fn select_story(&mut self, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.entries.iter().position(|entry| entry.title == title) {
            self.open_story(ix, NavMotion::Animated, window, cx);
        }
    }

    /// The index of the story on the current page, if the current page is
    /// a story.
    pub fn current_story_index(&self, cx: &App) -> Option<usize> {
        let page = self.stack.read(cx).current()?.clone();
        let page = page.downcast::<ComponentsPage>().ok()?;
        Some(page.read(cx).story)
    }

    /// The story on the current page, if it is a `T`.
    pub fn current_story<T: 'static>(&self, cx: &App) -> Option<Entity<T>> {
        let ix = self.current_story_index(cx)?;
        self.story_views[ix].clone()?.downcast::<T>().ok()
    }

    /// Whether the settings page is the current page.
    fn settings_showing(&self, cx: &App) -> bool {
        self.stack
            .read(cx)
            .current()
            .is_some_and(|page| page.entity_id() == self.settings.entity_id())
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
        self.close_sheet(cx);
    }

    /// The indexes of the stories whose titles contain the sidebar's filter
    /// text, without regard to case, in display order. With no text every
    /// story matches.
    pub fn visible_stories(&self, cx: &App) -> Vec<usize> {
        let query = self.filter.read(cx).value();
        matching(self.entries.iter().map(|entry| entry.title), &query)
    }

    /// The settings page.
    pub fn settings(&self) -> &Entity<SettingsPage> {
        &self.settings
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

/// Drives the gallery without a user, for a profiler: a second of wheel
/// steps down the page, a second back up, a sidebar slide, and the next
/// story, over and over. Never returns.
pub async fn exercise(gallery: Entity<Gallery>, handle: AnyWindowHandle, cx: &mut AsyncApp) {
    let mut story = 0;
    loop {
        for delta in [-12., 12.] {
            for _ in 0..60 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let _ = cx.update_window(handle, |_, window, cx| {
                    let size = window.viewport_size();
                    // On the page, to the right of the sidebar.
                    let position = gpui_kit::point(size.width * 0.7, size.height * 0.6);
                    window.dispatch_event(
                        PlatformInput::ScrollWheel(ScrollWheelEvent {
                            position,
                            delta: ScrollDelta::Pixels(gpui_kit::point(px(0.), px(delta))),
                            ..Default::default()
                        }),
                        cx,
                    );
                });
            }
        }
        for _ in 0..2 {
            cx.update(|cx| gallery.update(cx, |gallery, cx| gallery.toggle_sidebar(cx)));
            cx.background_executor()
                .timer(std::time::Duration::from_millis(600))
                .await;
        }
        story = (story + 1) % stories().len();
        let _ = cx.update_window(handle, |_, window, cx| {
            gallery.update(cx, |gallery, cx| {
                gallery.open_story(story, NavMotion::Animated, window, cx)
            });
        });
        cx.background_executor()
            .timer(std::time::Duration::from_millis(600))
            .await;
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The HUD reads GPUI's own frame trace, so its numbers are what a
        // frame of this window cost, not an estimate from outside.
        let _ = (&window, &cx);
        // One sidebar for every page, so paging keeps its scroll position.
        // Settings shows its own sections in it.
        let sidebar = if self.settings_showing(cx) {
            self.settings
                .update(cx, |settings, cx| settings.render_sidebar(cx))
        } else {
            self.render_sidebar(cx)
        };
        div()
            .relative()
            .size_full()
            .child(
                SidebarLayout::new(&self.sidebar)
                    .sidebar(sidebar)
                    .child(NavStack::new(&self.stack).flex_1().min_h_0()),
            )
            .child(shell_controls(&self.sidebar, &self.stack, window, cx))
            .child({
                let gallery = cx.entity().downgrade();
                CommandDialog::new("gallery-palette", &self.palette)
                    .open(self.palette_open)
                    .empty("No components found.")
                    .on_open_change(move |open, _, cx| {
                        let _ = gallery.update(cx, |gallery, cx| {
                            gallery.palette_open = open;
                            cx.notify();
                        });
                    })
            })
            .when(self.show_hud, |this| {
                #[cfg(feature = "fps")]
                {
                    this.child(gpui_fps::fps_monitor(window, cx))
                }
                #[cfg(not(feature = "fps"))]
                {
                    this
                }
            })
    }
}

/// A page of the gallery: one story, under its title. The gallery draws
/// the sidebar beside the pages, once, so it keeps its scroll position
/// and state while pages change.
pub struct ComponentsPage {
    story: usize,
    page: AnyView,
    entry: StoryEntry,
    sidebar: Entity<SidebarState>,
}

impl ComponentsPage {
    fn new(
        story: usize,
        page: AnyView,
        entry: StoryEntry,
        sidebar: &Entity<SidebarState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(sidebar, |_, _, cx| cx.notify()).detach();
        Self {
            story,
            page,
            entry,
            sidebar: sidebar.clone(),
        }
    }
}

/// The indexes of the titles that contain `query`, trimmed and compared
/// without regard to case. An empty query matches every title.
fn matching<'a>(titles: impl Iterator<Item = &'a str>, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    titles
        .enumerate()
        .filter(|(_, title)| title.to_lowercase().contains(&query))
        .map(|(ix, _)| ix)
        .collect()
}

impl Gallery {
    /// The components sidebar: the story list, with the showing story
    /// selected.
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Sidebar {
        let current = self.current_story_index(cx);
        let heading = cx.theme().text_heading;
        let muted = cx.theme().muted_foreground();
        let open = self.sidebar.read(cx).is_open();
        let visible = self.visible_stories(cx);
        let mut sections: Vec<(StorySection, Vec<usize>)> = Vec::new();
        for &ix in &visible {
            let section = self.entries[ix].section;
            match sections.last_mut() {
                Some((last, rows)) if *last == section => rows.push(ix),
                _ => sections.push((section, vec![ix])),
            }
        }
        Sidebar::new()
            .header(sidebar_title_bar())
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
            .when(open, |this| {
                this.header(
                    div().px_2().child(
                        Input::new(&self.filter)
                            .id("story-filter")
                            .accessibility_label("Search components")
                            .prefix(Icon::from(IconName::Search).size_4())
                            .cleanable(true),
                    ),
                )
            })
            .when(visible.is_empty(), |this| {
                this.child(
                    div()
                        .id("story-empty")
                        .test_support()
                        .px_4()
                        .py_2()
                        .text_sm()
                        .text_color(muted)
                        .child("No components match."),
                )
            })
            .children(
                sections
                    .into_iter()
                    .enumerate()
                    .map(|(n, (section, rows))| {
                        SidebarGroup::new()
                            .label(section.title())
                            .when(n == 0, |group| group.pt_0())
                            .children(rows.into_iter().map(|ix| {
                                let entry = &self.entries[ix];
                                SidebarMenuButton::new(story_row(entry.title))
                                    .icon(entry.icon)
                                    .label(entry.title)
                                    .selected(current == Some(ix))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_story(ix, NavMotion::Animated, window, cx)
                                    }))
                            }))
                    }),
            )
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
        let entry = &self.entry;
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                TitleBar::new()
                    .inset(!open && !icon_only)
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
                ScrollArea::new("page")
                    .flex_1()
                    .min_h_0()
                    .p_6()
                    .child(self.page.clone()),
            )
    }
}

/// The sidebar's title bar in the shell: an empty strip that moves the
/// window. The trigger and the arrows belong to the window, not to the
/// sidebar; see [`shell_controls`].
pub fn sidebar_title_bar() -> AnyElement {
    TitleBar::new().into_any_element()
}

/// The trigger and the arrows: a layer over the top-left corner of the
/// window, after the window controls, whether the sidebar is open, a
/// rail, a sheet, or gone. The sidebar slides under them; they never move.
pub fn shell_controls(
    sidebar: &Entity<SidebarState>,
    stack: &Entity<NavStackState>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let inset = cfg!(target_os = "macos") && !window.is_fullscreen();
    div()
        .id("shell-controls")
        .absolute()
        .top_0()
        .left_0()
        .h(theme.metrics.title_bar)
        .pt(theme.metrics.title_bar_content_offset)
        .flex()
        .items_center()
        .gap(theme.base.spacing.xs)
        .map(|this| {
            if inset {
                this.pl(theme.metrics.window_controls_inset)
            } else {
                this.pl_3()
            }
        })
        .child(SidebarTrigger::new("trigger", sidebar))
        .child(NavButtons::new("nav", stack))
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

/// Scrolls the gallery page until the element `id` is inside the window,
/// for a headless run that drives a control below the fold. The page
/// lays out every child, so an element below the window still has bounds.
#[cfg(feature = "snapshot")]
pub fn reveal(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) {
    use gpui_kit::test::TestWindowExt as _;
    let bounds = window.find(id).bounds();
    let height = window.viewport_size().height;
    let margin = px(80.);
    if bounds.bottom() + margin > height {
        let delta = height - bounds.bottom() - margin;
        window.scroll(
            "page",
            ScrollDelta::Pixels(gpui_kit::point(px(0.), delta)),
            cx,
        );
        window.render_frame(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::matching;

    #[test]
    fn stories_are_sorted_by_section_rank_then_title_without_regard_to_case() {
        let entries = super::stories();
        let keys: Vec<_> = entries
            .iter()
            .map(|e| (e.section().rank(), e.title().to_lowercase()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn foundations_comes_first_and_the_other_sections_follow_alphabetically() {
        use super::StorySection;
        assert_eq!(StorySection::ALL[0], StorySection::Foundations);
        let rest: Vec<_> = StorySection::ALL[1..]
            .iter()
            .map(|s| s.title().to_lowercase())
            .collect();
        let mut sorted = rest.clone();
        sorted.sort();
        assert_eq!(rest, sorted);
        let titles: Vec<_> = StorySection::ALL.iter().map(|s| s.title()).collect();
        assert_eq!(
            titles,
            [
                "Foundations",
                "Actions",
                "AI",
                "Feedback and display",
                "Inputs",
                "Navigation",
                "Overlays"
            ]
        );
    }

    #[test]
    fn foundations_holds_only_spacing_theme_and_typography() {
        let foundations: Vec<_> = super::stories()
            .iter()
            .filter(|e| e.section() == super::StorySection::Foundations)
            .map(|e| e.title())
            .collect();
        assert_eq!(foundations, ["Spacing", "Theme mode picker", "Typography"]);
    }

    #[test]
    fn every_story_is_registered_once_in_one_section_and_no_section_is_empty() {
        let entries = super::stories();
        let mut titles: Vec<_> = entries.iter().map(|e| e.title()).collect();
        titles.sort();
        let count = titles.len();
        titles.dedup();
        assert_eq!(titles.len(), count, "a title is registered twice");
        assert_eq!(count, 29, "a story was dropped or added without this count");
        for section in super::StorySection::ALL {
            assert!(
                entries.iter().any(|e| e.section() == section),
                "{section:?} has no story"
            );
        }
    }

    #[test]
    fn the_filter_matches_titles_by_substring_without_regard_to_case() {
        let titles = ["Switch", "Select", "Scroll area", "Title bar"];
        assert_eq!(matching(titles.into_iter(), ""), [0, 1, 2, 3]);
        assert_eq!(matching(titles.into_iter(), "  "), [0, 1, 2, 3]);
        assert_eq!(matching(titles.into_iter(), "S"), [0, 1, 2]);
        assert_eq!(matching(titles.into_iter(), "sel"), [1]);
        assert_eq!(matching(titles.into_iter(), " AREA "), [2]);
        assert!(matching(titles.into_iter(), "zzz").is_empty());
    }
}
