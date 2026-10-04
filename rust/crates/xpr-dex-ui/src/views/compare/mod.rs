//! Comparisons: two species in one game, one species across two games, and
//! three species. Port of Solodex `ComparisonView.tsx`,
//! `SelfComparisonView.tsx`, `TripleComparisonView.tsx`.
//!
//! The header (identity, defensive type matchups, stat rows with their
//! ranking popovers) and the movepool tables are drawn here; the table rows
//! are the movepool module's [`move_row`](crate::views::movepool::move_row),
//! so cross-outs, the right-click test set and the TM / tutor popovers work
//! as in the Pokédex. Not ported: Solodex's PNG export buttons and
//! export-mode toggle, and the Wiki popovers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::scroll_area::ScrollSource;
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};
use xpr_dex::UserBans;

use crate::palette;
use crate::state::DexSettings;
use crate::views::movepool::{crossed_out_moves, MovepoolView, SortState};
use crate::widgets::text_w;
use crate::DexCx;

mod draw;
mod header;
mod pair;
pub mod probe;
mod rank;
mod selfcmp;
mod stats;
mod tables;
mod triple;
mod two;

/// Scroll areas scroll by wheel and bar only: a drag would also scroll while
/// right-clicking a row.
pub(super) const NO_DRAG: ScrollSource = ScrollSource {
    drag: false,
    ..ScrollSource::ALL
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Pair,
    SelfCmp,
    Triple,
}

struct CrossCache {
    flags: (bool, bool, bool),
    bans: UserBans,
    set: Arc<HashSet<String>>,
}

#[derive(Default)]
pub struct CompareView {
    mode: Option<Mode>,
    last_frame: u64,
    /// bumped whenever the view is mounted anew (it keys the scroll areas, so
    /// they start at the top again, as Solodex's remounted components do)
    mount: u64,
    /// the sort of each of the five tables, shared by every column
    sorts: [SortState; 5],
    pair: Option<pair::Model>,
    selfcmp: Option<selfcmp::Model>,
    triple: Option<triple::Model>,
    /// the table whose spreadsheet was just copied, and until when the hint says so
    copied: Option<(String, f64)>,
    cross: HashMap<String, CrossCache>,
}

impl CompareView {
    /// `cx.state.selected()` vs `cx.state.settings.comparing_with` in `cx.state.game()`.
    pub fn pair_ui(&mut self, ui: &mut Ui, cx: &mut DexCx, movepool: &mut MovepoolView) {
        let _ = movepool;
        pair::ui(self, ui, cx);
    }

    /// The selected species across two games (`cx.state.self_compare_right_game`).
    pub fn self_ui(&mut self, ui: &mut Ui, cx: &mut DexCx, movepool: &mut MovepoolView) {
        let _ = movepool;
        selfcmp::ui(self, ui, cx);
    }

    /// Selected vs comparing_with vs comparing_third.
    pub fn triple_ui(&mut self, ui: &mut Ui, cx: &mut DexCx, movepool: &mut MovepoolView) {
        let _ = movepool;
        triple::ui(self, ui, cx);
    }

    /// Start of a frame of `mode`: tables sort in their own order again when
    /// the view was not drawn last frame (Solodex remounts the component).
    fn begin(&mut self, mode: Mode, ctx: &egui::Context) {
        probe::begin_frame();
        let frame = ctx.cumulative_frame_nr();
        if self.mode != Some(mode) || frame > self.last_frame + 1 {
            self.sorts = Default::default();
            self.mode = Some(mode);
            self.copied = None;
            self.mount += 1;
        }
        self.last_frame = frame;
    }

    /// The moves to cross out in `game` under the cross-out settings.
    fn cross_set(&mut self, settings: &DexSettings, game: &str) -> Arc<HashSet<String>> {
        let flags = (
            settings.cross_out_banned,
            settings.cross_out_postgame,
            settings.cross_out_conditional,
        );
        if let Some(c) = self.cross.get(game) {
            if c.flags == flags && c.bans == settings.user_bans {
                return c.set.clone();
            }
        }
        let set = Arc::new(crossed_out_moves(settings, game));
        self.cross.insert(
            game.to_string(),
            CrossCache {
                flags,
                bans: settings.user_bans.clone(),
                set: set.clone(),
            },
        );
        set
    }
}

/// The distinct entries of `v`, in order (`[...new Set(abilities)]`).
fn dedup(v: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    v.iter().filter(|a| seen.insert((*a).clone())).cloned().collect()
}

/// A species is missing from the game: Solodex's gray message with an exit
/// button. Returns true when the button was clicked.
fn unavailable(ui: &mut Ui, cx: &mut DexCx, text: &str, button: &str, hint: Option<&str>) -> bool {
    let theme = cx.theme;
    let rect = ui.available_rect_before_wrap();
    let font = palette::px(theme, 16.0);
    let small = palette::px(theme, 14.0);
    let tw = text_w(ui, text, &font);
    let bw = text_w(ui, button, &small) + hint.map(|h| 4.0 + text_w(ui, h, &small)).unwrap_or(0.0);
    let btn_w = bw + 24.0;
    let total = tw + 16.0 + btn_w;
    let x = rect.center().x - total / 2.0;
    ui.painter().text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        palette::GRAY_500,
    );
    let btn = Rect::from_min_size(
        Pos2::new(x + tw + 16.0, rect.center().y - 14.0),
        Vec2::new(btn_w, 28.0),
    );
    let resp = ui
        .interact(btn, ui.id().with("cmp_unavailable_exit"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.painter().rect_filled(
        btn,
        egui::CornerRadius::same(4),
        if resp.hovered() {
            palette::GRAY_600
        } else {
            palette::GRAY_700
        },
    );
    ui.painter().text(
        Pos2::new(btn.min.x + 12.0, btn.center().y),
        Align2::LEFT_CENTER,
        button,
        small.clone(),
        palette::GRAY_500,
    );
    if let Some(h) = hint {
        ui.painter().text(
            Pos2::new(btn.min.x + 12.0 + text_w(ui, button, &small) + 4.0, btn.center().y),
            Align2::LEFT_CENTER,
            h,
            small,
            palette::GRAY_500,
        );
    }
    probe::record("button", "unavailable", 0, button, btn);
    ui.allocate_rect(rect, Sense::hover());
    resp.clicked()
}
