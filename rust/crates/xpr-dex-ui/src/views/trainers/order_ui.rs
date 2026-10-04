//! The team order calculator modal (Solodex `TeamOrderCalculator.tsx`): pick
//! the player's typing, read the predicted send-out order.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use xpr_ui_kit::theme::{self, Theme};

use super::data::{TrainerInfo, VersionIndex};
use super::order::{simulate_order, GenEnv, OrderMon, SimStep, StepReason};
use crate::images::{self, DexImages, SpriteSize};
use crate::palette::{self, px, px_bold};
use crate::widgets;

/// The calculator's types: the pre-Fairy chart.
pub const PRE_FAIRY_TYPES: [&str; 17] = ["Normal", "Fire", "Water", "Electric", "Grass", "Ice", "Fighting", "Poison", "Ground", "Flying", "Psychic", "Bug", "Rock", "Ghost", "Dragon", "Dark", "Steel"];

/// The player's typing, as picked in the modal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderUi {
    pub type1: String,
    pub type2: Option<String>,
}

impl Default for OrderUi {
    fn default() -> OrderUi {
        OrderUi { type1: "Normal".to_string(), type2: None }
    }
}

/// Brighter text shades for the types whose badge colours vanish on the
/// modal's dark background (Solodex `TYPE_TEXT_COLOR_OVERRIDES`).
pub fn move_text_color(t: &str) -> Color32 {
    match t {
        "Ghost" => palette::hex("#b497d6"),
        "Dark" => palette::hex("#a89392"),
        "Ground" => palette::hex("#d2a06a"),
        "Dragon" => palette::hex("#8a96e8"),
        "Poison" => palette::hex("#c084ee"),
        "Bug" => palette::hex("#bcc944"),
        "Grass" => palette::hex("#6cd64f"),
        "Fire" => palette::hex("#ff5454"),
        "Water" => palette::hex("#5da4f5"),
        "Fighting" => palette::hex("#ff9e3d"),
        "Normal" => palette::hex("#c9c9c9"),
        other => palette::type_color(other),
    }
}

pub fn reason_label(step: &SimStep) -> &'static str {
    match step.reason {
        StepReason::Lead => "Lead \u{2014} slot 0 is always sent out first",
        StepReason::Gen2SePick => "Has a super-effective move; player not super-effective vs it",
        StepReason::Gen2NeutralPick => "Neutral matchup (no SE move, player not SE vs it)",
        StepReason::Gen2ForcedRandom | StepReason::Gen3ForcedLinear | StepReason::Gen4ForcedLinear => "Only remaining Pok\u{e9}mon \u{2014} sent out by elimination",
        StepReason::Gen2Random => "All remaining Pok\u{e9}mon are at a type disadvantage \u{2014} game picks randomly",
        StepReason::Gen3Phase1 => "Phase 1: takes the most damage from your STAB and has a super-effective move",
        StepReason::Gen3Phase2 => "Phase 2 fallback: damage-based pick that depends on engine quirks (uses the active battler's stats and a u8 wraparound) \u{2014} can't be deterministically predicted",
        StepReason::Gen4Stage1 => "Stage 1: highest offensive type score against you AND has a super-effective move",
        StepReason::Gen4Stage2 => "Stage 2 fallback: damage-based pick that depends on engine quirks (uses the active battler's stats and a u8 wraparound) \u{2014} can't be deterministically predicted",
    }
}

pub fn gen_note(gen: u8) -> &'static str {
    match gen {
        2 => "Lead is always slot 0. After each faint, the trainer picks the lowest-slot Pok\u{e9}mon with a super-effective move that you can't super-effectively counter; failing that, the lowest-slot neutral matchup; failing that, a random pick among the remaining. Move power is not consulted.",
        3 => "Lead is always slot 0. After each faint, Phase 1 picks the alive Pok\u{e9}mon that takes the *most* damage from your STAB AND has at least one super-effective move (a counter-intuitive defensive scoring quirk in stock Emerald). If no candidate qualifies, the engine falls back to a damage-based estimate (Phase 2) whose result is hard to predict deterministically.",
        _ => "Lead is always slot 0. After each faint, Stage 1 picks the alive Pok\u{e9}mon whose offensive type score against you is highest AND that has at least one super-effective move (Gen 4 fixed Gen 3's defensive-scoring direction). The score is summed across both of the candidate's types and is held in a u8 \u{2014} values above 255 wrap silently in stock Platinum, which can flip the winner. If Stage 1 finds nobody, the engine falls back to a damage-based estimate (Stage 2) that can't be deterministically predicted.",
    }
}

/// `typeDmg` in engine units as a multiplier ("2\u{d7}", "0.5\u{d7}").
pub fn format_multiplier(type_dmg: i64) -> String {
    let m = type_dmg as f64 / 10.0;
    format!("{}\u{d7}", m)
}

fn fmt(theme: &Theme, size: f32, color: Color32) -> TextFormat {
    TextFormat { font_id: px(theme, size), color, ..Default::default() }
}

/// A type picker button: the type's colour, dimmed while not selected, a white
/// outline while selected. Returns true when clicked.
fn type_button(ui: &mut Ui, theme: &Theme, t: &str, selected: bool) -> bool {
    let font = px_bold(theme, 12.0);
    let w = (widgets::text_w(ui, t, &font) + 16.0).max(64.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    let alpha = if selected { 255 } else { 140 };
    widgets::paint_glossy(ui, rect, 4, theme::with_alpha(palette::type_color(t), alpha));
    let fg = Color32::from_white_alpha(alpha);
    ui.painter().text(rect.center() + Vec2::new(0.0, 1.0), Align2::CENTER_CENTER, t, font.clone(), Color32::from_black_alpha(if selected { 100 } else { 55 }));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, t, font, fg);
    if selected {
        ui.painter().rect_stroke(rect.expand(3.0), CornerRadius::same(6), Stroke::new(2.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn none_button(ui: &mut Ui, theme: &Theme, selected: bool) -> bool {
    let font = px_bold(theme, 12.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(64.0, 24.0), Sense::click());
    let a = if selected { 255 } else { 140 };
    ui.painter().rect_filled(rect, CornerRadius::same(4), theme::with_alpha(palette::GRAY_700, a));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "None", font, theme::with_alpha(palette::GRAY_300, a));
    if selected {
        ui.painter().rect_stroke(rect.expand(3.0), CornerRadius::same(6), Stroke::new(2.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn caption(ui: &mut Ui, theme: &Theme, text: &str) {
    let g = xpr_ui_kit::widgets::caption_galley(ui, text, px(theme, 12.0), palette::GRAY_500);
    let (rect, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, palette::GRAY_500);
}

fn mon_icon(ui: &mut Ui, images: &mut DexImages, t: &TrainerInfo, slot: usize, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let m = &t.party[slot];
    if let Some(key) = m.dex_key {
        if let Some(tex) = images.sprite(ui.ctx(), key, m.dex, SpriteSize::Small) {
            images::paint_fit(ui, &tex, rect, Color32::WHITE);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_row(ui: &mut Ui, theme: &Theme, images: &mut DexImages, ix: &VersionIndex, t: &TrainerInfo, step: &SimStep, index: usize) {
    let mono = egui::FontId::new(px(theme, 12.0).size, egui::FontFamily::Monospace);
    let width = ui.available_width();
    if step.reason.is_fallback() {
        let heading = match step.reason {
            StepReason::Gen2Random => "Random fallback",
            StepReason::Gen3Phase2 => "Phase 2 fallback",
            _ => "Stage 2 fallback",
        };
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(0x71, 0x3f, 0x12, 26))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xa1, 0x62, 0x07, 153)))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(width - 26.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                    ui.label(RichText::new(format!("#{}", index + 1)).font(mono.clone()).color(palette::YELLOW_400));
                    ui.label(RichText::new(heading).font(px_bold(theme, 14.0)).color(palette::hex("#fde047")));
                });
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(reason_label(step)).font(px(theme, 12.0)).color(palette::GRAY_400)).wrap());
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    for &slot in &step.fallback_candidates {
                        let ev = step.evaluations.iter().find(|e| e.slot == slot);
                        egui::Frame::new().fill(palette::GRAY_800).stroke(Stroke::new(1.0_f32, palette::GRAY_700)).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                mon_icon(ui, images, t, slot, 20.0);
                                let mut job = LayoutJob::default();
                                job.append(&format!("slot {}", slot), 0.0, fmt(theme, 12.0, palette::GRAY_500));
                                job.append(&format!(" \u{b7} {}", t.party[slot].display), 0.0, fmt(theme, 12.0, palette::GRAY_200));
                                if step.reason == StepReason::Gen3Phase2 {
                                    if let Some(d) = ev.and_then(|e| e.type_dmg) {
                                        job.append(&format!(" \u{b7} {}", format_multiplier(d)), 0.0, fmt(theme, 12.0, palette::GRAY_500));
                                    }
                                }
                                if step.reason == StepReason::Gen4Stage2 {
                                    if let Some(s) = ev.and_then(|e| e.gen4_score) {
                                        job.append(&format!(" \u{b7} score {}", s), 0.0, fmt(theme, 12.0, palette::GRAY_500));
                                    }
                                }
                                ui.label(job);
                            });
                        });
                    }
                });
            });
        return;
    }
    let Some(slot) = step.sent_out else { return };
    let mon = &t.party[slot];
    let ev = step.evaluations.iter().find(|e| e.slot == slot);
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(0x1f, 0x29, 0x37, 102))
        .stroke(Stroke::new(1.0_f32, palette::GRAY_700))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(width - 26.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(24.0, 28.0), Sense::hover());
                ui.painter().text(Pos2::new(r.min.x, r.center().y), Align2::LEFT_CENTER, format!("#{}", index + 1), mono.clone(), palette::GRAY_500);
                mon_icon(ui, images, t, slot, 28.0);
                let chip = format!("slot {}", slot);
                let cw = widgets::text_w(ui, &chip, &mono) + 12.0;
                let (r, _) = ui.allocate_exact_size(Vec2::new(cw, 20.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(4), palette::GRAY_700);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, chip, mono.clone(), palette::GRAY_400);
                ui.label(RichText::new(&mon.display).font(px_bold(theme, 14.0)).color(Color32::WHITE));
                ui.label(RichText::new(format!("Lv {}", mon.level)).font(px(theme, 12.0)).color(palette::GRAY_500));
            });
            ui.add_space(6.0);
            let indent = 68.0;
            let line = |ui: &mut Ui, job: LayoutJob| {
                ui.horizontal_top(|ui| {
                    ui.add_space(indent);
                    ui.add(egui::Label::new(job).wrap());
                });
            };
            let mut job = LayoutJob::default();
            job.append(reason_label(step), 0.0, fmt(theme, 12.0, palette::GRAY_400));
            line(ui, job);
            if let Some(mv) = ev.and_then(|e| e.se_move.as_deref()) {
                ui.add_space(2.0);
                let mut job = LayoutJob::default();
                job.append("SE move: ", 0.0, fmt(theme, 12.0, palette::GRAY_500));
                let color = ix.gen.move_db().get_move(mv).map(|m| move_text_color(&m.move_type)).unwrap_or(palette::GRAY_200);
                job.append(mv, 0.0, TextFormat { font_id: px_bold(theme, 12.0), color, ..Default::default() });
                line(ui, job);
            }
            if step.reason == StepReason::Gen3Phase1 {
                if let Some(d) = ev.and_then(|e| e.type_dmg) {
                    ui.add_space(2.0);
                    let mut job = LayoutJob::default();
                    job.append("Damage taken from your STAB: ", 0.0, fmt(theme, 12.0, palette::GRAY_500));
                    job.append(&format_multiplier(d), 0.0, fmt(theme, 12.0, palette::GRAY_300));
                    line(ui, job);
                }
            }
            if step.reason == StepReason::Gen4Stage1 {
                if let Some(s) = ev.and_then(|e| e.gen4_score) {
                    ui.add_space(2.0);
                    let mut job = LayoutJob::default();
                    job.append("Offensive type score: ", 0.0, fmt(theme, 12.0, palette::GRAY_500));
                    job.append(&s.to_string(), 0.0, fmt(theme, 12.0, palette::GRAY_300));
                    if let Some(b) = ev.and_then(|e| e.gen4_score_8bit) {
                        if b != s {
                            job.append(&format!(" (8-bit: {} \u{2014} wraps in stock Platinum)", b), 0.0, fmt(theme, 12.0, Color32::from_rgba_unmultiplied(0xea, 0xb3, 0x08, 179)));
                        }
                    }
                    line(ui, job);
                }
            }
        });
}

/// Show the calculator; returns true when it should close.
#[allow(clippy::too_many_arguments)]
pub fn show(ctx: &egui::Context, theme: &Theme, images: &mut DexImages, ix: &VersionIndex, t: &TrainerInfo, party: &[OrderMon], gen: u8, st: &mut OrderUi) -> bool {
    let screen = ctx.content_rect();
    let card_w = (screen.width() - 32.0).min(672.0);
    let max_body = (screen.height() * 0.9 - 64.0).max(200.0);
    let mut close = false;
    let modal = egui::Modal::new(Id::new("trainer_order_calculator"))
        .frame(
            egui::Frame::new()
                .fill(palette::hex("#1a1f29"))
                .stroke(Stroke::new(1.0_f32, palette::GRAY_700))
                .corner_radius(CornerRadius::same(12))
                .shadow(egui::Shadow { offset: [0, 16], blur: 48, spread: 0, color: Color32::from_black_alpha(160) }),
        )
        .backdrop_color(Color32::from_black_alpha(153));
    let resp = modal.show(ctx, |ui| {
        ui.set_width(card_w);
        // an Area only offers the space below its anchor; the card may use up to 90% of the window
        ui.set_max_height(max_body + 64.0);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        // header
        let head_h = 56.0;
        let (head, _) = ui.allocate_exact_size(Vec2::new(card_w, head_h), Sense::hover());
        ui.painter().text(Pos2::new(head.min.x + 20.0, head.min.y + 19.0), Align2::LEFT_CENTER, "Team Order Calculator", px_bold(theme, 16.0), Color32::WHITE);
        ui.painter().text(Pos2::new(head.min.x + 20.0, head.min.y + 38.0), Align2::LEFT_CENTER, format!("{} \u{b7} Gen {} send-out logic", t.trainer.name, gen), px(theme, 12.0), palette::GRAY_500);
        let x_rect = Rect::from_center_size(Pos2::new(head.max.x - 28.0, head.center().y), Vec2::splat(28.0));
        let xr = ui.interact(x_rect, Id::new("trainer_order_close"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let xc = if xr.hovered() { Color32::WHITE } else { palette::GRAY_500 };
        let c = x_rect.center();
        ui.painter().line_segment([c + Vec2::new(-6.0, -6.0), c + Vec2::new(6.0, 6.0)], Stroke::new(2.0_f32, xc));
        ui.painter().line_segment([c + Vec2::new(-6.0, 6.0), c + Vec2::new(6.0, -6.0)], Stroke::new(2.0_f32, xc));
        if xr.clicked() {
            close = true;
        }
        ui.painter().hline(head.x_range(), head.max.y + 0.5, Stroke::new(1.0_f32, palette::GRAY_700));
        ui.add_space(1.0);

        xpr_ui_kit::widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("trainer_order_body").max_height(max_body).auto_shrink([false, true]), |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 20, top: 16, bottom: 16 }).show(ui, |ui| {
                ui.set_width(card_w - 40.0);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                caption(ui, theme, "Your Pok\u{e9}mon's type 1");
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    for t in PRE_FAIRY_TYPES {
                        if type_button(ui, theme, t, st.type1 == t) {
                            st.type1 = t.to_string();
                        }
                    }
                });
                ui.add_space(20.0);
                caption(ui, theme, "Your Pok\u{e9}mon's type 2 (optional)");
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    if none_button(ui, theme, st.type2.is_none()) {
                        st.type2 = None;
                    }
                    for t in PRE_FAIRY_TYPES {
                        if type_button(ui, theme, t, st.type2.as_deref() == Some(t)) {
                            st.type2 = if st.type2.as_deref() == Some(t) { None } else { Some(t.to_string()) };
                        }
                    }
                });
                ui.add_space(20.0);
                caption(ui, theme, "Predicted send-out order");
                ui.add_space(8.0);
                let steps = simulate_order(&GenEnv(&ix.gen), party, gen, &st.type1, st.type2.as_deref());
                for (i, step) in steps.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(8.0);
                    }
                    step_row(ui, theme, images, ix, t, step, i);
                }
                if steps.is_empty() {
                    ui.label(RichText::new("No party.").font(px(theme, 14.0)).color(palette::GRAY_500));
                }
                ui.add_space(20.0);
                widgets::hairline(ui, palette::GRAY_800);
                ui.add_space(12.0);
                let note = format!("{} Assumes your active Pok\u{e9}mon does not switch out.", gen_note(gen));
                ui.add(egui::Label::new(RichText::new(note).font(px(theme, 11.0)).color(palette::GRAY_600)).wrap());
            });
        });
    });
    if resp.should_close() {
        close = true;
    }
    close
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipliers_print_like_solodex() {
        assert_eq!(format_multiplier(0), "0\u{d7}");
        assert_eq!(format_multiplier(10), "1\u{d7}");
        assert_eq!(format_multiplier(20), "2\u{d7}");
        assert_eq!(format_multiplier(40), "4\u{d7}");
        assert_eq!(format_multiplier(5), "0.5\u{d7}");
        assert_eq!(format_multiplier(2), "0.2\u{d7}");
    }

    #[test]
    fn the_type_list_is_the_pre_fairy_chart() {
        assert_eq!(PRE_FAIRY_TYPES.len(), 17);
        assert!(!PRE_FAIRY_TYPES.contains(&"Fairy"));
        assert_eq!(OrderUi::default(), OrderUi { type1: "Normal".into(), type2: None });
    }
}
