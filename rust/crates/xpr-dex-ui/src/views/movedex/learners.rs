//! The learner table: which species learn a move in a game and how, laid out
//! as virtual rows with wrapping method chips. Used by the list's right-hand
//! panel (Solodex's "Pokemon / Method" table) and by the move detail view.

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use super::data::{learners_of, Learner};
use super::kit::{self, tw, CHIP_H, RULE};
use crate::images::{self, SpriteSize};
use crate::palette;
use crate::widgets::text_w;
use crate::DexCx;

/// How the rows look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// draw the species sprite before the name (the detail view)
    pub sprite: bool,
    /// name font size in CSS px
    pub name_px: f32,
    /// width of the whole table
    pub width: f32,
}

struct RowLayout {
    chips: Vec<(Vec2, f32)>,
    /// top of the chip block inside the row (the block is centred)
    chips_y: f32,
    height: f32,
}

#[derive(Default)]
pub struct Learners {
    move_name: String,
    game: String,
    learners: Vec<Learner>,
    loaded: bool,
    style: Option<Style>,
    name_col: f32,
    rows: Vec<RowLayout>,
    offsets: Vec<f32>,
}

const PAD_Y: f32 = 4.0;

impl Learners {
    /// The species that learn the move (computed when the move or game
    /// changes).
    pub fn learners(&mut self, move_name: &str, game: &str) -> &[Learner] {
        if !self.loaded || self.move_name != move_name || self.game != game {
            self.move_name = move_name.to_string();
            self.game = game.to_string();
            self.learners = learners_of(move_name, game);
            self.loaded = true;
            self.style = None;
        }
        &self.learners
    }

    /// Width of the name column (valid after [`Learners::height`] / `show`).
    pub fn name_col(&self) -> f32 {
        self.name_col
    }

    /// Total height of the table at `style`.
    pub fn height(&mut self, ui: &Ui, theme: &Theme, style: Style) -> f32 {
        self.layout(ui, theme, style);
        *self.offsets.last().unwrap_or(&0.0)
    }

    fn layout(&mut self, ui: &Ui, theme: &Theme, style: Style) {
        if self.style == Some(style) {
            return;
        }
        self.style = Some(style);
        let name_font = palette::px(theme, style.name_px);
        let widest = self.learners.iter().map(|l| text_w(ui, xpr_dex::display_name(&l.species), &name_font)).fold(0.0_f32, f32::max);
        let lead = if style.sprite { 32.0 } else { 0.0 };
        // keep the chips at least 40% of the table
        self.name_col = (lead + widest + 8.0 + 4.0).min(style.width * 0.6);
        let chip_w = (style.width - self.name_col - 4.0).max(40.0);
        let min_h = if style.sprite { 32.0 } else { CHIP_H + 2.0 * PAD_Y };
        self.rows.clear();
        self.offsets.clear();
        let mut y = 0.0;
        for l in &self.learners {
            let widths: Vec<f32> = l.methods.iter().map(|m| kit::chip_width(ui, theme, m)).collect();
            let (pos, h) = kit::flow(&widths, chip_w);
            let height = (h + 2.0 * PAD_Y + 1.0).max(min_h + 1.0);
            let chips_y = (height - 1.0 - h) / 2.0;
            self.rows.push(RowLayout { chips: pos.into_iter().zip(widths).collect(), chips_y, height });
            self.offsets.push(y);
            y += height;
        }
        self.offsets.push(y);
    }

    /// Draw the rows into `area` (whose top-left and width place the table;
    /// the caller has made the `Ui` tall enough for [`Learners::height`]).
    /// Returns the species that was clicked.
    pub fn show(&mut self, ui: &mut Ui, cx: &mut DexCx, area: Rect, style: Style, id: Id) -> Option<String> {
        self.layout(ui, cx.theme, style);
        let mut clicked = None;
        let clip = ui.clip_rect();
        let range = kit::visible_rows(&self.offsets, clip.min.y - area.min.y, clip.max.y - area.min.y);
        let name_font = palette::px(cx.theme, style.name_px);
        for i in range {
            let learner = &self.learners[i];
            let row = &self.rows[i];
            let rect = Rect::from_min_size(Pos2::new(area.min.x, area.min.y + self.offsets[i]), Vec2::new(style.width, row.height));
            let resp = ui.interact(rect, id.with(("learner", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::ZERO, tw("#1f2937", 0.6));
            }
            kit::rule_top(ui, rect, RULE);
            let mut x = rect.min.x + 4.0;
            if style.sprite {
                if let Some(tex) = cx.images.sprite(ui.ctx(), &learner.species, learner.dex, SpriteSize::Small) {
                    images::paint_fit(ui, &tex, Rect::from_center_size(Pos2::new(x + 12.0, rect.center().y), Vec2::splat(26.0)), Color32::WHITE);
                }
                x += 32.0;
            }
            let name = xpr_dex::display_name(&learner.species);
            let name_w = (self.name_col - (x - rect.min.x) - 4.0).max(10.0);
            let shown = xpr_ui_kit::widgets::elide(ui, name, &name_font, name_w);
            ui.painter().text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, shown, name_font.clone(), if resp.hovered() { Color32::WHITE } else { palette::GRAY_100 });
            let chips_x = rect.min.x + self.name_col;
            for (m, (off, w)) in learner.methods.iter().zip(&row.chips) {
                kit::paint_chip(ui, cx.theme, Pos2::new(chips_x + off.x, rect.min.y + 1.0 + row.chips_y + off.y), *w, m);
            }
            if resp.clicked() {
                clicked = Some(learner.species.clone());
            }
        }
        clicked
    }
}

/// Selecting a learner shows it in the Pokédex (Solodex's `onSelectPokemon`).
pub fn open_in_pokedex(cx: &mut DexCx, species: &str) {
    cx.state.select_species(species);
    cx.state.focused_move = None;
    cx.state.settings.tab = crate::DexTab::Pokedex;
    cx.state.touch();
}
