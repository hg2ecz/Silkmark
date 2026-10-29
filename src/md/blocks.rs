//! Display blocks: the unit the reader widgets are built from.
//!
//! Everything here is plain data derived from `Document::lines` right after parsing, so it is
//! computed on the worker thread and the UI thread only looks things up.
use super::*;
use std::collections::HashMap;

/// Separates table cells in `BlockIndex::search_text`; cannot occur in rendered text.
const CELL_SEPARATOR: char = '\u{1}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    /// A single line of `Document::lines`.
    Line(usize),
    /// A whole table: header row, separator and body rows are `lines[first..end]`.
    Table { first: usize, end: usize },
}

#[derive(Clone, Default)]
pub struct BlockIndex {
    pub blocks: Vec<Block>,
    /// Lower-cased plain text of every block; table cells are joined with `CELL_SEPARATOR`.
    pub search_text: Vec<String>,
    /// Heading and footnote anchors mapped to the block that holds them.
    pub anchors: HashMap<String, usize>,
}

impl BlockIndex {
    pub fn build(lines: &[Line]) -> Self {
        let mut index = BlockIndex::default();
        let mut i = 0;
        while i < lines.len() {
            if matches!(lines[i].style, Style::TableRow) && lines.get(i + 1).is_some_and(|l| matches!(l.style, Style::TableSep)) {
                let mut end = i + 2;
                while end < lines.len() && matches!(lines[end].style, Style::TableRow) {
                    end += 1;
                }
                index.push(Block::Table { first: i, end }, table_search_text(&lines[i..end]), &lines[i..end]);
                i = end;
                continue;
            }
            let line = &lines[i];
            // Lines the renderer cannot show produce no block at all.
            let renderable = match line.style {
                Style::Image => line.image.is_some(),
                Style::Code => line.code.is_some(),
                Style::Diagram => line.diagram.is_some(),
                _ => true,
            };
            if renderable {
                index.push(Block::Line(i), line_search_text(line), std::slice::from_ref(line));
            }
            i += 1;
        }
        index
    }

    fn push(&mut self, block: Block, text: String, lines: &[Line]) {
        let id = self.blocks.len();
        for line in lines {
            for anchor in line.anchor.iter().chain(line.extra_anchors.iter()) {
                self.anchors.entry(anchor.clone()).or_insert(id);
            }
        }
        self.blocks.push(block);
        self.search_text.push(text);
    }
}

fn line_search_text(line: &Line) -> String {
    match line.style {
        Style::Rule | Style::Image => String::new(),
        Style::Code | Style::Diagram => line.code.as_ref().map(|b| b.text.to_lowercase()).unwrap_or_default(),
        _ => crate::strip_pango_markup(&line.markup).to_lowercase(),
    }
}

fn table_search_text(rows: &[Line]) -> String {
    let mut out = String::new();
    for row in rows.iter().filter(|r| matches!(r.style, Style::TableRow)) {
        for cell in row.table.iter().flat_map(|t| t.cells.iter()) {
            if !out.is_empty() {
                out.push(CELL_SEPARATOR);
            }
            out.push_str(&strip_inline_markup(cell).to_lowercase());
        }
    }
    out
}

/// Number of search hits in one block. A table counts each matching cell, every other
/// block counts at most once. `needle` must already be lower-case.
pub fn count_hits(text: &str, needle: &str, is_table: bool) -> usize {
    if needle.is_empty() {
        0
    } else if is_table {
        text.split(CELL_SEPARATOR).filter(|cell| cell.contains(needle)).count()
    } else {
        usize::from(text.contains(needle))
    }
}
