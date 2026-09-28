use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, ContextMenu, DropdownMenu, Input, InputState, MenuBar,
    MenuBarMenu, MenuBarState, MenuEntry, MenuEvent, MenuItem, MenuState, MenuSubmenu, Textarea,
    TextareaState, Theme, ThemeMode, TitleBar, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Global, InteractiveElement as _,
    IntoElement, KeyBinding, Menu, MenuItem as AppMenuItem, ParentElement as _, Render,
    SharedString, Styled as _, WeakEntity, Window, actions,
    base::{Align, Disableable as _, input, v_flex},
    div,
};

use crate::{Story, ToggleSidebar, frame, note, page, section};

actions!(
    menu_story,
    [
        /// Starts a new document.
        NewFile,
        /// Opens a document.
        OpenFile,
        /// Saves the document.
        SaveFile,
        /// Saves the document under a new name.
        SaveFileAs,
        /// Closes the document.
        CloseFile,
    ]
);

/// The key context of the story, where its shortcuts are bound, so they
/// work from the keyboard inside it and show in its menus.
const CONTEXT: &str = "MenuStory";

/// Marks that the story's key bindings are in, so a second visit does not
/// bind them again.
struct Bindings;

impl Global for Bindings {}

/// Binds the story's shortcuts, once.
fn bind_keys(cx: &mut App) {
    if cx.has_global::<Bindings>() {
        return;
    }
    let keys = if cfg!(target_os = "macos") {
        ["cmd-n", "cmd-o", "cmd-s", "cmd-shift-s", "cmd-w"]
    } else {
        ["ctrl-n", "ctrl-o", "ctrl-s", "ctrl-shift-s", "ctrl-w"]
    };
    cx.bind_keys([
        KeyBinding::new(keys[0], NewFile, Some(CONTEXT)),
        KeyBinding::new(keys[1], OpenFile, Some(CONTEXT)),
        KeyBinding::new(keys[2], SaveFile, Some(CONTEXT)),
        KeyBinding::new(keys[3], SaveFileAs, Some(CONTEXT)),
        KeyBinding::new(keys[4], CloseFile, Some(CONTEXT)),
    ]);
    cx.set_global(Bindings);
}

/// Menus in a small editor: a menu bar over a document, a dropdown, a
/// context menu, and the menus of text fields.
pub struct MenuStory {
    bar: Entity<MenuBarState>,
    /// A plain bar in a title bar, on the menus an app gives the native bar.
    title_bar_menus: Entity<MenuBarState>,
    document: Entity<TextareaState>,
    options: Entity<MenuState>,
    region: Entity<MenuState>,
    name: Entity<InputState>,
    notes: Entity<TextareaState>,
    dated: Entity<InputState>,
    /// A field whose menu the story replaces.
    cased: Entity<InputState>,
    /// The story's own focus, which the dropdown's actions go to.
    focus: FocusHandle,
    word_wrap: bool,
    status: SharedString,
}

impl MenuStory {
    /// The menu bar, whose menus a headless run opens.
    pub const MENU_BAR: &'static str = "menu-bar";
    /// The plain bar in the title bar example.
    pub const TITLE_MENU_BAR: &'static str = "title-menu-bar";
    /// The document a headless run selects in before opening a menu.
    pub const DOCUMENT: &'static str = "document";
    /// The dropdown's trigger.
    pub const OPTIONS: &'static str = "options-trigger";
    /// The area a headless run right-clicks.
    pub const REGION: &'static str = "region";
}

impl Story for MenuStory {
    fn title() -> &'static str {
        "Menu"
    }

    fn icon() -> IconName {
        IconName::Menu
    }

    fn description() -> &'static str {
        "Displays a menu of commands, from a menu bar, a button, or a right click."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        bind_keys(cx);
        cx.new(|cx| {
            let this = cx.weak_entity();
            let mode = Theme::global(cx).mode;
            let bar = cx.new(|cx| MenuBarState::new(menus(true, mode, this.clone()), cx));
            let title_bar_menus = cx
                .new(|cx| MenuBarState::new(app_menus().into_iter().map(|menu| menu.owned()), cx));
            cx.observe(&title_bar_menus, |_, _, cx| cx.notify())
                .detach();
            cx.observe(&bar, |_, _, cx| cx.notify()).detach();
            // The Theme radios follow the theme however it changes.
            cx.observe_global_in::<Theme>(window, |this: &mut Self, window, cx| {
                this.refresh_menus(window, cx)
            })
            .detach();
            let options = cx.new(MenuState::new);
            let region = cx.new(MenuState::new);
            for menu in [&options, &region] {
                cx.observe(menu, |_, _, cx| cx.notify()).detach();
                cx.subscribe(menu, |this: &mut Self, _, event: &MenuEvent, cx| {
                    if let MenuEvent::Activated(key) = event {
                        this.status = format!("Ran {key}").into();
                        cx.notify();
                    }
                })
                .detach();
            }
            Self {
                bar,
                title_bar_menus,
                document: cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(4, 4)
                        .default_value(
                            "Select some text, then use Edit > Copy and Edit > Paste. File > \
                         New, Open, Save, and Close work from their shortcuts too.",
                        )
                }),
                options,
                region,
                name: cx.new(|cx| InputState::new(window, cx).default_value("Ada Lovelace")),
                notes: cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(2, 2)
                        .placeholder("Notes")
                }),
                dated: cx.new(|cx| InputState::new(window, cx).placeholder("Journal entry")),
                cased: cx.new(|cx| InputState::new(window, cx).default_value("Change My Case")),
                focus: cx.focus_handle(),
                word_wrap: true,
                status: "Ready".into(),
            }
        })
        .into()
    }
}

impl MenuStory {
    /// Rebuilds the menus after a toggle or the theme changed, so their
    /// checks and radios show the current state.
    fn refresh_menus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let menus = menus(self.word_wrap, Theme::global(cx).mode, cx.weak_entity());
        self.bar
            .update(cx, |bar, cx| bar.set_menus(menus, window, cx));
    }

    fn toggle_word_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.word_wrap = !self.word_wrap;
        let wrap = self.word_wrap;
        self.document
            .update(cx, |document, cx| document.set_soft_wrap(wrap, window, cx));
        self.status = if wrap {
            "Word wrap on".into()
        } else {
            "Word wrap off".into()
        };
        self.refresh_menus(window, cx);
    }

    fn ran(&mut self, status: &'static str, cx: &mut Context<Self>) {
        self.status = status.into();
        cx.notify();
    }
}

/// The File, Edit, and Selection menus, in GPUI's own menu data: the list
/// an application gives `App::set_menus` for the native menu bar. The
/// in-window bar converts the same list, so the menus are defined once.
fn app_menus() -> Vec<Menu> {
    vec![
        Menu::new("File").items([
            AppMenuItem::action("New", NewFile),
            AppMenuItem::action("Open...", OpenFile),
            AppMenuItem::submenu(Menu::new("Open Recent").items([
                AppMenuItem::action("notes.md", OpenFile),
                AppMenuItem::action("todo.md", OpenFile),
                AppMenuItem::action("README.md", OpenFile),
            ])),
            AppMenuItem::separator(),
            AppMenuItem::action("Save", SaveFile),
            AppMenuItem::action("Save As...", SaveFileAs).disabled(true),
            AppMenuItem::separator(),
            AppMenuItem::action("Close", CloseFile),
        ]),
        Menu::new("Edit").items([
            AppMenuItem::action("Undo", input::Undo),
            AppMenuItem::action("Redo", input::Redo),
            AppMenuItem::separator(),
            AppMenuItem::action("Cut", input::Cut),
            AppMenuItem::action("Copy", input::Copy),
            AppMenuItem::action("Paste", input::Paste),
            AppMenuItem::action("Select All", input::SelectAll),
        ]),
        Menu::new("Selection").items([
            AppMenuItem::action("Select All", input::SelectAll),
            AppMenuItem::action("Select to End", input::SelectToEnd),
        ]),
    ]
}

/// Every menu of the bar: the application's menus, then View and Help,
/// which use rows GPUI's menu data has no words for: radio items, a
/// label, and a link.
fn menus(word_wrap: bool, mode: ThemeMode, story: WeakEntity<MenuStory>) -> Vec<MenuBarMenu> {
    let theme = |key: &'static str, label: &'static str, choice: ThemeMode| {
        MenuItem::new(key, label)
            .radio(mode == choice)
            .on_select(move |_, cx| Theme::change(choice, cx))
    };
    let mut menus: Vec<MenuBarMenu> = app_menus()
        .into_iter()
        .map(|menu| MenuBarMenu::from(menu.owned()))
        .collect();
    menus.push(
        MenuBarMenu::new("View", "View").entries([
            MenuEntry::from(
                MenuItem::new("word-wrap", "Word Wrap")
                    .checked(word_wrap)
                    .on_select(move |window, cx| {
                        story
                            .update(cx, |this, cx| this.toggle_word_wrap(window, cx))
                            .ok();
                    }),
            ),
            MenuEntry::Separator,
            MenuEntry::label("Theme"),
            theme("theme-light", "Light", ThemeMode::Light).into(),
            theme("theme-dark", "Dark", ThemeMode::Dark).into(),
            theme("theme-system", "System", ThemeMode::System).into(),
            MenuEntry::Separator,
            MenuItem::new("toggle-sidebar", "Toggle Sidebar")
                .action(ToggleSidebar)
                .into(),
        ]),
    );
    menus.push(
        MenuBarMenu::new("Help", "Help").entries([
            MenuItem::new("documentation", "Documentation")
                .description("The gpui-cn repository")
                .link("https://github.com/prabirshrestha/gpui-cn"),
            MenuItem::new("about", "About gpui-cn"),
        ]),
    );
    menus
}

fn options_entries() -> Vec<MenuEntry> {
    vec![
        MenuItem::new("new-file", "New File")
            .icon(IconName::File)
            .action(NewFile)
            .into(),
        MenuItem::new("rename", "Rename")
            .icon(IconName::FileText)
            .description("Give the document a new name")
            .into(),
        MenuSubmenu::new("share", "Share")
            .icon(IconName::ExternalLink)
            .entries([
                MenuItem::new("share-email", "Email"),
                MenuItem::new("share-link", "Copy Link"),
            ])
            .into(),
        MenuEntry::Separator,
        MenuItem::new("delete", "Delete")
            .icon(IconName::Delete)
            .destructive()
            .into(),
    ]
}

fn region_entries() -> Vec<MenuEntry> {
    vec![
        MenuItem::new("region-cut", "Cut").into(),
        MenuItem::new("region-copy", "Copy")
            .icon(IconName::Copy)
            .into(),
        MenuItem::new("region-paste", "Paste").disabled(true).into(),
        MenuEntry::Separator,
        MenuEntry::label("Item"),
        MenuItem::new("region-rename", "Rename").into(),
        MenuItem::new("region-reveal", "Reveal in Finder")
            .icon(IconName::FolderOpen)
            .into(),
    ]
}

/// The width a story field takes, so a field does not run the page.
const FIELD_WIDTH: gpui_kit::Pixels = gpui_kit::px(360.);

impl Render for MenuStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (border, muted, radius, control) = (
            theme.border(),
            theme.muted_foreground(),
            theme.radius_lg(),
            theme.text_control,
        );
        let title_bar_height = cx.theme().metrics.title_bar;
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NewFile, _, cx| this.ran("New document", cx)))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.ran("Opened", cx)))
            .on_action(cx.listener(|this, _: &SaveFile, _, cx| this.ran("Saved", cx)))
            .on_action(cx.listener(|this, _: &CloseFile, _, cx| this.ran("Closed", cx)))
            .child(page([
                section(
                    "Menu bar",
                    v_flex()
                        .items_start()
                        .gap_3()
                        .child(note(
                            "File, Edit, and Selection are the menus an app gives the native \
                             bar, converted; Edit acts on the document.",
                            cx,
                        ))
                        .child(MenuBar::new(Self::MENU_BAR, &self.bar))
                        .child(
                            div().w(FIELD_WIDTH).child(
                                Textarea::new(&self.document)
                                    .id(Self::DOCUMENT)
                                    .accessibility_label("Document"),
                            ),
                        )
                        .child(
                            div()
                                .text_size(control.size)
                                .line_height(control.line_height)
                                .text_color(muted)
                                .child(self.status.clone()),
                        ),
                )
                .into_any_element(),
                section(
                    "Menu bar in a title bar",
                    v_flex()
                        .w_full()
                        .gap_3()
                        .child(note(
                            "A plain bar drops its frame where the title bar gives the surface.",
                            cx,
                        ))
                        .child(
                            frame(title_bar_height, cx).child(
                                TitleBar::new()
                                    .inset(false)
                                    .border_b_1()
                                    .border_color(border)
                                    .child(
                                        MenuBar::new(Self::TITLE_MENU_BAR, &self.title_bar_menus)
                                            .plain(),
                                    ),
                            ),
                        ),
                )
                .into_any_element(),
                section(
                    "Dropdown menu",
                    v_flex()
                        .items_start()
                        .gap_3()
                        .child(note(
                            "A button opens icons, a shortcut, a description, a submenu, and a \
                             destructive command.",
                            cx,
                        ))
                        .child(
                            DropdownMenu::new("options", &self.options)
                                .align(Align::Start)
                                .action_context(&self.focus)
                                .trigger(
                                    Button::new(Self::OPTIONS)
                                        .size(ButtonSize::Default)
                                        .icon(IconName::Ellipsis)
                                        .label("Options"),
                                )
                                .items(|_, _| options_entries()),
                        ),
                )
                .into_any_element(),
                section(
                    "Context menu",
                    v_flex()
                        .gap_3()
                        .child(note("A right click opens the menu at the pointer.", cx))
                        .child(
                            ContextMenu::new(Self::REGION, &self.region)
                                .items(|_, _| region_entries())
                                .w(FIELD_WIDTH)
                                .h(gpui_kit::px(120.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(radius)
                                .border_1()
                                .border_dashed()
                                .border_color(border)
                                .text_size(control.size)
                                .text_color(muted)
                                .child("Right click here"),
                        ),
                )
                .into_any_element(),
                section(
                    "Text fields",
                    v_flex()
                        .gap_3()
                        .child(note(
                            "Fields offer Cut, Copy, Paste, and Select All; the third adds Insert \
                             Date, and the last replaces the menu with its own commands.",
                            cx,
                        ))
                        .child(
                            div()
                                .w(FIELD_WIDTH)
                                .child(Input::new(&self.name).id("name")),
                        )
                        .child(
                            div()
                                .w(FIELD_WIDTH)
                                .child(Textarea::new(&self.notes).id("notes")),
                        )
                        .child(div().w(FIELD_WIDTH).child(
                            Input::new(&self.dated).id("dated").context_menu({
                                let dated = self.dated.clone();
                                move |mut entries, capabilities, _, _| {
                                    let dated = dated.clone();
                                    entries.push(MenuEntry::Separator);
                                    entries.push(
                                        MenuItem::new("insert-date", "Insert Date")
                                            .disabled(!capabilities.is_editable())
                                            .on_select(move |window, cx| {
                                                dated.update(cx, |dated, cx| {
                                                    dated.insert("2026-09-23", window, cx)
                                                });
                                            })
                                            .into(),
                                    );
                                    entries
                                }
                            }),
                        ))
                        .child(div().w(FIELD_WIDTH).child(
                            Input::new(&self.cased).id("cased").context_menu({
                                let cased = self.cased.clone();
                                move |_, capabilities, _, _| {
                                    let change =
                                        |key: &'static str, label: &'static str, upper: bool| {
                                            let cased = cased.clone();
                                            MenuItem::new(key, label)
                                                .disabled(!capabilities.is_editable())
                                                .on_select(move |window, cx| {
                                                    cased.update(cx, |cased, cx| {
                                                        let text = cased.value();
                                                        let text = if upper {
                                                            text.to_uppercase()
                                                        } else {
                                                            text.to_lowercase()
                                                        };
                                                        cased.set_value(text, window, cx);
                                                    });
                                                })
                                                .into()
                                        };
                                    vec![
                                        change("uppercase", "Uppercase", true),
                                        change("lowercase", "Lowercase", false),
                                    ]
                                }
                            }),
                        )),
                )
                .into_any_element(),
            ]))
    }
}
