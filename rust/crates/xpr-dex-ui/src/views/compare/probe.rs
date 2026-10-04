//! Where the comparison views drew things last frame, for tests (the page's
//! widgets have no other way to be found headlessly). Off unless a test
//! calls [`enable`]; thread-local, so parallel tests do not see each other.

use std::cell::{Cell, RefCell};

use egui::Rect;

/// One thing a comparison view drew.
#[derive(Clone, Debug)]
pub struct Probe {
    /// `"row"` (table row), `"th"` (table header cell), `"label"` (section
    /// label), `"stat"` (a stat row), `"evo"` (evolution chip), `"game"` (game
    /// chip), `"type"` (an effectiveness type), `"button"`, `"pop"` (a ranking
    /// popover row), `"poptitle"` (a ranking popover's title)
    pub kind: &'static str,
    /// where: `left` / `right` / `a` / `b` / `c` plus the table key
    /// (`left:level`), or the stat key
    pub scope: String,
    /// row index in display order, or the header column
    pub index: usize,
    /// move name, stat label, species, ...
    pub text: String,
    pub rect: Rect,
}

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static ITEMS: RefCell<Vec<Probe>> = const { RefCell::new(Vec::new()) };
}

/// Start recording on this thread.
pub fn enable() {
    ON.with(|o| o.set(true));
}

pub(super) fn begin_frame() {
    if ON.with(|o| o.get()) {
        ITEMS.with(|i| i.borrow_mut().clear());
    }
}

pub(super) fn record(kind: &'static str, scope: &str, index: usize, text: &str, rect: Rect) {
    if ON.with(|o| o.get()) {
        ITEMS.with(|i| {
            i.borrow_mut().push(Probe {
                kind,
                scope: scope.to_string(),
                index,
                text: text.to_string(),
                rect,
            })
        });
    }
}

/// Everything drawn in the last frame.
pub fn items() -> Vec<Probe> {
    ITEMS.with(|i| i.borrow().clone())
}

/// The first item of `kind` in `scope` with `text`.
pub fn find(kind: &str, scope: &str, text: &str) -> Option<Rect> {
    items()
        .into_iter()
        .find(|p| p.kind == kind && p.scope == scope && p.text == text)
        .map(|p| p.rect)
}

/// The rect of the row of `move_name` in table `scope`.
pub fn row(scope: &str, move_name: &str) -> Option<Rect> {
    find("row", scope, move_name)
}

/// The rect of header column `col` of table `scope`.
pub fn header(scope: &str, col: usize) -> Option<Rect> {
    items()
        .into_iter()
        .find(|p| p.kind == "th" && p.scope == scope && p.index == col)
        .map(|p| p.rect)
}

/// Move names of a table's rows in display order.
pub fn row_order(scope: &str) -> Vec<String> {
    let mut rows: Vec<Probe> = items()
        .into_iter()
        .filter(|p| p.kind == "row" && p.scope == scope)
        .collect();
    rows.sort_by_key(|p| p.index);
    rows.into_iter().map(|p| p.text).collect()
}

/// The first item of `kind` with `text`, in any scope.
pub fn any(kind: &str, text: &str) -> Option<Rect> {
    items()
        .into_iter()
        .find(|p| p.kind == kind && p.text == text)
        .map(|p| p.rect)
}
