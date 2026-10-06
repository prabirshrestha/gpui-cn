//! The pure logic the folder and file pickers share: how the path text splits into a
//! directory and a query, how a query ranks the folders, and which folder
//! a typed separator goes into. It works on plain strings, so it needs no
//! window and no file system.

use std::path::{Path, PathBuf};

use super::Entry;
use gpui_kit::SharedString;

pub(crate) use crate::fuzzy::Match;

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

/// Whether the directory text starts at the home folder: `~` alone, or
/// `~` and a separator.
pub(crate) fn is_home_text(dir: &str, windows: bool) -> bool {
    dir == "~"
        || dir
            .strip_prefix('~')
            .is_some_and(|rest| rest.starts_with(|c| is_separator(c, windows)))
}

/// The path a directory's text names, with a leading `~` standing for
/// `home` when there is one.
pub(crate) fn resolve_dir(dir: &str, home: Option<&Path>, windows: bool) -> PathBuf {
    match home {
        Some(home) if is_home_text(dir, windows) => {
            let rest = dir[1..].trim_matches(|c| is_separator(c, windows));
            if rest.is_empty() {
                home.to_path_buf()
            } else {
                home.join(rest)
            }
        }
        _ => directory_path(dir, windows),
    }
}

/// The text of a directory under `home` in the `~/...` form, with a
/// trailing separator, and `text` itself when it is not under `home`.
pub(crate) fn collapse_home(text: &str, home: Option<&Path>, windows: bool) -> String {
    let Some(home) = home else {
        return text.to_string();
    };
    let home = home.to_string_lossy();
    let home = home.trim_end_matches(|c| is_separator(c, windows));
    if home.is_empty() {
        return text.to_string();
    }
    let sep = separator(windows);
    let trimmed = text.trim_end_matches(|c| is_separator(c, windows));
    if trimmed == home {
        return format!("~{sep}");
    }
    match text.strip_prefix(home) {
        Some(rest) if rest.starts_with(|c| is_separator(c, windows)) => format!("~{rest}"),
        _ => text.to_string(),
    }
}

/// Ranks `entries` by how well `query` matches their names: a subsequence
/// of the name, ignoring case, scored so that runs and word starts rank
/// first. Ties go to the shorter name, then keep the listing's order. The folders that do not match
/// are left out.
#[cfg(test)]
pub(crate) fn filter<E: Entry>(entries: &[E], query: &str) -> Vec<Match> {
    rank(entries, &|_| true, query)
}

/// Every visible entry in the listing's order, with no matched ranges: what
/// an empty query shows.
pub(crate) fn all_visible<E: Entry>(entries: &[E], visible: &dyn Fn(&E) -> bool) -> Vec<Match> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| visible(entry))
        .map(|(index, _)| Match {
            index,
            score: 0,
            ranges: Vec::new(),
        })
        .collect()
}

/// [`filter`] over the entries `visible` lets through, ranked by
/// [`crate::fuzzy::rank`] on their names.
pub(crate) fn rank<E: Entry>(
    entries: &[E],
    visible: &dyn Fn(&E) -> bool,
    query: &str,
) -> Vec<Match> {
    let shown: Vec<(usize, &E)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| visible(entry))
        .collect();
    crate::fuzzy::rank(query, &shown, |(_, entry)| entry.name())
        .into_iter()
        .map(|found| Match {
            index: shown[found.index].0,
            ..found
        })
        .collect()
}

/// The folder a typed separator goes into: the one named exactly like the
/// query, or else the best match. `None` when the query is empty or
/// nothing matches.
pub(crate) fn resolve_descend<E: Entry>(
    entries: &[E],
    matches: &[Match],
    query: &str,
) -> Option<SharedString> {
    if query.is_empty() {
        return None;
    }
    entries
        .iter()
        .find(|entry| entry.is_folder() && entry.name().as_ref() == query)
        .or_else(|| {
            matches
                .iter()
                .filter_map(|found| entries.get(found.index))
                .find(|entry| entry.is_folder())
        })
        .map(|entry| entry.name().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestEntry(SharedString);

    impl Entry for TestEntry {
        fn name(&self) -> &SharedString {
            &self.0
        }

        fn hidden(&self) -> bool {
            self.0.starts_with('.')
        }

        fn is_folder(&self) -> bool {
            true
        }
    }

    fn entries(names: &[&str]) -> Vec<TestEntry> {
        names
            .iter()
            .map(|name| TestEntry(SharedString::from(name.to_string())))
            .collect()
    }

    fn names(entries: &[TestEntry], matches: &[Match]) -> Vec<String> {
        matches
            .iter()
            .map(|found| entries[found.index].name().to_string())
            .collect()
    }

    #[test]
    fn a_leading_tilde_names_the_home_when_there_is_one() {
        let home = Some(Path::new("/Users/me"));
        let at = |text: &str, home| resolve_dir(text, home, false);
        assert_eq!(at("~/", home), PathBuf::from("/Users/me"));
        assert_eq!(at("~", home), PathBuf::from("/Users/me"));
        assert_eq!(at("~/code/", home), PathBuf::from("/Users/me/code"));
        assert_eq!(at("~/", None), PathBuf::from("~"), "no home: a plain name");
        assert_eq!(at("/~/", home), PathBuf::from("/~"), "only a leading tilde");
        assert!(!is_home_text("~me/", false));
    }

    #[test]
    fn a_directory_under_home_takes_the_tilde_form() {
        let home = Some(Path::new("/Users/me"));
        let at = |text: &str| collapse_home(text, home, false);
        assert_eq!(at("/Users/me/"), "~/");
        assert_eq!(at("/Users/me/code/"), "~/code/");
        assert_eq!(at("/Users/"), "/Users/");
        assert_eq!(at("/Users/meow/"), "/Users/meow/", "not a child of home");
        assert_eq!(collapse_home("/Users/me/", None, false), "/Users/me/");
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
}
