use crate::remote::RemoteSim;
use gpui_cn::{
    FileEntry, FileFilter, FilePicker, FilePickerEvent, FilePickerState, MemoryFiles, Switch,
    gpui_kit::assets::IconName,
};
use std::time::Duration;

use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div,
};

use crate::{Story, note, page, picker_trigger, section};

/// The home folder the fixture lists in the snapshot build.
const FIXTURE_HOME: &str = "/home/prabirshrestha";

/// A dialog to choose files on any file system, one file or several.
pub struct FilePickerStory {
    one_open: bool,
    many_open: bool,
    one: Entity<FilePickerState>,
    many: Entity<FilePickerState>,
    allow_new_folder: bool,
    remote: bool,
    sim: RemoteSim,
    chosen: SharedString,
    chosen_many: Vec<String>,
}

impl FilePickerStory {
    /// The button that opens the picker of one file.
    pub const TRIGGER_ONE: &'static str = "file-picker-trigger-one";
    /// The button that opens the picker of several files.
    pub const TRIGGER_MANY: &'static str = "file-picker-trigger-many";

    /// What the page says was chosen in the picker of one file.
    pub fn chosen(&self) -> &str {
        &self.chosen
    }

    /// The paths the picker of several files chose.
    pub fn chosen_many(&self) -> &[String] {
        &self.chosen_many
    }

    /// Whether the picker of one file is open.
    pub fn one_open(&self) -> bool {
        self.one_open
    }

    /// The state of the picker of one file.
    pub fn one(&self) -> &Entity<FilePickerState> {
        &self.one
    }

    /// The state of the picker of several files.
    pub fn many(&self) -> &Entity<FilePickerState> {
        &self.many
    }

    /// The simulated build server the pickers list while the remote
    /// switch is on.
    pub fn sim(&self) -> &RemoteSim {
        &self.sim
    }

    /// Lists from the simulated build server, or from the local file
    /// system (the fixture, in a snapshot build) again.
    pub fn set_remote(&mut self, remote: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.remote = remote;
        for state in [self.one.clone(), self.many.clone()] {
            let sim = self.sim.clone();
            state.update(cx, |state, cx| {
                if remote {
                    state.set_source(sim, window, cx);
                } else if cfg!(feature = "snapshot") {
                    state.set_source(fixture(), window, cx);
                } else {
                    state.set_source(gpui_cn::LocalFiles, window, cx);
                }
            });
        }
        cx.notify();
    }
}

/// A fixed set of entries, so the snapshot does not depend on the disk it
/// runs on.
fn fixture() -> MemoryFiles {
    MemoryFiles::new()
        .with_home(FIXTURE_HOME)
        .with_dir(
            FIXTURE_HOME,
            [
                FileEntry::folder("code"),
                FileEntry::folder("Documents"),
                FileEntry::folder(".config"),
                FileEntry::file("Cargo.toml").with_size(612),
                FileEntry::file("README.md").with_size(4_300),
                FileEntry::file("main.rs").with_size(2_048),
                FileEntry::file("lib.rs").with_size(18_400),
                FileEntry::file("notes.txt").with_size(930),
                FileEntry::file("photo.png").with_size(2_400_000),
                FileEntry::file("report.pdf").with_size(480_000),
                FileEntry::file(".zshrc").with_size(1_100),
            ],
        )
        .with_dir(
            format!("{FIXTURE_HOME}/code"),
            [
                FileEntry::folder("gpui-cn"),
                FileEntry::folder("website"),
                FileEntry::file("todo.md").with_size(210),
            ],
        )
}

/// The file types of both pickers: every file first, so files always
/// show, then Rust files.
fn filters() -> Vec<FileFilter> {
    vec![
        FileFilter::new("All files", Vec::<String>::new()),
        FileFilter::new("Rust files", ["rs"]),
    ]
}

fn picker(
    window: &mut Window,
    cx: &mut Context<FilePickerStory>,
    sim: &RemoteSim,
    configure: impl FnOnce(FilePickerState, &mut Context<FilePickerState>) -> FilePickerState,
) -> Entity<FilePickerState> {
    let signing = sim.clone();
    let state = cx.new(|cx| {
        let this = cx.weak_entity();
        let state = FilePickerState::new(window, cx).on_auth_required(move |_, cx| {
            signing.sign_in();
            this.update(cx, |state, cx| state.retry(cx)).ok();
        });
        let state = if cfg!(feature = "snapshot") {
            state
                .with_source(fixture(), window, cx)
                .allow_new_folder(true)
        } else {
            state
        };
        configure(state, cx)
    });
    cx.observe(&state, |_, _, cx| cx.notify()).detach();
    state
}

impl Story for FilePickerStory {
    fn title() -> &'static str {
        "File picker"
    }

    fn icon() -> IconName {
        IconName::File
    }

    fn description() -> &'static str {
        "A dialog to choose one file or several: a typed path, a fuzzy filter, a file type \
         filter, and listings that load without blocking."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let sim = RemoteSim::new(if cfg!(feature = "snapshot") {
                Duration::ZERO
            } else {
                Duration::from_millis(700)
            });
            let one = picker(window, cx, &sim, |state, cx| {
                state.with_filters(filters(), cx)
            });
            let many = picker(window, cx, &sim, |state, cx| {
                state.with_multiple(true).with_filters(filters(), cx)
            });
            cx.subscribe(&one, |this: &mut Self, _, event, cx| {
                match event {
                    FilePickerEvent::Confirmed(paths) => {
                        let path = paths.first().map(|path| path.to_string());
                        this.chosen = format!("Chose {}.", path.unwrap_or_default()).into();
                    }
                    FilePickerEvent::Cancelled => {}
                    _ => return,
                }
                this.one_open = false;
                cx.notify();
            })
            .detach();
            cx.subscribe(&many, |this: &mut Self, _, event, cx| {
                match event {
                    FilePickerEvent::Confirmed(paths) => {
                        this.chosen_many = paths.iter().map(|p| p.to_string()).collect();
                    }
                    FilePickerEvent::Cancelled => {}
                    _ => return,
                }
                this.many_open = false;
                cx.notify();
            })
            .detach();
            Self {
                one_open: false,
                many_open: false,
                one,
                many,
                allow_new_folder: cfg!(feature = "snapshot"),
                remote: false,
                sim,
                chosen: "No file chosen.".into(),
                chosen_many: Vec::new(),
            }
        })
        .into()
    }
}

impl Render for FilePickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let (open_one, open_many) = (this.clone(), this.clone());
        let (close_one, close_many) = (this.clone(), this.clone());
        let toggle = this.clone();
        let remote_toggle = this;
        let many_summary: SharedString = match self.chosen_many.len() {
            0 => "No files chosen.".into(),
            1 => "1 file chosen:".into(),
            count => format!("{count} files chosen:").into(),
        };
        page([
            section(
                "Single file",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "A click selects a file and Enter, a double click, or the open button \
                         chooses it. The file type menu offers all files and Rust files. The \
                         picker opens in the source's home folder, and `~` in the path stands \
                         for it.",
                        cx,
                    ))
                    .child(div().flex().child(
                        picker_trigger(Self::TRIGGER_ONE, "Open a file", IconName::File).on_click(
                            move |_, _, cx| {
                                open_one
                                    .update(cx, |this, cx| {
                                        this.one_open = true;
                                        cx.notify();
                                    })
                                    .ok();
                            },
                        ),
                    ))
                    .child(note(self.chosen.clone(), cx))
                    .child(
                        FilePicker::new("file-picker-one", &self.one)
                            .open(self.one_open)
                            .title("Open a file")
                            .on_open_change(move |_, _, cx| {
                                close_one
                                    .update(cx, |this, cx| {
                                        this.one_open = false;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ),
            )
            .into_any_element(),
            section(
                "Multiple files",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Cmd (Ctrl elsewhere) toggles a file, Shift selects a range, and \
                         Cmd+A selects every file while the path has no query. The open button \
                         counts the files. Cmd+Enter chooses the selection.",
                        cx,
                    ))
                    .child(div().flex().child(
                        picker_trigger(Self::TRIGGER_MANY, "Open files", IconName::File).on_click(
                            move |_, _, cx| {
                                open_many
                                    .update(cx, |this, cx| {
                                        this.many_open = true;
                                        cx.notify();
                                    })
                                    .ok();
                            },
                        ),
                    ))
                    .child(note(many_summary, cx))
                    .children(
                        self.chosen_many
                            .iter()
                            .map(|path| note(path.clone(), cx).into_any_element()),
                    )
                    .child(
                        FilePicker::new("file-picker-many", &self.many)
                            .open(self.many_open)
                            .title("Open files")
                            .on_open_change(move |_, _, cx| {
                                close_many
                                    .update(cx, |this, cx| {
                                        this.many_open = false;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ),
            )
            .into_any_element(),
            section(
                "New folder",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "New folder adds a button that opens a row at the top of the list. \
                         Type a name and press Enter to make the folder, or Escape to cancel. \
                         Cmd+Shift+N (Ctrl+Shift+N elsewhere) opens the row too. It shows only \
                         when the source can make folders, and the picker puts the highlight \
                         on the new folder.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child({
                                let (one, many) = (self.one.clone(), self.many.clone());
                                Switch::new("allow-new-folder")
                                    .checked(self.allow_new_folder)
                                    .accessibility_label("Allow new folder")
                                    .on_change(move |value, _, cx| {
                                        let value = *value;
                                        for state in [&one, &many] {
                                            state.update(cx, |state, cx| {
                                                state.set_allow_new_folder(value, cx)
                                            });
                                        }
                                        toggle
                                            .update(cx, |this, cx| {
                                                this.allow_new_folder = value;
                                                cx.notify();
                                            })
                                            .ok();
                                    })
                            })
                            .child("Allow new folder"),
                    ),
            )
            .into_any_element(),
            section(
                "Remote (simulated)",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Both pickers list a build server held in memory, with POSIX paths \
                         whatever this machine is. Every answer takes a moment and the dialog \
                         keeps its size. Open /srv/data/ to lose the connection once and press \
                         Retry, or /secure/ to be asked to sign in.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Switch::new("remote-simulated")
                                    .checked(self.remote)
                                    .accessibility_label("Remote (simulated)")
                                    .on_change(move |value, window, cx| {
                                        let value = *value;
                                        remote_toggle
                                            .update(cx, |this, cx| {
                                                this.set_remote(value, window, cx)
                                            })
                                            .ok();
                                    }),
                            )
                            .child("Remote (simulated)"),
                    ),
            )
            .into_any_element(),
            section(
                "Filters and hidden files",
                note(
                    "Folders always show, so a filter never hides the way to a file. A line \
                     under the list says when the filter or the hidden switch hides files, and \
                     offers to show them. Hidden entries wait behind the switch.",
                    cx,
                ),
            )
            .into_any_element(),
        ])
    }
}
