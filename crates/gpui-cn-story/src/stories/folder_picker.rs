use gpui_cn::{
    FolderEntry, FolderPage, FolderPicker, FolderPickerEvent, FolderPickerState, FolderSource,
    ListError, PageToken, PathStyle, SourcePath, gpui_kit::assets::IconName,
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
    state: Entity<FolderPickerState>,
    chosen: SharedString,
}

impl FolderPickerStory {
    /// The button that opens the picker.
    pub const TRIGGER: &'static str = "folder-picker-trigger";
}

/// A fixed set of folders, so the snapshot does not depend on the disk
/// it runs on. The code folder answers in two pages.
struct Fixture;

impl FolderSource for Fixture {
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
        cx.background_spawn(async move {
            let (names, next): (&[&str], Option<&str>) = match (Some(dir.as_str()), page.as_ref()) {
                (Some(FIXTURE_HOME), _) => (
                    &[
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
                (Some("/home/prabirshrestha/.zed_server"), _) => (&[], None),
                (Some("/home/prabirshrestha/code"), None) => {
                    (&["gpui-cn", "gpui-kit", "psl-tools"], Some("2"))
                }
                (Some("/home/prabirshrestha/code"), Some(_)) => (&["website", "zed"], None),
                _ => return Err(ListError::NotFound),
            };
            let page = FolderPage::new(names.iter().map(|name| FolderEntry::new(*name)));
            Ok(match next {
                Some(token) => page.with_next(PageToken::new(token)),
                None => page,
            })
        })
    }
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
                if cfg!(feature = "snapshot") {
                    state.with_source(Fixture, window, cx)
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
                    "The picker opens in the source's home folder, and `~` in the path \
                     stands for it: type `~/code/` to list the code folder under home. \
                     Type a path: the text up to the last slash is the folder \
                     that is listed, and the text after it filters that folder's \
                     sub-folders. A slash after the filter goes inside the best match. \
                     Enter or a click on a row goes inside it, and the arrow button goes \
                     up. Cmd+Enter (Ctrl+Enter elsewhere) chooses the folder.",
                    cx,
                ))
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
