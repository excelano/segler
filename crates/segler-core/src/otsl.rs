//! The OTSL grid: a table's cell tokens read into rows and columns.
//!
//! A table body is a flat run of tokens. Each of `fcel`, `ecel`, `ched`,
//! `rhed`, `corn` and `srow` begins a cell, and whatever follows it until
//! the next token is that cell's content. `lcel` extends the cell to its
//! left by one column, `ucel` extends the cell above by one row, `xcel`
//! sits inside a region that spans both ways, and `nl` ends the row. The
//! grid is not in the XML; it is computed from the sequence, and every cell
//! remembers where its token and its content sit among the element's
//! children so that an edit can go back to the sequence.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::HashMap;
use std::ops::Range;

use crate::doclang::{self, Kind};
use crate::tree::{Element, Node};

/// What a cell is, from the token that begins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellKind {
    Full,
    Empty,
    ColumnHeader,
    RowHeader,
    Corner,
    SectionRow,
}

impl CellKind {
    pub fn from_kind(k: Kind) -> Option<CellKind> {
        Some(match k {
            Kind::Fcel => CellKind::Full,
            Kind::Ecel => CellKind::Empty,
            Kind::Ched => CellKind::ColumnHeader,
            Kind::Rhed => CellKind::RowHeader,
            Kind::Corn => CellKind::Corner,
            Kind::Srow => CellKind::SectionRow,
            _ => return None,
        })
    }

    /// The token element that begins a cell of this kind.
    pub fn token(self) -> Kind {
        match self {
            CellKind::Full => Kind::Fcel,
            CellKind::Empty => Kind::Ecel,
            CellKind::ColumnHeader => Kind::Ched,
            CellKind::RowHeader => Kind::Rhed,
            CellKind::Corner => Kind::Corn,
            CellKind::SectionRow => Kind::Srow,
        }
    }

    pub const ALL: [CellKind; 6] = [
        CellKind::Full,
        CellKind::Empty,
        CellKind::ColumnHeader,
        CellKind::RowHeader,
        CellKind::Corner,
        CellKind::SectionRow,
    ];

    /// Whether the cell is any kind of header.
    pub fn is_header(self) -> bool {
        matches!(
            self,
            CellKind::ColumnHeader | CellKind::RowHeader | CellKind::Corner | CellKind::SectionRow
        )
    }
}

/// A cell that begins with its own token. Merged positions belong to the
/// cell whose span covers them and have no `Cell` of their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
    pub kind: CellKind,
    pub rowspan: usize,
    pub colspan: usize,
    /// Index into the table element's `children()` of the cell's token.
    pub token: usize,
    /// Indices into `children()` of the cell's content: everything after
    /// the token up to the next token or row end.
    pub content: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Cell>,
    /// Which cell owns each position, by index into `cells`.
    owner: HashMap<(usize, usize), usize>,
}

impl Grid {
    /// Read the grid of a `table`, `index` or `tabular` element. Tolerant:
    /// an extension token with nothing to extend becomes an empty cell, so
    /// that an invalid table still shows, and the validator says why.
    pub fn parse(el: &Element) -> Grid {
        let children = el.children();
        let tokens: Vec<(usize, Kind)> = children
            .iter()
            .enumerate()
            .filter_map(|(i, n)| match n {
                Node::Element(e) => doclang::kind(e).filter(|k| is_token(*k)).map(|k| (i, k)),
                _ => None,
            })
            .collect();

        let mut grid = Grid::default();
        let (mut row, mut col) = (0usize, 0usize);
        for (t, (index, kind)) in tokens.iter().enumerate() {
            let content_end = tokens.get(t + 1).map_or(children.len(), |(next, _)| *next);
            match kind {
                Kind::Nl => {
                    grid.cols = grid.cols.max(col);
                    row += 1;
                    col = 0;
                    continue;
                }
                Kind::Lcel | Kind::Ucel | Kind::Xcel => {
                    let from = match kind {
                        Kind::Lcel => col.checked_sub(1).map(|c| (row, c)),
                        _ => row.checked_sub(1).map(|r| (r, col)),
                    };
                    match from.and_then(|pos| grid.owner.get(&pos).copied()) {
                        Some(i) => {
                            let cell = &mut grid.cells[i];
                            cell.colspan = cell.colspan.max(col + 1 - cell.col);
                            cell.rowspan = cell.rowspan.max(row + 1 - cell.row);
                            grid.owner.insert((row, col), i);
                        }
                        None => {
                            grid.push(row, col, CellKind::Empty, *index, *index + 1..content_end)
                        }
                    }
                }
                other => {
                    let ck = CellKind::from_kind(*other).expect("a cell-start token");
                    grid.push(row, col, ck, *index, *index + 1..content_end);
                }
            }
            col += 1;
        }
        if col > 0 {
            grid.cols = grid.cols.max(col);
            row += 1;
        }
        grid.rows = row;
        grid
    }

    fn push(
        &mut self,
        row: usize,
        col: usize,
        kind: CellKind,
        token: usize,
        content: Range<usize>,
    ) {
        self.owner.insert((row, col), self.cells.len());
        self.cells.push(Cell {
            row,
            col,
            kind,
            rowspan: 1,
            colspan: 1,
            token,
            content,
        });
    }

    /// The cell owning a position, whether it begins there or spans it.
    pub fn at(&self, row: usize, col: usize) -> Option<&Cell> {
        self.owner.get(&(row, col)).map(|i| &self.cells[*i])
    }

    /// Whether a position is the top-left of its cell.
    pub fn is_origin(&self, row: usize, col: usize) -> bool {
        self.at(row, col)
            .is_some_and(|c| c.row == row && c.col == col)
    }
}

pub fn is_token(k: Kind) -> bool {
    k.category() == doclang::Category::Structural && k != Kind::Ldiv
}

/// Where a cell's body starts: after any property elements that open its
/// virtual text head.
pub fn body_start(el: &Element, cell: &Cell) -> usize {
    let children = el.children();
    let mut i = cell.content.start;
    while i < cell.content.end {
        match &children[i] {
            Node::Element(e) if doclang::kind(e).is_some_and(Kind::is_property) => i += 1,
            Node::Text(t) if t.value().trim().is_empty() => i += 1,
            _ => break,
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Document;

    fn grid(body: &str) -> Grid {
        let doc = Document::parse(&format!("<doclang><table>{body}</table></doclang>")).unwrap();
        let grid = Grid::parse(doc.root.child_elements().next().unwrap());
        grid
    }

    #[test]
    fn a_plain_grid() {
        let g = grid("<ched/>A<ched/>B<nl/><fcel/>1<fcel/>2<nl/>");
        assert_eq!((g.rows, g.cols, g.cells.len()), (2, 2, 4));
        assert_eq!(g.at(0, 1).unwrap().kind, CellKind::ColumnHeader);
        assert_eq!(g.at(1, 0).unwrap().kind, CellKind::Full);
        assert!(g.at(2, 0).is_none());
    }

    #[test]
    fn spans_from_extension_tokens() {
        // Row 0: a header spanning two columns. Row 1: a cell spanning two
        // rows in column 0, and two cells beside it.
        let g = grid("<ched/>Wide<lcel/><nl/><fcel/>Tall<fcel/>b<nl/><ucel/><fcel/>c<nl/>");
        assert_eq!((g.rows, g.cols), (3, 2));
        let wide = g.at(0, 0).unwrap();
        assert_eq!((wide.colspan, wide.rowspan), (2, 1));
        assert_eq!(g.at(0, 1).unwrap().col, 0);
        let tall = g.at(1, 0).unwrap();
        assert_eq!((tall.colspan, tall.rowspan), (1, 2));
        assert!(!g.is_origin(2, 0));
        assert!(g.is_origin(2, 1));
        assert_eq!(g.cells.len(), 4);
    }

    #[test]
    fn a_cross_span_and_a_trailing_row() {
        let g = grid("<fcel/>A<lcel/><fcel/>B<nl/><ucel/><xcel/><fcel/>C");
        assert_eq!((g.rows, g.cols), (2, 3));
        let a = g.at(0, 0).unwrap();
        assert_eq!((a.colspan, a.rowspan), (2, 2));
        assert_eq!(g.at(1, 1).unwrap().col, 0);
        assert_eq!(g.at(1, 2).unwrap().kind, CellKind::Full);
    }

    #[test]
    fn an_extension_with_nothing_to_extend_is_an_empty_cell() {
        let g = grid("<lcel/><fcel/>x<nl/>");
        assert_eq!(g.at(0, 0).unwrap().kind, CellKind::Empty);
        assert_eq!(g.cells.len(), 2);
    }

    #[test]
    fn content_ranges_and_body_start() {
        let doc = Document::parse("<doclang><table><fcel/><location value=\"1\"/><location value=\"2\"/><location value=\"3\"/><location value=\"4\"/>Header<nl/></table></doclang>").unwrap();
        let table = doc.root.child_elements().next().unwrap();
        let g = Grid::parse(table);
        let cell = g.at(0, 0).unwrap();
        assert_eq!(cell.token, 0);
        assert_eq!(cell.content, 1..6);
        assert_eq!(body_start(table, cell), 5);
    }
}
