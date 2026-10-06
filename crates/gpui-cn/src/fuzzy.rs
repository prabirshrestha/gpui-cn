//! The fuzzy matcher every search in gpui-cn uses: the folder and file
//! pickers, the model picker, the searchable status lists, and the palettes.
//!
//! A query matches an item when its characters are a subsequence of the
//! item's text, ignoring case and accents. Matches that run together or start words score
//! higher. [`rank`] returns the matches best first, with the byte ranges of
//! the characters the query matched, so a row can mark them.
//!
//! The module is public as a small utility, so an application's own search
//! box can rank like the library's.

use std::ops::Range;

use gpui_kit::{FontWeight, HighlightStyle, Hsla, SharedString, StyledText};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{Atom, AtomKind, CaseMatching, Normalization},
};

/// An item that matches the query, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    /// The index of the item in the list that was ranked.
    pub index: usize,
    /// The score: a higher one ranks first.
    pub score: u32,
    /// The byte ranges of the item's text that the query matched. Empty
    /// for an empty query.
    pub ranges: Vec<Range<usize>>,
}

/// Ranks `items` by how well `query` matches `key(item)`, best first. An
/// empty query (or one of only spaces) returns every item in its own
/// order. Items that do not match are left out. Equal scores go to the
/// shorter text, then keep the list's order.
///
/// ```
/// use gpui_cn::fuzzy::rank;
///
/// let names = ["feature/composer", "main", "fix/popover"];
/// let found = rank("cmpsr", &names, |name| name);
/// assert_eq!(found.len(), 1);
/// assert_eq!(names[found[0].index], "feature/composer");
/// ```
pub fn rank<T>(query: &str, items: &[T], key: impl Fn(&T) -> &str) -> Vec<Match> {
    let query = query.trim();
    if query.is_empty() {
        return (0..items.len())
            .map(|index| Match {
                index,
                score: 0,
                ranges: Vec::new(),
            })
            .collect();
    }
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
    let mut matches: Vec<Match> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let text = key(item);
            indices.clear();
            let haystack = Utf32Str::new(text, &mut buffer);
            let score = atom.indices(haystack, &mut matcher, &mut indices)?;
            indices.sort_unstable();
            indices.dedup();
            Some(Match {
                index,
                score: u32::from(score),
                ranges: byte_ranges(text, &indices),
            })
        })
        .collect();
    matches.sort_by(|a, b| {
        let length = |found: &Match| key(&items[found.index]).chars().count();
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

/// `text` with the matched `ranges` in `color` and medium weight, for a row
/// that shows why it matched. No range gives plain text.
pub fn highlighted(text: SharedString, ranges: &[Range<usize>], color: Hsla) -> StyledText {
    StyledText::new(text).with_highlights(ranges.iter().cloned().map(|range| {
        (
            range,
            HighlightStyle {
                color: Some(color),
                font_weight: Some(FontWeight::MEDIUM),
                ..Default::default()
            },
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(query: &str, items: &[&str]) -> Vec<String> {
        rank(query, items, |item| item)
            .into_iter()
            .map(|m| items[m.index].to_string())
            .collect()
    }

    #[test]
    fn a_typo_tolerant_subsequence_finds_the_item() {
        assert_eq!(
            found("gpt56m", &["Opus 5.5", "GPT-5.6 Mini"]),
            ["GPT-5.6 Mini"]
        );
        assert_eq!(found("ops", &["GPT-5.6 Mini", "Opus 5.5"]), ["Opus 5.5"]);
        assert_eq!(
            found("cmpsr", &["main", "feature/composer"]),
            ["feature/composer"]
        );
    }

    #[test]
    fn an_exact_prefix_ranks_first() {
        assert_eq!(
            found("fea", &["a-fresh-edge-a", "feature", "unfeasible"])[0],
            "feature"
        );
    }

    #[test]
    fn an_empty_query_keeps_the_order_and_no_match_is_empty() {
        assert_eq!(found("", &["b", "a", "c"]), ["b", "a", "c"]);
        assert_eq!(found("  ", &["b", "a"]), ["b", "a"]);
        assert!(found("zzz", &["b", "a"]).is_empty());
    }

    #[test]
    fn matched_characters_come_back_as_byte_ranges() {
        let items = ["feature/composer"];
        let found = rank("cmp", &items, |item| item);
        assert_eq!(found[0].ranges, vec![8..9, 10..12]);
        let accented = ["\u{e9}t\u{e9}"];
        assert_eq!(rank("t", &accented, |item| item)[0].ranges, vec![2..3]);
    }

    #[test]
    fn case_does_not_matter() {
        assert_eq!(found("GPT", &["gpt-5", "other"]), ["gpt-5"]);
        assert_eq!(found("gpt", &["gpt-5", "GPT-5"]).len(), 2);
    }
}
