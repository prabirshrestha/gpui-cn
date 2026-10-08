//! Helpers shared by the engine tests.

#![allow(dead_code, reason = "each test binary uses a subset of the helpers")]

use ghostty_vt::render::{CellIterator, RenderState, RowIterator};
use ghostty_vt::{Terminal, TerminalOptions};

/// A terminal with the given size and no scrollback limit surprises.
pub fn terminal(cols: u16, rows: u16) -> Terminal {
    Terminal::new(TerminalOptions {
        cols,
        rows,
        ..Default::default()
    })
    .expect("terminal")
}

/// The visible rows of the terminal as trimmed strings, read through the
/// render state API.
pub fn rows(terminal: &Terminal) -> Vec<String> {
    let mut state = RenderState::new().expect("render state");
    let mut row_iter = RowIterator::new().expect("row iterator");
    let mut cell_iter = CellIterator::new().expect("cell iterator");
    let snapshot = state.update(terminal).expect("update");
    let mut out = Vec::new();
    let mut rows = row_iter.update(&snapshot).expect("rows");
    while let Some(row) = rows.next() {
        let mut text = String::new();
        let mut cells = cell_iter.update(row).expect("cells");
        while let Some(cell) = cells.next() {
            let graphemes = cell.graphemes().expect("graphemes");
            if graphemes.is_empty() {
                text.push(' ');
            } else {
                text.extend(graphemes);
            }
        }
        out.push(text.trim_end().to_owned());
    }
    out
}

/// The visible rows as one string with `\n` between rows and trailing
/// blank rows removed.
pub fn screen(terminal: &Terminal) -> String {
    let mut rows = rows(terminal);
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    rows.join("\n")
}
