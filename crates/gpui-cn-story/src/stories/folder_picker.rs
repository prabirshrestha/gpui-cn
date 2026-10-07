use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::remote::RemoteSim;
use gpui_cn::{
    CreateFolderError, FolderEntry, FolderPage, FolderPicker, FolderPickerEvent, FolderPickerState,
    FolderSource, ListError, PageToken, PathStyle, SourcePath, Switch, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Task, Window, div,
};

use crate::{Story, note, page, picker_trigger, section};

/// The home folder the fixture lists in the snapshot build.
const FIXTURE_HOME: &str = "/home/prabirshrestha";

/// A dialog to choose a folder on any file system.
pub struct FolderPickerStory {
    open: bool,
    allow_new_folder: bool,
    remote: bool,
    sim: RemoteSim,
    state: Entity<FolderPickerState>,
    chosen: SharedString,
}

impl FolderPickerStory {
    /// The button that opens the picker.
    pub const TRIGGER: &'static str = "folder-picker-trigger";

    /// The state of the picker.
    pub fn state(&self) -> &Entity<FolderPickerState> {
        &self.state
    }

    /// The simulated build server the picker lists while the remote switch
    /// is on.
    pub fn sim(&self) -> &RemoteSim {
        &self.sim
    }

    /// Lists from the simulated build server, or from the local file
    /// system (the fixture, in a snapshot build) again.
    pub fn set_remote(&mut self, remote: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.remote = remote;
        let sim = self.sim.clone();
        self.state.update(cx, |state, cx| {
            if remote {
                state.set_source(sim, window, cx);
            } else if cfg!(feature = "snapshot") {
                state.set_source(Fixture::default(), window, cx);
            } else {
                state.set_source(gpui_cn::LocalFolders, window, cx);
            }
        });
        cx.notify();
    }
}

/// A fixed set of folders, so the snapshot does not depend on the disk
/// it runs on. The code folder answers in two pages.
#[derive(Clone, Default)]
struct Fixture {
    /// The folders made in this session, which the fixture then lists.
    made: Rc<RefCell<Vec<SourcePath>>>,
}

impl FolderSource for Fixture {
    fn can_create_folders(&self) -> bool {
        true
    }

    fn create_folder(
        &self,
        parent: &SourcePath,
        name: &str,
        _: &mut App,
    ) -> Task<Result<SourcePath, CreateFolderError>> {
        let path = parent.join(name);
        let taken = self.made.borrow().contains(&path)
            || fixture_names(parent.as_str(), None).is_some_and(|(names, _)| {
                names.iter().any(|known| known.eq_ignore_ascii_case(name))
            });
        if taken {
            return Task::ready(Err(CreateFolderError::Exists));
        }
        self.made.borrow_mut().push(path.clone());
        Task::ready(Ok(path))
    }

    fn home(&self) -> Option<SourcePath> {
        Some(PathStyle::posix().path(FIXTURE_HOME))
    }

    fn path_style(&self) -> PathStyle {
        PathStyle::posix()
    }

    fn list(
        &self,
        dir: &SourcePath,
        page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<Result<FolderPage, ListError>> {
        let dir = dir.clone();
        let made: Vec<SourcePath> = self.made.borrow().clone();
        cx.background_spawn(async move {
            let known = fixture_names(dir.as_str(), page.as_ref().map(|token| token.as_str()));
            let (names, next) = match known {
                Some(found) => found,
                None if made.contains(&dir) => (Vec::new(), None),
                None => return Err(ListError::NotFound),
            };
            let mut entries: Vec<FolderEntry> = names.into_iter().map(FolderEntry::new).collect();
            entries.extend(
                made.iter()
                    .filter(|path| path.parent().as_ref() == Some(&dir))
                    .filter_map(|path| path.file_name().map(FolderEntry::new)),
            );
            let style = PathStyle::posix();
            entries.sort_by(|a, b| {
                a.hidden()
                    .cmp(&b.hidden())
                    .then_with(|| style.compare_names(a.name(), b.name()))
            });
            let page = FolderPage::new(entries);
            Ok(match next {
                Some(token) => page.with_next(PageToken::new(token)),
                None => page,
            })
        })
    }
}

/// The folders the fixture holds in `dir`, and where the next page starts.
type Names = (Vec<&'static str>, Option<&'static str>);

fn fixture_names(dir: &str, page: Option<&str>) -> Option<Names> {
    Some(match (dir, page) {
        (FIXTURE_HOME, _) => (
            vec![
                "code",
                ".cache",
                ".codex",
                ".config",
                ".docker-stack-deploy",
                ".local",
                ".npm",
                ".ssh",
            ],
            None,
        ),
        ("/home/prabirshrestha/.zed_server", _) => (Vec::new(), None),
        ("/home/prabirshrestha/code", None) => {
            (vec!["gpui-cn", "gpui-kit", "psl-tools"], Some("2"))
        }
        ("/home/prabirshrestha/code", Some(_)) => (vec!["website", "zed"], None),
        _ => return None,
    })
}

impl Story for FolderPickerStory {
    fn title() -> &'static str {
        "Folder picker"
    }

    fn icon() -> IconName {
        IconName::FolderOpen
    }

    fn description() -> &'static str {
        "A dialog to choose a folder: a typed path, a fuzzy filter, and listings that load \
         without blocking."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let sim = RemoteSim::new(if cfg!(feature = "snapshot") {
                Duration::ZERO
            } else {
                Duration::from_millis(700)
            });
            let signing = sim.clone();
            let state = cx.new(|cx| {
                let this = cx.weak_entity();
                let state = FolderPickerState::new(window, cx).on_auth_required(move |_, cx| {
                    signing.sign_in();
                    this.update(cx, |state, cx| state.retry(cx)).ok();
                });
                if cfg!(feature = "snapshot") {
                    state
                        .with_source(Fixture::default(), window, cx)
                        .allow_new_folder(true)
                } else {
                    state
                }
            });
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            cx.subscribe(&state, |this: &mut Self, _, event, cx| {
                if let FolderPickerEvent::Chosen(path) = event {
                    this.chosen = format!("Chose {path}.").into();
                }
                this.open = false;
                cx.notify();
            })
            .detach();
            Self {
                open: false,
                allow_new_folder: cfg!(feature = "snapshot"),
                remote: false,
                sim,
                state,
                chosen: "No folder chosen.".into(),
            }
        })
        .into()
    }
}

impl Render for FolderPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let opener = this.clone();
        let toggle = this.clone();
        let remote_toggle = this.clone();
        page([section(
            "Local file system",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(
                    "The picker opens in the source's home folder, and `~` in the path \
                     stands for it: type `~/code/` to list the code folder under home. \
                     Type a path: the text up to the last slash is the folder \
                     that is listed, and the text after it filters that folder's \
                     sub-folders. A slash after the filter goes inside the best match. \
                     Enter or a click on a row goes inside it, and the arrow button goes \
                     up. Cmd+Enter (Ctrl+Enter elsewhere) chooses the folder.",
                    cx,
                ))
                .child(note(
                    "New folder adds a button that opens a row at the top of the list. Type \
                     a name and press Enter to make the folder, or Escape to cancel. \
                     Cmd+Shift+N (Ctrl+Shift+N elsewhere) opens the row too. It shows only \
                     when the source can make folders, and the new folder becomes the one \
                     Use folder chooses.",
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child({
                            let state = self.state.clone();
                            Switch::new("allow-new-folder")
                                .checked(self.allow_new_folder)
                                .accessibility_label("Allow new folder")
                                .on_change(move |value, _, cx| {
                                    let value = *value;
                                    state.update(cx, |state, cx| {
                                        state.set_allow_new_folder(value, cx)
                                    });
                                    toggle
                                        .update(cx, |this, cx| {
                                            this.allow_new_folder = value;
                                            cx.notify();
                                        })
                                        .ok();
                                })
                        })
                        .child("Allow new folder"),
                )
                .child(note(
                    "Remote (simulated) lists a build server held in memory, with POSIX \
                     paths whatever this machine is. Every answer takes a moment and the \
                     dialog keeps its size. Open /srv/data/ to lose the connection once and \
                     press Retry, or /secure/ to be asked to sign in.",
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
                                        .update(cx, |this, cx| this.set_remote(value, window, cx))
                                        .ok();
                                }),
                        )
                        .child("Remote (simulated)"),
                )
                .child(div().flex().child(
                    picker_trigger(Self::TRIGGER, "Choose a folder", IconName::Folder).on_click(
                        move |_, _, cx| {
                            opener
                                .update(cx, |this, cx| {
                                    this.open = true;
                                    cx.notify();
                                })
                                .ok();
                        },
                    ),
                ))
                .child(note(self.chosen.clone(), cx))
                .child(
                    FolderPicker::new("folder-picker", &self.state)
                        .open(self.open)
                        .title("Choose a folder")
                        .on_open_change(move |_, _, cx| {
                            this.update(cx, |this, cx| {
                                this.open = false;
                                cx.notify();
                            })
                            .ok();
                        }),
                ),
        )
        .into_any_element()])
    }
}
