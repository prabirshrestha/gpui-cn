//! The viewport selection model.
//!
//! Adapted from tt v2 (Apache-2.0), crates/desktop/src/terminal/selection.rs,
//! itself derived from Herdr.

use crate::terminal::frame::{CellSelection, TerminalFrame};

/// A cell position in the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    /// The row.
    pub row: u16,
    /// The column.
    pub column: u16,
}

impl Cell {
    /// A cell at `column`, `row`.
    #[must_use]
    pub fn new(column: u16, row: u16) -> Self {
        Self { row, column }
    }
}

/// A selection anchored to the content it was made on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    anchor: Cell,
    head: Cell,
    revision: u64,
    rectangle: bool,
    dragging: bool,
}

impl Selection {
    /// A selection starting at `anchor` on frame content `revision`.
    #[must_use]
    pub fn new(anchor: Cell, revision: u64, rectangle: bool) -> Self {
        Self {
            anchor,
            head: anchor,
            revision,
            rectangle,
            dragging: true,
        }
    }

    /// Move the head.
    pub fn extend(&mut self, head: Cell) {
        self.head = head;
    }

    /// Whether the pointer is still down.
    #[must_use]
    pub fn is_dragging(self) -> bool {
        self.dragging
    }

    /// The pointer went up.
    pub fn finish(&mut self) {
        self.dragging = false;
    }

    /// Keep the selection across a new frame when the selected rows did not
    /// change, so streaming output on another row does not clear it.
    #[must_use]
    pub fn refresh(mut self, previous: &TerminalFrame, next: &TerminalFrame) -> Option<Self> {
        if previous.viewport != next.viewport
            || previous.scroll != next.scroll
            || previous.modes.alternate_screen != next.modes.alternate_screen
        {
            return None;
        }
        if previous.content_revision != next.content_revision {
            let (start, end) = self.ordered();
            for index in start.row..=end.row {
                let old = previous.rows.get(usize::from(index))?;
                let new = next.rows.get(usize::from(index))?;
                if old.text != new.text
                    || old.cells != new.cells
                    || old.soft_wrapped != new.soft_wrapped
                {
                    return None;
                }
            }
        }
        self.revision = next.content_revision;
        Some(self)
    }

    /// True when the drag never left its starting cell.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.anchor == self.head
    }

    fn ordered(self) -> (Cell, Cell) {
        if self.head < self.anchor {
            (self.head, self.anchor)
        } else {
            (self.anchor, self.head)
        }
    }

    /// The selected columns on one row; empty when the row is outside.
    #[must_use]
    pub fn columns(self, row: u16, columns: u16) -> std::ops::Range<u16> {
        let (start, end) = self.ordered();
        if row < start.row || row > end.row {
            return 0..0;
        }
        if self.rectangle {
            let left = start.column.min(end.column);
            let right = start.column.max(end.column);
            return left..right.min(columns);
        }
        let left = if row == start.row { start.column } else { 0 };
        let right = if row == end.row { end.column } else { columns };
        left..right.min(columns)
    }

    /// The selection as inclusive cells for the engine.
    #[must_use]
    pub fn to_cells(self) -> CellSelection {
        let (start, end) = self.ordered();
        CellSelection::new(
            (start.column, start.row),
            (end.column.saturating_sub(1), end.row),
            self.rectangle,
        )
    }

    /// The selected text from the visible rows of a frame.
    ///
    /// Cells carry their own byte range in the row text, so a selection that
    /// cuts across wide characters copies whole graphemes.
    #[must_use]
    pub fn text(self, frame: &TerminalFrame) -> String {
        let mut lines: Vec<String> = Vec::new();
        for (index, row) in frame.rows.iter().enumerate() {
            let Ok(row_index) = u16::try_from(index) else {
                break;
            };
            let columns = self.columns(row_index, frame.viewport.columns());
            if columns.is_empty() {
                continue;
            }
            let mut line = String::new();
            for cell in row.cells.iter() {
                if cell.spacer || !columns.contains(&cell.column) {
                    continue;
                }
                if let Some(text) = row.text.get(cell.text.clone()) {
                    line.push_str(text);
                }
            }
            lines.push(line);
        }
        lines.join("\n")
    }
}

/// Trim trailing whitespace from every line.
pub(crate) fn trim_lines(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_without_movement_selects_nothing() {
        let selection = Selection::new(Cell::new(3, 1), 7, false);
        assert!(selection.is_empty());
        assert!(selection.columns(1, 80).is_empty());
    }

    #[test]
    fn a_multi_row_drag_selects_to_the_row_edges_in_either_direction() {
        let mut forward = Selection::new(Cell::new(5, 1), 7, false);
        forward.extend(Cell::new(2, 3));
        assert_eq!(forward.columns(0, 80), 0..0);
        assert_eq!(forward.columns(1, 80), 5..80);
        assert_eq!(forward.columns(2, 80), 0..80);
        assert_eq!(forward.columns(3, 80), 0..2);
        let mut backward = Selection::new(Cell::new(2, 3), 7, false);
        backward.extend(Cell::new(5, 1));
        for row in 0..5 {
            assert_eq!(forward.columns(row, 80), backward.columns(row, 80));
        }
    }

    #[test]
    fn a_rectangular_drag_uses_the_same_columns_on_every_row() {
        let mut selection = Selection::new(Cell::new(5, 1), 7, true);
        selection.extend(Cell::new(2, 3));
        for row in 1..=3 {
            assert_eq!(selection.columns(row, 80), 2..5);
        }
        assert_eq!(selection.columns(4, 80), 0..0);
    }

    #[test]
    fn the_engine_selection_is_ordered_and_inclusive() {
        let mut selection = Selection::new(Cell::new(5, 3), 42, false);
        selection.extend(Cell::new(2, 1));
        let cells = selection.to_cells();
        assert_eq!(cells.start, (2, 1));
        assert_eq!(cells.end, (4, 3));
    }
}
