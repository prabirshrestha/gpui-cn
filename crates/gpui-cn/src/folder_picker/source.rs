//! Where a folder picker gets its folders: a trait any file system,
//! remote machine, or API can implement, and the local disk as the
//! default.

use std::{io, path::Path, path::PathBuf, sync::Arc};

use gpui_kit::{App, AppContext as _, SharedString, Task};
use smol::stream::StreamExt as _;

/// A folder in a listing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FolderEntry {
    name: SharedString,
    hidden: bool,
}

impl FolderEntry {
    /// A folder with a name. A name that starts with a dot is hidden.
    pub fn new(name: impl Into<SharedString>) -> Self {
        let name = name.into();
        let hidden = name.starts_with('.');
        Self { name, hidden }
    }

    /// Whether the folder is hidden, which sorts it after the others.
    pub fn with_hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// The folder's name, without a path.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// Whether the folder is hidden.
    pub fn hidden(&self) -> bool {
        self.hidden
    }
}

/// Where the next page of a listing starts. The source makes it and reads
/// it back, so it can hold anything: an offset, a cursor, a continuation
/// token. The picker only stores it and hands it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageToken(Arc<str>);

impl PageToken {
    /// A token that holds `value`.
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    /// The value the source put in the token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One page of a listing: folders, and where the next page starts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FolderPage {
    entries: Vec<FolderEntry>,
    next: Option<PageToken>,
}

impl FolderPage {
    /// The last page of a listing, with `entries` in the order to show.
    pub fn new(entries: impl IntoIterator<Item = FolderEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            next: None,
        }
    }

    /// Says that more folders follow, from `token`. The picker shows a
    /// "Load more" row and asks for that page when the user reaches it.
    pub fn with_next(mut self, token: PageToken) -> Self {
        self.next = Some(token);
        self
    }

    /// The folders of the page.
    pub fn entries(&self) -> &[FolderEntry] {
        &self.entries
    }

    /// Where the next page starts, when there is one.
    pub fn next(&self) -> Option<&PageToken> {
        self.next.as_ref()
    }
}

/// Where a [`FolderPicker`](super::FolderPicker) lists folders from: the
/// local disk, an SSH session, a bucket, an API. The picker never waits
/// for a source. It calls `list` and awaits the task, and it drops the
/// task to cancel it.
///
/// A source lists one directory a page at a time. It should give entries
/// in the order to show: the picker keeps that order and sorts nothing,
/// so a source that pages must sort across its pages itself. Folders
/// that repeat across pages, by name, show once.
///
/// ```
/// use std::{io, path::Path};
///
/// use gpui_cn::{FolderEntry, FolderPage, FolderSource, PageToken};
/// use gpui_kit::{App, AppContext as _, Task};
///
/// /// A source over a list of names, ten to a page.
/// struct Names(Vec<String>);
///
/// impl FolderSource for Names {
///     fn list(
///         &self,
///         _dir: &Path,
///         page: Option<PageToken>,
///         cx: &mut App,
///     ) -> Task<io::Result<FolderPage>> {
///         let start = page.and_then(|t| t.as_str().parse().ok()).unwrap_or(0);
///         let end = (start + 10).min(self.0.len());
///         let names = self.0[start..end].to_vec();
///         let more = (end < self.0.len()).then(|| PageToken::new(end.to_string()));
///         // Any work goes on a task: a request, a query, a disk read.
///         cx.background_spawn(async move {
///             let page = FolderPage::new(names.into_iter().map(FolderEntry::new));
///             Ok(match more {
///                 Some(token) => page.with_next(token),
///                 None => page,
///             })
///         })
///     }
/// }
/// ```
pub trait FolderSource: 'static {
    /// Lists the folders of `dir`, from the start when `page` is `None`,
    /// or else from the page `token` a previous page named.
    fn list(
        &self,
        dir: &Path,
        page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<io::Result<FolderPage>>;
}

/// The local file system, in one page. It reads on a background task,
/// counts a symlink to a folder as a folder, and puts the visible folders
/// first and the hidden ones after, each in alphabetical order without
/// regard to case.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalFolders;

impl FolderSource for LocalFolders {
    fn list(
        &self,
        dir: &Path,
        _page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<io::Result<FolderPage>> {
        cx.background_spawn(list_local(dir.to_path_buf()))
    }
}

async fn list_local(dir: PathBuf) -> io::Result<FolderPage> {
    let mut read = smol::fs::read_dir(&dir).await?;
    let mut folders = Vec::new();
    while let Some(entry) = read.next().await {
        let Ok(entry) = entry else { continue };
        let is_folder = match entry.file_type().await {
            Ok(kind) if kind.is_dir() => true,
            Ok(kind) if kind.is_symlink() => smol::fs::metadata(entry.path())
                .await
                .is_ok_and(|meta| meta.is_dir()),
            _ => false,
        };
        if is_folder {
            folders.push(FolderEntry::new(
                entry.file_name().to_string_lossy().into_owned(),
            ));
        }
    }
    folders.sort_by(|a, b| {
        a.hidden()
            .cmp(&b.hidden())
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
            .then_with(|| a.name().cmp(b.name()))
    });
    Ok(FolderPage::new(folders))
}
