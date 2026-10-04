//! The TM and move-tutor popovers (Solodex `TmPopover.tsx`, `TutorPopover.tsx`).
//!
//! Solodex fills them from Bulbapedia / Serebii pages fetched through
//! Electron (where a TM or tutor is found, prices). That needs the network,
//! so here they show what the Dex's own data has (the TM's move and its
//! stats) and offer the same pages as links (`DexAction::OpenUrl`).

use egui::{Align2, Color32, CornerRadius, PopupCloseBehavior, RectAlign, Response, Sense, Vec2};
use xpr_dex::{get_move_data, tmhm::tmhm_list};
use xpr_ui_kit::theme::Theme;

use super::paint::link_button;
use crate::palette;
use crate::widgets;
use crate::DexAction;

const ALTERNATIVES: [RectAlign; 3] = [
    RectAlign::LEFT_START,
    RectAlign::BOTTOM_START,
    RectAlign::TOP_START,
];

fn popup<'a>(anchor: &Response, theme: &Theme) -> egui::Popup<'a> {
    egui::Popup::from_toggle_button_response(anchor)
        .align(RectAlign::RIGHT_START)
        .align_alternatives(&ALTERNATIVES)
        .gap(6.0)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(widgets::popover_frame(theme))
}

/// Bulbapedia's page name of a TM code: "TM24" -> "TM024", HMs stay.
pub fn bulbapedia_tm_page(code: &str) -> String {
    match code.strip_prefix("TM") {
        Some(n) => format!("TM{:0>3}", n),
        None => code.to_string(),
    }
}

pub fn bulbapedia_tm_url(code: &str) -> String {
    format!(
        "https://bulbapedia.bulbagarden.net/wiki/{}",
        bulbapedia_tm_page(code)
    )
}

/// The move a TM / HM code teaches in `game`.
pub fn tm_move(code: &str, game: &str) -> Option<String> {
    tmhm_list(game)
        .into_iter()
        .find(|(c, _)| c == code)
        .map(|(_, m)| m)
}

/// The popover of a TM / HM code, opened by clicking its `anchor`.
pub fn tm_popover(
    anchor: &Response,
    theme: &Theme,
    code: &str,
    game: &str,
    actions: &mut Vec<DexAction>,
) {
    let gen = xpr_dex::game_gen(game);
    popup(anchor, theme).show(|ui| {
        ui.set_width(380.0);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
        let mv = tm_move(code, game);
        ui.horizontal(|ui| {
            let title = match &mv {
                Some(m) => format!("{} \u{2014} {}", code, m),
                None => code.to_string(),
            };
            ui.label(
                egui::RichText::new(title)
                    .font(palette::px_bold(theme, 14.0))
                    .color(Color32::WHITE),
            );
            ui.label(
                egui::RichText::new(format!("Gen {}", gen))
                    .font(palette::px(theme, 12.0))
                    .color(palette::GRAY_500),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if link_button(ui, theme, "Bulbapedia")
                    .on_hover_text("Open on Bulbapedia")
                    .clicked()
                {
                    actions.push(DexAction::OpenUrl(bulbapedia_tm_url(code)));
                }
            });
        });
        if let Some(data) = mv.as_deref().and_then(|m| get_move_data(m, game)) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                widgets::type_chip(ui, theme, &data.move_type, 68.0);
                ui.label(
                    egui::RichText::new(&data.category)
                        .font(palette::px(theme, 14.0))
                        .color(palette::category_color(&data.category)),
                );
                let stat = |label: &str, v: Option<i32>| {
                    format!(
                        "{} {}",
                        label,
                        v.map(|n| n.to_string())
                            .unwrap_or_else(|| "\u{2014}".to_string())
                    )
                };
                let text = format!(
                    "{}   {}   {}",
                    stat("Pwr", data.power),
                    stat("Acc", data.accuracy),
                    stat("PP", data.pp)
                );
                ui.label(
                    egui::RichText::new(text)
                        .font(palette::px(theme, 14.0))
                        .color(palette::GRAY_100),
                );
            });
            if !data.description.is_empty() {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&data.description)
                            .font(palette::px(theme, 12.0))
                            .color(palette::GRAY_300),
                    )
                    .wrap(),
                );
            }
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "Where to find it and its price are on Bulbapedia (not available offline).",
                )
                .font(palette::px(theme, 12.0))
                .color(palette::GRAY_500),
            )
            .wrap(),
        );
    });
}

// ---------------------------------------------------------------------------
// move tutor
// ---------------------------------------------------------------------------

/// A game version with a Serebii move tutor page.
#[derive(Clone, Copy, Debug)]
pub struct TutorVersion {
    pub game: &'static str,
    pub label: &'static str,
    pub slug: &'static str,
}

const fn tv(game: &'static str, label: &'static str, slug: &'static str) -> TutorVersion {
    TutorVersion { game, label, slug }
}

/// Versions within each generation that have Serebii move tutor pages
/// (`GEN_TUTOR_VERSIONS`).
pub fn tutor_versions(gen: u8) -> &'static [TutorVersion] {
    const G2: [TutorVersion; 1] = [tv("Crystal", "Crystal", "crystalversion")];
    const G3: [TutorVersion; 3] = [
        tv("Ruby and Sapphire", "Ruby & Sapphire", "rubysapphire"),
        tv("Emerald", "Emerald", "emerald"),
        tv(
            "FireRed and LeafGreen",
            "FireRed & LeafGreen",
            "fireredleafgreen",
        ),
    ];
    const G4: [TutorVersion; 3] = [
        tv("Diamond and Pearl", "Diamond & Pearl", "diamondpearl"),
        tv("Platinum", "Platinum", "platinum"),
        tv(
            "HeartGold and SoulSilver",
            "HeartGold & SoulSilver",
            "heartgoldsoulsilver",
        ),
    ];
    const G5: [TutorVersion; 2] = [
        tv("Black", "Black & White", "blackwhite"),
        tv("Black 2 and White 2", "Black 2 & White 2", "black2white2"),
    ];
    const G6: [TutorVersion; 2] = [
        tv("X and Y", "X & Y", "xy"),
        tv(
            "Omega Ruby and Alpha Sapphire",
            "Omega Ruby & Alpha Sapphire",
            "omegarubyalphasapphire",
        ),
    ];
    const G7: [TutorVersion; 2] = [
        tv("Sun and Moon", "Sun & Moon", "sunmoon"),
        tv(
            "Ultra Sun and Ultra Moon",
            "Ultra Sun & Ultra Moon",
            "ultrasunultramoon",
        ),
    ];
    const G8: [TutorVersion; 1] = [tv("Sword and Shield", "Sword & Shield", "swordshield")];
    const G9: [TutorVersion; 1] = [tv(
        "Scarlet and Violet",
        "Scarlet & Violet",
        "scarletviolet",
    )];
    match gen {
        2 => &G2,
        3 => &G3,
        4 => &G4,
        5 => &G5,
        6 => &G6,
        7 => &G7,
        8 => &G8,
        9 => &G9,
        _ => &[],
    }
}

pub fn serebii_tutor_url(slug: &str) -> String {
    format!("https://www.serebii.net/{}/movetutor.shtml", slug)
}

pub const BULBAPEDIA_TUTOR_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Move_Tutor";

/// The popover of the "Tutor" label, opened by clicking its `anchor`: pick
/// the game version (when the generation has several), then links to the
/// tutor pages.
pub fn tutor_popover(anchor: &Response, theme: &Theme, game: &str, actions: &mut Vec<DexAction>) {
    let versions = tutor_versions(xpr_dex::game_gen(game));
    let sel_id = anchor.id.with("tutor_version");
    let shown = popup(anchor, theme).show(|ui| {
        ui.set_width(320.0);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
        // one version is selected at once; a single version is picked for the user
        let selected: Option<usize> = ui.data(|d| d.get_temp::<Option<usize>>(sel_id)).unwrap_or(if versions.len() == 1 { Some(0) } else { None });
        let mut next = selected;
        let font = palette::px(theme, 12.0);
        ui.horizontal(|ui| {
            if selected.is_some() && versions.len() > 1 {
                let (r, back) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                let c = if back.hovered() { palette::GRAY_300 } else { palette::GRAY_500 };
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "\u{25C0}", font.clone(), c);
                if back.on_hover_text("Back to version selection").clicked() {
                    next = None;
                }
            }
            let title = match selected.and_then(|i| versions.get(i)) {
                Some(v) => format!("Move Tutor \u{2014} {}", v.label),
                None => "Select Version".to_string(),
            };
            ui.label(egui::RichText::new(title).font(palette::px_bold(theme, 14.0)).color(Color32::WHITE));
        });
        match selected.and_then(|i| versions.get(i)) {
            None => {
                if versions.is_empty() {
                    ui.label(egui::RichText::new("No move tutor data available for this generation.").font(font.clone()).color(palette::GRAY_500));
                }
                for (i, v) in versions.iter().enumerate() {
                    let h = 32.0;
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
                    super::probe::record("version", "", i, v.label, rect);
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::same(4), palette::GRAY_700);
                    }
                    let c = if resp.hovered() { Color32::WHITE } else { palette::GRAY_300 };
                    ui.painter().text(egui::Pos2::new(rect.min.x + 12.0, rect.center().y), Align2::LEFT_CENTER, v.label, palette::px(theme, 14.0), c);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        next = Some(i);
                    }
                }
            }
            Some(v) => {
                ui.add(egui::Label::new(egui::RichText::new("Where each tutor is and what it costs are on Serebii and Bulbapedia (not available offline).").font(font.clone()).color(palette::GRAY_500)).wrap());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    if link_button(ui, theme, "Serebii").on_hover_text("Open on Serebii").clicked() {
                        actions.push(DexAction::OpenUrl(serebii_tutor_url(v.slug)));
                    }
                    if link_button(ui, theme, "Bulbapedia").on_hover_text("Open on Bulbapedia").clicked() {
                        actions.push(DexAction::OpenUrl(BULBAPEDIA_TUTOR_URL.to_string()));
                    }
                });
            }
        }
        if next != selected {
            ui.data_mut(|d| d.insert_temp(sel_id, next));
        }
    });
    if shown.is_none() {
        // closed: the next open starts at the version list again
        anchor
            .ctx
            .data_mut(|d| d.remove_temp::<Option<usize>>(sel_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulbapedia_pages() {
        assert_eq!(bulbapedia_tm_page("TM24"), "TM024");
        assert_eq!(bulbapedia_tm_page("TM100"), "TM100");
        assert_eq!(bulbapedia_tm_page("HM01"), "HM01");
        assert_eq!(
            bulbapedia_tm_url("TM05"),
            "https://bulbapedia.bulbagarden.net/wiki/TM005"
        );
    }

    #[test]
    fn tutor_versions_by_generation() {
        assert!(tutor_versions(1).is_empty());
        assert_eq!(tutor_versions(2).len(), 1);
        assert_eq!(tutor_versions(3).len(), 3);
        assert_eq!(tutor_versions(8)[0].slug, "swordshield");
    }
}
