use gpui_cn::{
    Button, Command, CommandDialog, CommandEntry, CommandEvent, CommandGroup, CommandItem,
    CommandState, Tag, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Global, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement as _, Render, SharedString, Styled as _, Window,
    actions, div, px,
};

use crate::{Story, note, page, section};

actions!(
    command_story,
    [
        /// Opens the profile.
        OpenProfile,
        /// Opens the settings.
        OpenSettings,
        /// Opens the palette dialog.
        OpenPalette,
    ]
);

/// The key context of the story, where its shortcuts are bound, so they
/// show in its palettes.
const CONTEXT: &str = "CommandStory";

/// Marks that the story's key bindings are in, so a second visit does not
/// bind them again.
struct Bindings;

impl Global for Bindings {}

fn bind_keys(cx: &mut App) {
    if cx.has_global::<Bindings>() {
        return;
    }
    let keys = if cfg!(target_os = "macos") {
        ["cmd-p", "cmd-,", "cmd-k"]
    } else {
        ["ctrl-p", "ctrl-,", "ctrl-k"]
    };
    cx.bind_keys([
        KeyBinding::new(keys[0], OpenProfile, Some(CONTEXT)),
        KeyBinding::new(keys[1], OpenSettings, Some(CONTEXT)),
        KeyBinding::new(keys[2], OpenPalette, Some(CONTEXT)),
    ]);
    cx.set_global(Bindings);
}

/// Command palettes: one inline, one in a dialog.
pub struct CommandStory {
    inline: Entity<CommandState>,
    dialog: Entity<CommandState>,
    dialog_open: bool,
    focus: FocusHandle,
    status: SharedString,
}

impl CommandStory {
    /// The palette's commands: shadcn's demo, with keywords, shortcuts, a
    /// disabled row, and a row with its own content.
    fn entries(focus: &FocusHandle) -> Vec<CommandEntry> {
        vec![
            CommandGroup::new()
                .heading("Suggestions")
                .items([
                    CommandItem::new("calendar", "Calendar").icon(IconName::Calendar),
                    CommandItem::new("search", "Search").icon(IconName::Search),
                    CommandItem::new("info", "Information")
                        .icon(IconName::Info)
                        .disabled(true),
                    CommandItem::new("starred", "Starred").render(|_, _, _| {
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_between()
                            .child("Starred")
                            .child(Tag::new("starred-new").label("New").secondary())
                    }),
                ])
                .into(),
            CommandEntry::Separator,
            CommandGroup::new()
                .heading("Settings")
                .items([
                    CommandItem::new("profile", "Profile")
                        .icon(IconName::User)
                        .keywords(["account"])
                        .action(OpenProfile)
                        .action_context(focus),
                    CommandItem::new("inbox", "Inbox").icon(IconName::Inbox),
                    CommandItem::new("settings", "Settings")
                        .icon(IconName::Settings)
                        .keywords(["preferences"])
                        .action(OpenSettings)
                        .action_context(focus),
                ])
                .into(),
        ]
    }

    fn report(&mut self, event: &CommandEvent, cx: &mut Context<Self>) {
        self.status = match event {
            CommandEvent::Confirmed(key) => format!("Chose {key}.").into(),
            CommandEvent::Submitted(query) => format!("Searched for \"{query}\".").into(),
            CommandEvent::QueryChanged(_) => return,
        };
        self.dialog_open = false;
        cx.notify();
    }
}

impl Story for CommandStory {
    fn title() -> &'static str {
        "Command"
    }

    fn icon() -> IconName {
        IconName::Search
    }

    fn description() -> &'static str {
        "A search field over a filtered list of commands, with groups, shortcuts, and \
         keyboard navigation."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        bind_keys(cx);
        cx.new(|cx| {
            let focus = cx.focus_handle();
            let inline = cx.new(|cx| {
                CommandState::new("Type a command or search...", window, cx)
                    .with_entries(Self::entries(&focus))
            });
            let dialog = cx.new(|cx| {
                CommandState::new("Type a command or search...", window, cx)
                    .with_entries(Self::entries(&focus))
            });
            for state in [&inline, &dialog] {
                cx.observe(state, |_, _, cx| cx.notify()).detach();
                cx.subscribe(state, |this: &mut Self, _, event, cx| {
                    this.report(event, cx)
                })
                .detach();
            }
            Self {
                inline,
                dialog,
                dialog_open: false,
                focus,
                status: "Choose a command.".into(),
            }
        })
        .into()
    }
}

impl CommandStory {
    /// The story's focus handle, which holds its key context.
    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Whether the palette dialog is open.
    pub fn dialog_open(&self) -> bool {
        self.dialog_open
    }

    /// The state of the palette in the dialog.
    pub fn dialog_state(&self) -> &Entity<CommandState> {
        &self.dialog
    }
}

impl Render for CommandStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let opener = this.clone();
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &OpenProfile, _, cx| {
                this.status = "Opened the profile.".into();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                this.status = "Opened the settings.".into();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenPalette, _, cx| {
                this.dialog_open = true;
                cx.notify();
            }))
            .child(page([
                section(
                    "Inline",
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(note(
                            "Type to filter by label and keywords (try \"account\"). Up and \
                             Down move, Enter chooses, and Escape clears the query. Enter \
                             with no match submits the query.",
                            cx,
                        ))
                        .child(Command::new("command-inline", &self.inline).w(px(360.)))
                        .child(note(self.status.clone(), cx)),
                )
                .into_any_element(),
                section(
                    "In a dialog",
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(note(
                            "The button, or Cmd+K (Ctrl+K elsewhere), opens the palette in a \
                             dialog near the top of the window, with the search field \
                             focused. Choosing a command, Escape, or a press on the backdrop \
                             closes it.",
                            cx,
                        ))
                        .child(
                            div().flex().child(
                                Button::new("command-dialog-trigger")
                                    .label("Open palette")
                                    .on_click(move |_, _, cx| {
                                        opener
                                            .update(cx, |this, cx| {
                                                this.dialog_open = true;
                                                cx.notify();
                                            })
                                            .ok();
                                    }),
                            ),
                        )
                        .child(
                            CommandDialog::new("command-dialog", &self.dialog)
                                .open(self.dialog_open)
                                .on_open_change(move |open, _, cx| {
                                    this.update(cx, |this, cx| {
                                        this.dialog_open = open;
                                        cx.notify();
                                    })
                                    .ok();
                                }),
                        ),
                )
                .into_any_element(),
            ]))
    }
}
