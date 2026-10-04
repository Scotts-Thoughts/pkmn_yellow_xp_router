//! Where the movepool drew things last frame, for tests (the page's widgets
//! have no other way to be found headlessly). Off unless a test calls
//! [`enable`]; thread-local, so parallel tests do not see each other.

use std::cell::{Cell, RefCell};

use egui::Rect;

/// One thing the movepool drew.
#[derive(Clone, Debug)]
pub struct Probe {
    /// `"row"`, `"th"` (header cell), `"label"` (section label), `"chip"`
    /// (coverage move chip), `"clear"` (coverage Clear all)
    pub kind: &'static str,
    /// table key: `level`, `tutor`, `egg`, `transfer`, `priorevo`, `tmhm`, ...
    pub section: String,
    /// row index in display order, or the header column
    pub index: usize,
    /// move name, header label, ...
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

pub(super) fn record(kind: &'static str, section: &str, index: usize, text: &str, rect: Rect) {
    if ON.with(|o| o.get()) {
        ITEMS.with(|i| {
            i.borrow_mut().push(Probe {
                kind,
                section: section.to_string(),
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

/// The rect of the row of `move_name` in table `section`.
pub fn row(section: &str, move_name: &str) -> Option<Rect> {
    items()
        .into_iter()
        .find(|p| p.kind == "row" && p.section == section && p.text == move_name)
        .map(|p| p.rect)
}

/// The rect of header column `col` of table `section`.
pub fn header(section: &str, col: usize) -> Option<Rect> {
    items()
        .into_iter()
        .find(|p| p.kind == "th" && p.section == section && p.index == col)
        .map(|p| p.rect)
}

/// The rect of the first item of `kind` and `text`.
pub fn find(kind: &str, text: &str) -> Option<Rect> {
    items()
        .into_iter()
        .find(|p| p.kind == kind && p.text == text)
        .map(|p| p.rect)
}

/// Move names of a table's rows in display order.
pub fn row_order(section: &str) -> Vec<String> {
    let mut rows: Vec<Probe> = items()
        .into_iter()
        .filter(|p| p.kind == "row" && p.section == section)
        .collect();
    rows.sort_by_key(|p| p.index);
    rows.into_iter().map(|p| p.text).collect()
}
