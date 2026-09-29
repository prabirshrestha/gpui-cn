//! The folder picker's pure logic: how the path text splits into a
//! directory and a query, how a query ranks the folders, and which folder
//! a typed separator goes into. It works on plain strings, so it needs no
//! window and no file system.

use std::{io, ops::Range, path::PathBuf, rc::Rc};

use gpui_kit::{App, SharedString, Task};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{Atom, AtomKind, CaseMatching, Normalization},
};

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

/// Lists the folders of a directory. The task can read a local disk, or
/// ask a remote machine; the picker never waits for it, and it drops the
/// task to cancel it.
///
/// See [`FolderPickerState::with_lister`](super::FolderPickerState::with_lister).
pub type FolderLister = Rc<dyn Fn(PathBuf, &mut App) -> Task<io::Result<Vec<FolderEntry>>>>;

/// A folder that matches the query, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Match {
    /// The index of the folder in the listing.
    pub(crate) index: usize,
    /// The score: a higher one ranks first.
    pub(crate) score: u32,
    /// The byte ranges of the name the query matched.
    pub(crate) ranges: Vec<Range<usize>>,
}

/// Whether `c` separates the parts of a path.
pub(crate) fn is_separator(c: char, windows: bool) -> bool {
    c == '/' || (windows && c == '\\')
}

/// The separator the picker writes.
pub(crate) fn separator(windows: bool) -> char {
    if windows { '\\' } else { '/' }
}

/// Splits the path text at its last separator: the directory, with its
/// separator, and the query after it. Text with no separator has the
/// root as its directory.
pub(crate) fn split_path(text: &str, windows: bool) -> (String, String) {
    match text.rfind(|c| is_separator(c, windows)) {
        Some(at) => (text[..=at].to_string(), text[at + 1..].to_string()),
        None => (separator(windows).to_string(), text.to_string()),
    }
}

/// The text of the directory above `dir`, with a trailing separator. The
/// root is its own parent.
pub(crate) fn parent_text(dir: &str, windows: bool) -> String {
    let trimmed = dir.trim_end_matches(|c| is_separator(c, windows));
    if is_root(dir, windows) {
        return dir.to_string();
    }
    match trimmed.rfind(|c| is_separator(c, windows)) {
        Some(at) => trimmed[..=at].to_string(),
        None => separator(windows).to_string(),
    }
}

/// `dir` with `name` inside it, and a trailing separator.
pub(crate) fn join_dir(dir: &str, name: &str, windows: bool) -> String {
    let mut text = dir.to_string();
    if !text.ends_with(|c| is_separator(c, windows)) {
        text.push(separator(windows));
    }
    text.push_str(name);
    text.push(separator(windows));
    text
}

/// Whether `dir` is a file system root: `/`, or a drive such as `C:\` on
/// Windows.
fn is_root(dir: &str, windows: bool) -> bool {
    let trimmed = dir.trim_end_matches(|c| is_separator(c, windows));
    trimmed.is_empty() || (windows && trimmed.ends_with(':'))
}

/// The path a directory's text names: no trailing separator, except for
/// the root.
pub(crate) fn directory_path(dir: &str, windows: bool) -> PathBuf {
    if is_root(dir, windows) {
        return PathBuf::from(if dir.is_empty() {
            separator(windows).to_string()
        } else {
            dir.to_string()
        });
    }
    PathBuf::from(dir.trim_end_matches(|c| is_separator(c, windows)))
}

/// Ranks `entries` by how well `query` matches their names: a subsequence
/// of the name, ignoring case, scored so that runs and word starts rank
/// first. Ties go to the shorter name, then keep the listing's order. The folders that do not match
/// are left out.
pub(crate) fn filter(entries: &[FolderEntry], query: &str) -> Vec<Match> {
    let atom = Atom::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
        false,
    );
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buffer = Vec::new();
    let mut indices = Vec::new();
    let mut matches: Vec<Match> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            indices.clear();
            let haystack = Utf32Str::new(entry.name(), &mut buffer);
            let score = atom.indices(haystack, &mut matcher, &mut indices)?;
            indices.sort_unstable();
            indices.dedup();
            Some(Match {
                index,
                score: u32::from(score),
                ranges: byte_ranges(entry.name(), &indices),
            })
        })
        .collect();
    matches.sort_by(|a, b| {
        let length = |found: &Match| entries[found.index].name().chars().count();
        b.score
            .cmp(&a.score)
            .then_with(|| length(a).cmp(&length(b)))
            .then_with(|| a.index.cmp(&b.index))
    });
    matches
}

/// The byte ranges of the characters at `indices`, joined where they run
/// together.
fn byte_ranges(text: &str, indices: &[u32]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut chars = text.char_indices().enumerate();
    for &wanted in indices {
        let Some((_, (start, c))) = chars.by_ref().find(|(at, _)| *at == wanted as usize) else {
            break;
        };
        let end = start + c.len_utf8();
        match ranges.last_mut() {
            Some(last) if last.end == start => last.end = end,
            _ => ranges.push(start..end),
        }
    }
    ranges
}

/// The folder a typed separator goes into: the one named exactly like the
/// query, or else the best match. `None` when the query is empty or
/// nothing matches.
pub(crate) fn resolve_descend(
    entries: &[FolderEntry],
    matches: &[Match],
    query: &str,
) -> Option<SharedString> {
    if query.is_empty() {
        return None;
    }
    entries
        .iter()
        .find(|entry| entry.name().as_ref() == query)
        .or_else(|| matches.first().and_then(|found| entries.get(found.index)))
        .map(|entry| entry.name().clone())
}

/// Sorts a listing as the picker shows it: the visible folders first, then
/// the hidden ones, each in alphabetical order without regard to case.
pub(crate) fn sort_entries(entries: &mut [FolderEntry]) {
    entries.sort_by(|a, b| {
        a.hidden()
            .cmp(&b.hidden())
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
            .then_with(|| a.name().cmp(b.name()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(names: &[&str]) -> Vec<FolderEntry> {
        names.iter().map(|name| FolderEntry::new(*name)).collect()
    }

    fn names(entries: &[FolderEntry], matches: &[Match]) -> Vec<String> {
        matches
            .iter()
            .map(|found| entries[found.index].name().to_string())
            .collect()
    }

    #[test]
    fn a_path_splits_at_its_last_separator() {
        let split = |text: &str| split_path(text, false);
        assert_eq!(split("/home/me/co"), ("/home/me/".into(), "co".into()));
        assert_eq!(split("/home/me/"), ("/home/me/".into(), "".into()));
        assert_eq!(split("/"), ("/".into(), "".into()));
        assert_eq!(split("/abc"), ("/".into(), "abc".into()));
        assert_eq!(split("abc"), ("/".into(), "abc".into()));
        assert_eq!(split(""), ("/".into(), "".into()));
    }

    #[test]
    fn windows_paths_split_at_either_separator() {
        assert_eq!(
            split_path("C:\\Users\\me/co", true),
            ("C:\\Users\\me/".into(), "co".into())
        );
        assert_eq!(split_path("C:\\", true), ("C:\\".into(), "".into()));
        assert_eq!(split_path("a\\b", false), ("/".into(), "a\\b".into()));
    }

    #[test]
    fn the_parent_keeps_a_trailing_separator_and_the_root_stays() {
        assert_eq!(parent_text("/home/me/", false), "/home/");
        assert_eq!(parent_text("/home/", false), "/");
        assert_eq!(parent_text("/", false), "/");
        assert_eq!(parent_text("C:\\Users\\", true), "C:\\");
        assert_eq!(parent_text("C:\\", true), "C:\\");
    }

    #[test]
    fn a_folder_joins_with_separators_on_both_sides() {
        assert_eq!(join_dir("/home/", "code", false), "/home/code/");
        assert_eq!(join_dir("/", ".config", false), "/.config/");
        assert_eq!(join_dir("C:\\", "Users", true), "C:\\Users\\");
    }

    #[test]
    fn a_directory_path_drops_the_trailing_separator_except_at_the_root() {
        assert_eq!(
            directory_path("/home/me/", false),
            PathBuf::from("/home/me")
        );
        assert_eq!(directory_path("/", false), PathBuf::from("/"));
        assert_eq!(directory_path("C:\\", true), PathBuf::from("C:\\"));
        assert_eq!(
            directory_path("C:\\Users\\", true),
            PathBuf::from("C:\\Users")
        );
    }

    #[test]
    fn a_query_ranks_a_subsequence_and_marks_the_matched_characters() {
        let all = entries(&["code", ".cache", ".codex", ".config", "docs"]);
        let found = filter(&all, "co");
        assert_eq!(names(&all, &found), ["code", ".codex", ".config"]);
        let code = found.iter().find(|m| m.index == 0).unwrap();
        assert_eq!(code.ranges, vec![0..2]);
        let found = filter(&all, "cx");
        assert_eq!(names(&all, &found), [".codex"]);
        assert_eq!(found[0].ranges, vec![1..2, 5..6]);
    }

    #[test]
    fn matching_ignores_case_and_leaves_out_what_does_not_match() {
        let all = entries(&["Documents", "Downloads", "music"]);
        assert_eq!(names(&all, &filter(&all, "DOC")), ["Documents"]);
        assert!(filter(&all, "zzz").is_empty());
    }

    #[test]
    fn equal_scores_go_to_the_shorter_name_then_the_listing_order() {
        let all = entries(&["a1", "a2", "a3", "a"]);
        assert_eq!(names(&all, &filter(&all, "a")), ["a", "a1", "a2", "a3"]);
    }

    #[test]
    fn multibyte_names_give_byte_ranges() {
        let all = entries(&["\u{e9}t\u{e9}"]);
        let found = filter(&all, "t");
        assert_eq!(found[0].ranges, vec![2..3]);
    }

    #[test]
    fn a_separator_goes_into_the_exact_folder_or_else_the_best_match() {
        let all = entries(&["code", "codex", "docs"]);
        let found = filter(&all, "code");
        assert_eq!(
            resolve_descend(&all, &found, "code").as_deref(),
            Some("code")
        );
        let found = filter(&all, "cdx");
        assert_eq!(
            resolve_descend(&all, &found, "cdx").as_deref(),
            Some("codex")
        );
        let found = filter(&all, "zzz");
        assert_eq!(resolve_descend(&all, &found, "zzz"), None);
        assert_eq!(resolve_descend(&all, &filter(&all, ""), ""), None);
    }

    #[test]
    fn an_exact_name_wins_over_a_better_ranked_match() {
        let all = entries(&["codex", "co"]);
        let found = filter(&all, "co");
        assert_eq!(names(&all, &found)[0], "co");
        let all = entries(&["cooking", "co"]);
        let found = filter(&all, "co");
        assert_eq!(resolve_descend(&all, &found, "co").as_deref(), Some("co"));
    }

    #[test]
    fn a_listing_sorts_visible_folders_first_then_hidden_ones() {
        let mut all = entries(&["b", ".zed", "A", ".Cache", "code"]);
        sort_entries(&mut all);
        let sorted: Vec<_> = all.iter().map(|entry| entry.name().to_string()).collect();
        assert_eq!(sorted, ["A", "b", "code", ".Cache", ".zed"]);
    }
}
