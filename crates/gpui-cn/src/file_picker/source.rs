//! Where a file picker gets its entries: a trait any file system, remote
//! machine, or API can implement, the local disk as the default, and an
//! in-memory source for fixtures.

use std::{collections::HashMap, io, path::Path, path::PathBuf, time::SystemTime};

use gpui_kit::{App, AppContext as _, SharedString, Task};
use smol::stream::StreamExt as _;

use crate::path_browser::{Entry, PageToken};

/// What an entry of a listing is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileKind {
    /// A folder, which a picker can go inside.
    Folder,
    /// A file, which a picker can choose.
    File,
}

/// A file or a folder in a listing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileEntry {
    name: SharedString,
    kind: FileKind,
    hidden: bool,
    size: Option<u64>,
    modified: Option<SystemTime>,
}

impl FileEntry {
    fn of(name: impl Into<SharedString>, kind: FileKind) -> Self {
        let name = name.into();
        let hidden = name.starts_with('.');
        Self {
            name,
            kind,
            hidden,
            size: None,
            modified: None,
        }
    }

    /// A file with a name. A name that starts with a dot is hidden.
    pub fn file(name: impl Into<SharedString>) -> Self {
        Self::of(name, FileKind::File)
    }

    /// A folder with a name. A name that starts with a dot is hidden.
    pub fn folder(name: impl Into<SharedString>) -> Self {
        Self::of(name, FileKind::Folder)
    }

    /// Sets whether the entry is hidden.
    pub fn with_hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Sets the size in bytes, which a file's row shows.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Sets the time of the last change.
    pub fn with_modified(mut self, modified: SystemTime) -> Self {
        self.modified = Some(modified);
        self
    }

    /// The name, without a path.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// Whether the entry is a file or a folder.
    pub fn kind(&self) -> FileKind {
        self.kind
    }

    /// Whether the entry is a folder.
    pub fn is_folder(&self) -> bool {
        self.kind == FileKind::Folder
    }

    /// Whether the entry is hidden.
    pub fn hidden(&self) -> bool {
        self.hidden
    }

    /// The size in bytes, when the source knows it.
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// The time of the last change, when the source knows it.
    pub fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// The extension of a file's name, without the dot and in lower case.
    pub fn extension(&self) -> Option<String> {
        if self.is_folder() {
            return None;
        }
        let (stem, extension) = self.name.rsplit_once('.')?;
        (!stem.is_empty() && !extension.is_empty()).then(|| extension.to_lowercase())
    }
}

impl Entry for FileEntry {
    fn name(&self) -> &SharedString {
        &self.name
    }

    fn hidden(&self) -> bool {
        self.hidden
    }

    fn is_folder(&self) -> bool {
        self.kind == FileKind::Folder
    }
}

/// Folders first, then files, the visible before the hidden in each group,
/// and each group in alphabetical order without regard to case: the order
/// of [`LocalFiles`] and [`MemoryFiles`].
pub(crate) fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| {
        b.is_folder()
            .cmp(&a.is_folder())
            .then_with(|| a.hidden.cmp(&b.hidden))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// One page of a listing: entries, and where the next page starts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FilePage {
    pub(super) entries: Vec<FileEntry>,
    pub(super) next: Option<PageToken>,
}

impl FilePage {
    /// The last page of a listing, with `entries` in the order to show.
    pub fn new(entries: impl IntoIterator<Item = FileEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            next: None,
        }
    }

    /// Says that more entries follow, from `token`. The picker shows a
    /// "Load more" row and asks for that page when the user reaches it.
    pub fn with_next(mut self, token: PageToken) -> Self {
        self.next = Some(token);
        self
    }

    /// The entries of the page.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Where the next page starts, when there is one.
    pub fn next(&self) -> Option<&PageToken> {
        self.next.as_ref()
    }
}

/// Where a [`FilePicker`](super::FilePicker) lists entries from: the local
/// disk, an SSH session, a bucket, an API. The picker never waits for a
/// source. It calls `list` and awaits the task, and it drops the task to
/// cancel it.
///
/// A source lists one directory a page at a time and gives entries in the
/// order to show. The picker keeps that order and sorts nothing, so a
/// source that pages must sort across its pages itself. Entries that
/// repeat across pages, by name, show once.
///
/// ```
/// use std::{io, path::Path};
///
/// use gpui_cn::{FileEntry, FilePage, FileSource, PageToken};
/// use gpui_kit::{App, AppContext as _, Task};
///
/// /// A source with the same two files in every folder.
/// struct Fixed;
///
/// impl FileSource for Fixed {
///     fn list(
///         &self,
///         _dir: &Path,
///         _page: Option<PageToken>,
///         cx: &mut App,
///     ) -> Task<io::Result<FilePage>> {
///         // Any work goes on a task: a request, a query, a disk read.
///         cx.background_spawn(async move {
///             Ok(FilePage::new([
///                 FileEntry::file("notes.txt").with_size(120),
///                 FileEntry::file("main.rs").with_size(2048),
///             ]))
///         })
///     }
/// }
/// ```
pub trait FileSource: 'static {
    /// Lists the entries of `dir`, from the start when `page` is `None`,
    /// or else from the page `token` a previous page named.
    fn list(&self, dir: &Path, page: Option<PageToken>, cx: &mut App)
    -> Task<io::Result<FilePage>>;

    /// The source's home folder, which a leading `~` in the typed path
    /// stands for and where a picker with no start of its own begins. The
    /// default is `None`: a source with no home does not expand `~`, and
    /// the picker starts at the root.
    fn home(&self) -> Option<PathBuf> {
        None
    }
}

/// The local file system, in one page. It reads on a background task,
/// counts a symlink to a folder as a folder, and lists in the order of
/// [`FileEntry`] sorting: folders first, then files.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalFiles;

impl FileSource for LocalFiles {
    /// The home directory of the user running the application.
    fn home(&self) -> Option<PathBuf> {
        std::env::home_dir()
    }

    fn list(
        &self,
        dir: &Path,
        _page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<io::Result<FilePage>> {
        cx.background_spawn(list_local(dir.to_path_buf()))
    }
}

async fn list_local(dir: PathBuf) -> io::Result<FilePage> {
    let mut read = smol::fs::read_dir(&dir).await?;
    let mut entries = Vec::new();
    while let Some(entry) = read.next().await {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type().await else {
            continue;
        };
        let meta = if kind.is_symlink() {
            smol::fs::metadata(entry.path()).await.ok()
        } else {
            entry.metadata().await.ok()
        };
        let mut found = match &meta {
            Some(meta) if meta.is_dir() => FileEntry::folder(name),
            _ => FileEntry::file(name),
        };
        if let Some(meta) = meta {
            if !meta.is_dir() {
                found = found.with_size(meta.len());
            }
            if let Ok(modified) = meta.modified() {
                found = found.with_modified(modified);
            }
        }
        entries.push(found);
    }
    sort_entries(&mut entries);
    Ok(FilePage::new(entries))
}

/// A file system held in memory, for fixtures, demos, and tests. It lists
/// a directory the application named, in the order of [`LocalFiles`], and
/// fails for any other directory.
#[derive(Clone, Debug, Default)]
pub struct MemoryFiles {
    home: Option<PathBuf>,
    dirs: HashMap<PathBuf, Vec<FileEntry>>,
}

impl MemoryFiles {
    /// A file system with no directories.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the home folder, which `~` stands for.
    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    /// Adds the directory `path` with `entries`.
    pub fn with_dir(
        mut self,
        path: impl Into<PathBuf>,
        entries: impl IntoIterator<Item = FileEntry>,
    ) -> Self {
        let mut entries: Vec<FileEntry> = entries.into_iter().collect();
        sort_entries(&mut entries);
        self.dirs.insert(path.into(), entries);
        self
    }
}

impl FileSource for MemoryFiles {
    fn home(&self) -> Option<PathBuf> {
        self.home.clone()
    }

    fn list(
        &self,
        dir: &Path,
        _page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<io::Result<FilePage>> {
        let found = self.dirs.get(dir).cloned();
        cx.background_spawn(async move {
            found
                .map(FilePage::new)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such directory"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(entries: &[FileEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name().as_ref()).collect()
    }

    #[test]
    fn folders_come_first_then_files_each_by_name_without_case() {
        let mut entries = vec![
            FileEntry::file("beta.txt"),
            FileEntry::folder("src"),
            FileEntry::file("Alpha.txt"),
            FileEntry::folder(".git"),
            FileEntry::file(".env"),
            FileEntry::folder("Docs"),
        ];
        sort_entries(&mut entries);
        assert_eq!(
            names(&entries),
            ["Docs", "src", ".git", "Alpha.txt", "beta.txt", ".env"]
        );
    }

    #[test]
    fn a_leading_dot_hides_an_entry_and_can_be_overridden() {
        assert!(FileEntry::file(".env").hidden());
        assert!(!FileEntry::file(".env").with_hidden(false).hidden());
        assert!(FileEntry::folder("src").with_hidden(true).hidden());
    }

    #[test]
    fn the_extension_is_lower_case_and_needs_a_stem() {
        assert_eq!(
            FileEntry::file("Main.RS").extension().as_deref(),
            Some("rs")
        );
        assert_eq!(
            FileEntry::file("a.tar.gz").extension().as_deref(),
            Some("gz")
        );
        assert_eq!(FileEntry::file(".env").extension(), None);
        assert_eq!(FileEntry::file("Makefile").extension(), None);
        assert_eq!(FileEntry::folder("v1.2").extension(), None);
    }
}
