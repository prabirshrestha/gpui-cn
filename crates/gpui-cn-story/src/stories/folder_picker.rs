use std::{io, path::PathBuf, rc::Rc};

use gpui_cn::{
    Button, FolderEntry, FolderLister, FolderPicker, FolderPickerEvent, FolderPickerState,
    gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div,
};

use crate::{Story, note, page, section};

/// The home folder the fixture lists in the snapshot build.
const FIXTURE_HOME: &str = "/home/prabirshrestha";

/// A dialog to choose a folder on any file system.
pub struct FolderPickerStory {
    open: bool,
    state: Entity<FolderPickerState>,
    chosen: SharedString,
}

impl FolderPickerStory {
    /// The button that opens the picker.
    pub const TRIGGER: &'static str = "folder-picker-trigger";
}

/// A fixed set of folders, so the snapshot does not depend on the disk
/// it runs on.
fn fixture_lister() -> FolderLister {
    Rc::new(|dir: PathBuf, cx: &mut App| {
        cx.background_spawn(async move {
            let names: &[&str] = match dir.to_str() {
                Some(FIXTURE_HOME) => &[
                    "code",
                    ".cache",
                    ".codex",
                    ".config",
                    ".docker-stack-deploy",
                    ".local",
                    ".npm",
                    ".ssh",
                ],
                Some("/home/prabirshrestha/.zed_server") => &[],
                Some("/home/prabirshrestha/code") => &["gpui-cn", "gpui-kit", "psl-tools"],
                _ => return Err(io::Error::other("not in the fixture")),
            };
            Ok(names.iter().map(|name| FolderEntry::new(*name)).collect())
        })
    })
}

/// The folder the picker opens in: the fixture's in the snapshot build,
/// the user's home otherwise.
fn start_dir() -> PathBuf {
    if cfg!(feature = "snapshot") {
        return PathBuf::from(FIXTURE_HOME);
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
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
            let state = cx.new(|cx| {
                let state = FolderPickerState::new(window, cx);
                let state = if cfg!(feature = "snapshot") {
                    state.with_lister(fixture_lister(), cx)
                } else {
                    state
                };
                state.with_initial(start_dir(), window, cx)
            });
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            cx.subscribe(&state, |this: &mut Self, _, event, cx| {
                if let FolderPickerEvent::Chosen(path) = event {
                    this.chosen = format!("Chose {}.", path.display()).into();
                }
                this.open = false;
                cx.notify();
            })
            .detach();
            Self {
                open: false,
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
        page([section(
            "Local file system",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(
                    "Type an absolute path: the text up to the last slash is the folder \
                     that is listed, and the text after it filters that folder's \
                     sub-folders. A slash after the filter goes inside the best match. \
                     Enter or a click on a row goes inside it, and the arrow button goes \
                     up.",
                    cx,
                ))
                .child(
                    div().flex().child(
                        Button::new(Self::TRIGGER)
                            .label("Choose a folder")
                            .on_click(move |_, _, cx| {
                                opener
                                    .update(cx, |this, cx| {
                                        this.open = true;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ),
                )
                .child(note(self.chosen.clone(), cx))
                .child(
                    FolderPicker::new("folder-picker", &self.state)
                        .open(self.open)
                        .title("Choose a source folder")
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
