//! The Overview tab (`docs/rust_port/design/route_compare/SPEC.md` §6.1).

use egui::{Align, Color32, Rect, Sense, Ui, Vec2};

use xpr_engine::compare::{format_time, MoveSource, RouteComparison, RouteDigest};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Side};

use crate::assets::{self, Assets};
use crate::summaries::type_color;

use super::shared::{self, Cmp, ROW_H};

/// Draws the tab. Returns a trainer name when the user clicked a name chip
/// (the caller switches to the event diff and scrolls there).
pub fn ui(ui: &mut Ui, theme: &Theme, assets: &mut Assets, cmp: &RouteComparison, highlight: bool) -> Option<String> {
    let c = Cmp { theme, highlight };
    let mut jump = None;
    let narrow = ui.available_width() < 1100.0;

    // identity cards: one width for both, measured before either is drawn
    // (the second card would otherwise get half of what the first left over)
    let id_w = if narrow { ui.available_width() } else { (ui.available_width() - 12.0) / 2.0 };
    row(ui, narrow, |ui| {
        identity_card(ui, theme, assets, cmp, Side::A, id_w);
        identity_card(ui, theme, assets, cmp, Side::B, id_w);
    });

    // setup + stats + exp sources
    let comparable = cmp.compat.same_generation;
    row(ui, narrow, |ui| {
        let (left, right) = split(ui, narrow, SIDE_W);
        if comparable {
            sized(ui, left.min(SIDE_W), |ui| setup_card(ui, &c, cmp));
        }
        // Without the setup card this column takes the whole row.
        let w = if comparable && !narrow { right } else { ui.available_width() };
        sized(ui, w, |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            if comparable {
                stats_card(ui, &c, cmp);
            }
            exp_sources_card(ui, &c, cmp);
        });
    });

    // totals + money / moves / trainers
    row(ui, narrow, |ui| {
        // The totals card must be given its width: a card left to itself
        // takes the whole row and pushes the right-hand column off screen.
        let (side, main) = split(ui, narrow, SIDE_W);
        sized(ui, main, |ui| totals_card(ui, &c, cmp));
        sized(ui, side.min(SIDE_W), |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            money_card(ui, &c, cmp);
            final_moves_card(ui, &c, cmp);
            if let Some(name) = trainers_card(ui, &c, cmp) {
                jump = Some(name);
            }
        });
    });

    moves_learned_card(ui, &c, cmp);
    jump
}

/// A row of cards that becomes one column when the page is narrow.
fn row(ui: &mut Ui, narrow: bool, add: impl FnOnce(&mut Ui)) {
    if narrow {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            add(ui);
        });
    } else {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            add(ui);
        });
    }
}

/// Width of the narrow side column of the two-column rows.
const SIDE_W: f32 = 440.0;

/// `(side column, the rest)` for the current row. When the page is narrow
/// both are the full width, because the row stacks.
fn split(ui: &Ui, narrow: bool, side: f32) -> (f32, f32) {
    let avail = ui.available_width();
    if narrow {
        return (avail, avail);
    }
    let side = side.min(avail * 0.5);
    (side, (avail - side - 12.0).max(200.0))
}

/// A fixed-width column inside a [`row`].
fn sized(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    let w = width.min(ui.available_width());
    ui.allocate_ui(Vec2::new(w, ui.available_height()), |ui| {
        ui.set_width(w);
        ui.vertical(|ui| add(ui));
    });
}

// ---------------------------------------------------------------------------

fn identity_card(ui: &mut Ui, theme: &Theme, assets: &mut Assets, cmp: &RouteComparison, side: Side, width: f32) {
    let d = if side == Side::A { &cmp.a } else { &cmp.b };
    ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
        ui.set_width(width);
        widgets::card(ui, theme, Some(egui::Margin { left: 16, right: 16, top: 14, bottom: 14 }), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                // icon tile
                let (tile, _) = ui.allocate_exact_size(Vec2::splat(68.0), Sense::hover());
                ui.painter().rect(
                    tile,
                    egui::CornerRadius::same(12),
                    theme.well_bg(),
                    egui::Stroke::new(1.0_f32, theme.card_border()),
                    egui::StrokeKind::Inside,
                );
                if let Some(tex) = assets.pkmn_icon(ui.ctx(), &d.species) {
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tile).layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)));
                    assets::draw_fit(&mut child, &tex, 60.0);
                }

                // The "final time" block is laid out first so the main
                // column can be given exactly the width that is left; a
                // right_to_left layout would overlap it instead.
                let right_w = 150.0_f32;
                let main_w = (ui.available_width() - right_w - 14.0).max(120.0);
                // `allocate_ui` would inherit this row's horizontal layout and
                // lay the three lines out side by side.
                ui.allocate_ui_with_layout(Vec2::new(main_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                    ui.set_width(main_w);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        widgets::route_badge(ui, theme, side);
                        ui.label(egui::RichText::new(&d.species).font(theme.font_bold(16.5)).color(theme.text_strong()));
                        // Neutral pill and chip: the tinted variants would
                        // read as "the other route" here (SPEC §3.1).
                        widgets::pill(ui, theme, &format!("Lv {}", d.end.level), theme.text_strong(), theme.well_bg(), theme.border);
                        widgets::chip_outlined(ui, theme, &d.version, theme.secondary, theme.well_bg(), theme.card_border());
                    });
                    if d.generation >= 2 {
                        let mut parts: Vec<String> = Vec::new();
                        if let Some(a) = &d.ability {
                            parts.push(a.clone());
                        }
                        if let Some(n) = &d.nature {
                            parts.push(n.clone());
                        }
                        parts.push(format!("Held item: {}", d.end.held_item.clone().unwrap_or_else(|| "none".into())));
                        ui.label(egui::RichText::new(parts.join(" \u{b7} ")).font(theme.body()).color(theme.secondary));
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{} events in {} folders \u{b7} {} disabled",
                            widgets::fmt_thousands(d.event_count() as i64),
                            d.folder_count,
                            d.disabled_events
                        ))
                        .font(theme.body())
                        .color(theme.secondary),
                    );
                });

                ui.allocate_ui_with_layout(Vec2::new(right_w, 0.0), egui::Layout::top_down(Align::Max), |ui| {
                    ui.set_width(right_w);
                    ui.spacing_mut().item_spacing.y = 3.0;
                    widgets::caption(ui, theme, "Final time", None);
                    let text = d.final_time.map(format_time).unwrap_or_else(|| "\u{2014}".into());
                    ui.label(egui::RichText::new(text).font(theme.font_bold(13.5)).color(theme.text_strong()));
                    if side == Side::B {
                        if let (Some(fa), Some(fb)) = (cmp.a.final_time, cmp.b.final_time) {
                            let delta = fb - fa;
                            if delta.abs() >= 0.05 {
                                let (word, color) = if delta < 0.0 { ("faster", theme.success) } else { ("slower", theme.failure) };
                                ui.label(
                                    egui::RichText::new(format!("{} {} than A", format_time(delta.abs()), word))
                                        .font(theme.caption_font())
                                        .color(color),
                                );
                            }
                        }
                    }
                });
            });
        });
    });
}

// ---------------------------------------------------------------------------

const STAT_NAMES: [&str; 6] = ["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"];
const STAT_NAMES_GEN1: [&str; 5] = ["HP", "Attack", "Defense", "Special", "Speed"];

/// Stat rows for a generation: `(label, index into the six-value arrays)`.
pub(super) fn stat_rows(generation: u8) -> Vec<(&'static str, usize)> {
    if generation == 1 {
        STAT_NAMES_GEN1.iter().copied().zip([0usize, 1, 2, 3, 5]).collect()
    } else {
        STAT_NAMES.iter().copied().zip(0usize..6).collect()
    }
}

fn setup_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    let (a, b) = (&cmp.a, &cmp.b);
    widgets::card(ui, theme, None, |ui| {
        let title = widgets::card_title_colored(ui, theme, "Pokémon setup", None, theme.text_strong());
        let cols = shared::three_col_heads(ui, theme, title, a.dv_text());

        for (name, idx) in stat_rows(a.generation) {
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            shared::name_cell(ui, theme, rect, name, false);
            shared::value_pair(ui, c, rect, &cols, a.dvs[idx], b.dvs[idx]);
        }
        let rect = widgets::table_row(ui, ROW_H, Some(theme.pane_divider()));
        shared::name_cell(ui, theme, rect, "Total", false);
        shared::value_pair(ui, c, rect, &cols, a.dv_total(), b.dv_total());

        // Gen 1 has none of these, so the heading would stand alone.
        let has_traits = a.hidden_power.is_some() || a.nature.is_some() || a.ability.is_some();
        if has_traits {
            shared::group_heading(ui, theme, "Traits");
        }
        // Trait values are words, not two-digit numbers, so they get wider
        // columns than the DV/IV rows above.
        let wide = shared::trait_cols(ui.min_rect().max.x);

        if let (Some(ha), Some(hb)) = (&a.hidden_power, &b.hidden_power) {
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            shared::name_cell(ui, theme, rect, "Hidden Power", false);
            hidden_power_cell(ui, theme, wide.a(rect), ha);
            hidden_power_cell(ui, theme, wide.b(rect), hb);
            shared::differs_cell(ui, c, wide.delta(rect), ha != hb);
        }
        if let (Some(na), Some(nb)) = (&a.nature, &b.nature) {
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            shared::name_cell(ui, theme, rect, "Nature", false);
            nature_cell(ui, theme, wide.a(rect), na, a);
            nature_cell(ui, theme, wide.b(rect), nb, b);
            shared::differs_cell(ui, c, wide.delta(rect), na != nb);
        }
        if let (Some(aa), Some(ab)) = (&a.ability, &b.ability) {
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            shared::name_cell(ui, theme, rect, "Ability", false);
            widgets::col_text(ui, wide.a(rect), aa, theme.body(), theme.text, Align::Max);
            widgets::col_text(ui, wide.b(rect), ab, theme.body(), theme.text, Align::Max);
            shared::differs_cell(ui, c, wide.delta(rect), aa != ab);
        }
    });
}

fn hidden_power_cell(ui: &mut Ui, theme: &Theme, cell: Rect, hp: &(String, i64)) {
    let power = format!(" {}", hp.1);
    let pw = widgets::text_width(ui, &power, &theme.body());
    let label = hp.0.to_uppercase();
    let tw = widgets::text_width(ui, &label, &theme.caption_font_bold()) + 16.0;
    let chip = Rect::from_min_size(egui::Pos2::new(cell.max.x - pw - tw, cell.center().y - 9.0), Vec2::new(tw, 18.0));
    let bg = type_color(&hp.0);
    ui.painter().rect_filled(chip, egui::CornerRadius::same(3), bg);
    let g = widgets::caption_galley(ui, &hp.0, theme.caption_font_bold(), theme.bg);
    ui.painter().galley(chip.center() - g.size() / 2.0, g, theme.bg);
    widgets::col_text(ui, cell, &power, theme.body(), theme.text, Align::Max);
}

fn nature_cell(ui: &mut Ui, theme: &Theme, cell: Rect, name: &str, d: &RouteDigest) {
    // Modifiers stay muted: green/red mean time only (D6).
    let mods = match (d.nature_up, d.nature_down) {
        (Some(up), Some(down)) => format!(" +{} \u{2212}{}", up, down),
        _ => String::new(),
    };
    let mw = widgets::text_width(ui, &mods, &theme.caption_font());
    let nw = widgets::text_width(ui, name, &theme.body());
    // Drop the modifiers rather than let them collide with the name.
    let mw = if mw + nw <= cell.width() { mw } else { 0.0 };
    if mw > 0.0 {
        widgets::col_text(ui, cell, &mods, theme.caption_font(), theme.secondary, Align::Max);
    }
    let name_cell = Rect::from_min_max(cell.min, egui::Pos2::new(cell.max.x - mw, cell.max.y));
    widgets::col_text(ui, name_cell, name, theme.body(), theme.text, Align::Max);
}

fn stats_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    let (a, b) = (&cmp.a, &cmp.b);
    widgets::card(ui, theme, None, |ui| {
        let title = widgets::card_title_colored(ui, theme, "Final stats and EVs", Some("at the end of each route"), theme.text_strong());
        let ev = a.ev_text();
        let cols = shared::stats_col_heads(ui, theme, title, ev);

        let max_stat = a.end.stats.iter().chain(b.end.stats.iter()).copied().max().unwrap_or(1).max(1) as f32;
        for (name, idx) in stat_rows(a.generation) {
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            shared::name_cell(ui, theme, rect, name, false);
            let (sa, sb) = (a.end.stats[idx], b.end.stats[idx]);
            shared::value_pair(ui, c, rect, &cols.stats, sa, sb);
            widgets::paired_bars(ui, theme, cols.bars(rect), sa as f32 / max_stat, sb as f32 / max_stat);
            shared::value_pair_plain(ui, c, rect, &cols.evs, a.end.evs[idx], b.end.evs[idx]);
        }
        let rect = widgets::table_row(ui, ROW_H, Some(theme.pane_divider()));
        shared::name_cell(ui, theme, rect, &format!("Total {}", ev), false);
        shared::value_pair_plain(ui, c, rect, &cols.evs, a.end.ev_total(), b.end.ev_total());
    });
}

// ---------------------------------------------------------------------------

fn exp_sources_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    let (a, b) = (&cmp.a, &cmp.b);
    let (ta, tb) = (a.totals.xp_total(), b.totals.xp_total());
    let right = if ta == tb {
        format!("{} exp gained in each route", widgets::fmt_thousands(ta))
    } else {
        format!("A {} \u{b7} B {}", widgets::fmt_thousands(ta), widgets::fmt_thousands(tb))
    };
    let colors = [
        xpr_ui_kit::theme::lighten(theme.bg, 0.42),
        xpr_ui_kit::theme::lighten(theme.bg, 0.25),
        xpr_ui_kit::theme::lighten(xpr_ui_kit::theme::parse_hex("#61520f"), 0.35),
    ];
    widgets::card(ui, theme, None, |ui| {
        widgets::card_title_colored(ui, theme, "Where the exp came from", Some(&right), theme.text_strong());
        for (side, d) in [(Side::A, a), (Side::B, b)] {
            let t = &d.totals;
            let total = t.xp_total().max(1) as f32;
            let parts = [t.xp_from_trainers, t.xp_from_wild, t.xp_from_candies];
            let rect = widgets::table_row(ui, 30.0, None);
            let badge = Rect::from_min_size(egui::Pos2::new(rect.min.x, rect.center().y - 10.0), Vec2::splat(20.0));
            widgets::paint_route_badge(ui, theme, badge, side);

            let pct = format!(
                "{} \u{b7} {} \u{b7} {} %",
                (parts[0] as f32 / total * 100.0).round(),
                (parts[1] as f32 / total * 100.0).round(),
                (parts[2] as f32 / total * 100.0).round()
            );
            let pw = 90.0;
            let bar = Rect::from_min_max(
                egui::Pos2::new(badge.max.x + 10.0, rect.center().y - 8.0),
                egui::Pos2::new(rect.max.x - pw - 10.0, rect.center().y + 8.0),
            );
            let segments: Vec<(f32, Color32)> = parts.iter().zip(colors).map(|(v, col)| (*v as f32 / total, col)).collect();
            widgets::stacked_bar(ui, theme, bar, &segments);
            widgets::col_text(ui, rect, &pct, theme.caption_font(), theme.secondary, Align::Max);

            ui.interact(bar, ui.id().with(("xp_tip", side.letter())), Sense::hover()).on_hover_text(format!(
                "Trainers {}\nWild {}\nRare Candy {}",
                widgets::fmt_thousands(parts[0]),
                widgets::fmt_thousands(parts[1]),
                widgets::fmt_thousands(parts[2])
            ));
        }
        // legend
        let rect = widgets::table_row(ui, 18.0, None);
        let mut x = rect.min.x;
        for (label, color) in ["Trainers", "Wild", "Rare Candy"].iter().zip(colors) {
            let sw = Rect::from_min_size(egui::Pos2::new(x, rect.center().y - 4.0), Vec2::splat(8.0));
            ui.painter().rect_filled(sw, egui::CornerRadius::same(2), color);
            let w = widgets::text_width(ui, label, &theme.caption_font());
            widgets::col_text(
                ui,
                Rect::from_min_size(egui::Pos2::new(x + 13.0, rect.min.y), Vec2::new(w, rect.height())),
                label,
                theme.caption_font(),
                theme.secondary,
                Align::Min,
            );
            x += 13.0 + w + 14.0;
        }
    });
}

// ---------------------------------------------------------------------------

/// One row of the route-totals table.
struct TotalRow {
    name: &'static str,
    a: i64,
    b: i64,
    sub: bool,
    detail: String,
}

fn total_rows(cmp: &RouteComparison) -> Vec<(&'static str, Vec<TotalRow>)> {
    let (x, y) = (&cmp.a.totals, &cmp.b.totals);
    let r = |name: &'static str, a: i64, b: i64| TotalRow { name, a, b, sub: false, detail: String::new() };
    let sub = |name: &'static str, a: i64, b: i64| TotalRow { name, a, b, sub: true, detail: String::new() };
    let vitamin_detail = {
        let fmt = |d: &RouteDigest| {
            d.totals
                .vitamins_by_name
                .iter()
                .map(|(n, c)| format!("{} \u{d7}{}", n, c))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (fa, fb) = (fmt(&cmp.a), fmt(&cmp.b));
        match (fa.is_empty(), fb.is_empty()) {
            (true, true) => String::new(),
            (false, true) => format!("A: {}", fa),
            (true, false) => format!("B: {}", fb),
            (false, false) => format!("A: {} \u{b7} B: {}", fa, fb),
        }
    };
    vec![
        (
            "Battles",
            vec![
                r("Trainers fought", x.trainers, y.trainers),
                sub("of which major fights", x.major_fights, y.major_fights),
                r("Trainer Pokémon defeated", x.trainer_pokemon, y.trainer_pokemon),
                r("Wild Pokémon fought", x.wild_pokemon, y.wild_pokemon),
                sub("different species", x.wild_species, y.wild_species),
                sub("trainer-owned singles", x.trainer_owned_singles, y.trainer_owned_singles),
            ],
        ),
        (
            "Levelling",
            vec![
                r("Final level", cmp.a.end.level, cmp.b.end.level),
                r("Rare Candies used", x.rare_candies, y.rare_candies),
                TotalRow {
                    name: "Vitamins used",
                    a: x.vitamins,
                    b: y.vitamins,
                    sub: false,
                    detail: vitamin_detail,
                },
                r("EV berries used", x.ev_berries, y.ev_berries),
            ],
        ),
        (
            "Moves",
            vec![
                r("Moves learned", x.moves_learned(), y.moves_learned()),
                sub("by level-up", x.moves_level_up, y.moves_level_up),
                sub("by TM / HM", x.moves_tm_hm, y.moves_tm_hm),
                sub("by tutor or deleter", x.moves_tutor, y.moves_tutor),
            ],
        ),
        (
            "Items",
            vec![
                r("Items picked up", x.items_picked_up, y.items_picked_up),
                r("Purchases", x.purchases, y.purchases),
                r("Sales", x.sales, y.sales),
                r("Items used or dropped", x.items_used, y.items_used),
                r("Held item changes", x.held_item_changes, y.held_item_changes),
            ],
        ),
        (
            "Other",
            vec![
                r("Pokémon Center heals", x.heals, y.heals),
                r("Saves", x.saves, y.saves),
                r("Blackouts", x.blackouts, y.blackouts),
                r("Evolutions", x.evolutions, y.evolutions),
                r("Events with errors", x.events_with_errors, y.events_with_errors),
            ],
        ),
    ]
}

fn totals_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    widgets::card(ui, theme, None, |ui| {
        let title = widgets::card_title_colored(ui, theme, "Route totals", Some("enabled events only"), theme.text_strong());
        let cols = shared::totals_col_heads(ui, theme, title);
        for (group, rows) in total_rows(cmp) {
            shared::group_heading(ui, theme, group);
            for (i, row) in rows.iter().enumerate() {
                // Keep the first row of a group even when it is zero, so the
                // group is never an empty heading.
                if i > 0 && row.a == 0 && row.b == 0 {
                    continue;
                }
                let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
                shared::name_cell(ui, theme, rect, row.name, row.sub);
                shared::value_pair(ui, c, rect, &cols.values, row.a, row.b);
                let m = row.a.max(row.b).max(1) as f32;
                widgets::paired_bars(ui, theme, cols.bars(rect), row.a as f32 / m, row.b as f32 / m);
                if !row.detail.is_empty() {
                    widgets::col_text(ui, cols.detail(rect), &row.detail, theme.caption_font(), theme.secondary, Align::Min);
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------

fn money_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    let (x, y) = (&cmp.a.totals, &cmp.b.totals);
    let right = if cmp.a.start.money == cmp.b.start.money {
        format!("start {}", shared::money(cmp.a.start.money))
    } else {
        format!("start A {} \u{b7} B {}", shared::money(cmp.a.start.money), shared::money(cmp.b.start.money))
    };
    widgets::card(ui, theme, None, |ui| {
        let title = widgets::card_title_colored(ui, theme, "Money", Some(&right), theme.text_strong());
        let cols = shared::three_col_heads(ui, theme, title, "");
        let rows: [(&str, i64, i64); 5] = [
            ("Won from trainers", x.money_from_trainers, y.money_from_trainers),
            ("Sales", x.money_from_sales, y.money_from_sales),
            ("Purchases", x.money_spent, y.money_spent),
            ("Lost to blackouts", x.money_lost_blackouts, y.money_lost_blackouts),
            ("Other", x.money_other, y.money_other),
        ];
        for (name, a, b) in rows {
            if a == 0 && b == 0 {
                continue;
            }
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            shared::name_cell(ui, theme, rect, name, false);
            widgets::col_text(ui, cols.a(rect), &shared::money(a), theme.body(), theme.text, Align::Max);
            widgets::col_text(ui, cols.b(rect), &shared::money(b), theme.body(), theme.text, Align::Max);
            shared::money_delta_cell(ui, c, cols.delta(rect), b - a);
        }
        let rect = widgets::table_row(ui, ROW_H, Some(theme.pane_divider()));
        shared::name_cell(ui, theme, rect, "Final money", false);
        let (fa, fb) = (cmp.a.end.money, cmp.b.end.money);
        let font = if c.diff(fa != fb) { theme.font_bold(10.5) } else { theme.body() };
        let color = if c.diff(fa != fb) { theme.text_strong() } else { theme.secondary };
        widgets::col_text(ui, cols.a(rect), &shared::money(fa), font.clone(), color, Align::Max);
        widgets::col_text(ui, cols.b(rect), &shared::money(fb), font, color, Align::Max);
        shared::money_delta_cell(ui, c, cols.delta(rect), fb - fa);
    });
}

fn final_moves_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    widgets::card(ui, theme, None, |ui| {
        widgets::card_title_colored(ui, theme, "Final moves", Some("bold = not in the other route"), theme.text_strong());
        let (sa, sb) = (cmp.a.end.move_set(), cmp.b.end.move_set());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let col_w = (ui.available_width() - 12.0) / 2.0;
            for (side, d, other) in [(Side::A, &cmp.a, &sb), (Side::B, &cmp.b, &sa)] {
                ui.allocate_ui_with_layout(Vec2::new(col_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                    ui.set_width(col_w);
                    let rect = widgets::table_row(ui, 24.0, None);
                    let badge = Rect::from_min_size(rect.min, Vec2::splat(20.0));
                    widgets::paint_route_badge(ui, theme, badge, side);
                    for (i, m) in d.end.moves.iter().enumerate() {
                        let rect = widgets::table_row(ui, 30.0, Some(theme.row_divider()));
                        widgets::col_text(
                            ui,
                            Rect::from_min_size(rect.min, Vec2::new(14.0, rect.height())),
                            &format!("{}", i + 1),
                            theme.caption_font(),
                            theme.secondary,
                            Align::Min,
                        );
                        let name_rect = Rect::from_min_max(egui::Pos2::new(rect.min.x + 22.0, rect.min.y), rect.max);
                        match m.as_deref().filter(|s| !s.is_empty()) {
                            Some(name) => {
                                let shared_move = other.contains(name);
                                let (font, color) = if shared_move || !c.highlight {
                                    (theme.body(), theme.text)
                                } else {
                                    (theme.body_bold(), theme.text_strong())
                                };
                                let text = hidden_power_label(name, d);
                                widgets::col_text(ui, name_rect, &text, font, color, Align::Min);
                            }
                            None => widgets::col_text(ui, name_rect, "\u{2014}", theme.body(), theme.secondary, Align::Min),
                        }
                    }
                });
            }
        });
    });
}

/// `Hidden Power` carries its type, as it does everywhere else in the app.
fn hidden_power_label(name: &str, d: &RouteDigest) -> String {
    match (&d.hidden_power, name == xpr_core::consts::HIDDEN_POWER_MOVE_NAME) {
        (Some((t, _)), true) if !t.is_empty() => format!("{} ({})", name, t),
        _ => name.to_string(),
    }
}

/// One row of the Trainers card.
struct SetRow<'a> {
    label: &'a str,
    count: usize,
    side: Option<Side>,
    names: Option<&'a Vec<String>>,
}

fn trainers_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) -> Option<String> {
    let theme = c.theme;
    let s = &cmp.trainer_sets;
    let mut jump = None;
    let right = format!("{} vs {}", cmp.a.totals.trainers, cmp.b.totals.trainers);
    widgets::card(ui, theme, None, |ui| {
        widgets::card_title_colored(ui, theme, "Trainers", Some(&right), theme.text_strong());
        let rows: Vec<SetRow> = vec![
            SetRow { label: "Fought in both, same order", count: s.same_order, side: None, names: None },
            SetRow { label: "Fought in both, different order", count: s.different_order, side: None, names: None },
            SetRow { label: "Only in A", count: s.only_a.len(), side: Some(Side::A), names: Some(&s.only_a) },
            SetRow { label: "Only in B", count: s.only_b.len(), side: Some(Side::B), names: Some(&s.only_b) },
            SetRow { label: "Fought twice in A", count: s.repeat_a.len(), side: Some(Side::A), names: Some(&s.repeat_a) },
            SetRow { label: "Fought twice in B", count: s.repeat_b.len(), side: Some(Side::B), names: Some(&s.repeat_b) },
        ];
        for (i, SetRow { label, count, side, names }) in rows.iter().enumerate() {
            if names.is_some() && *count == 0 {
                continue;
            }
            let id = ui.id().with(("trainer_set", i));
            let mut open = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
            let rect = widgets::table_row(ui, 30.0, None);
            let clickable = names.is_some();
            let resp = ui.interact(rect, id.with("row"), if clickable { Sense::click() } else { Sense::hover() });
            if clickable && resp.hovered() {
                ui.painter().rect_filled(rect, egui::CornerRadius::same(4), theme.hover_bg);
            }
            let mut x = rect.min.x;
            if clickable {
                let tri = Rect::from_min_size(egui::Pos2::new(x, rect.center().y - 6.0), Vec2::splat(12.0));
                widgets::paint_disclosure_chevron(ui, tri, open, theme.secondary);
            }
            x += 16.0;
            if let Some(side) = side {
                let badge = Rect::from_min_size(egui::Pos2::new(x, rect.center().y - 10.0), Vec2::splat(20.0));
                widgets::paint_route_badge(ui, theme, badge, *side);
                x += 26.0;
            }
            widgets::col_text(
                ui,
                Rect::from_min_max(egui::Pos2::new(x, rect.min.y), egui::Pos2::new(rect.max.x - 44.0, rect.max.y)),
                label,
                theme.body(),
                theme.text,
                Align::Min,
            );
            widgets::col_text(ui, rect, &count.to_string(), theme.font_bold(10.5), theme.text_strong(), Align::Max);
            if resp.clicked() {
                open = !open;
                ui.ctx().data_mut(|d| d.insert_temp(id, open));
            }
            if open {
                if let Some(names) = names {
                    if let Some(name) = name_chips(ui, theme, names) {
                        jump = Some(name);
                    }
                }
            }
        }
    });
    jump
}

/// Wrapping chips; clicking one asks the caller to jump to that fight.
fn name_chips(ui: &mut Ui, theme: &Theme, names: &[String]) -> Option<String> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for name in names {
            let w = widgets::text_width(ui, name, &theme.caption_font()) + 14.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
            if ui.is_rect_visible(rect) {
                let bg = if resp.hovered() { theme.hover_bg } else { theme.well_bg() };
                ui.painter().rect(rect, egui::CornerRadius::same(4), bg, egui::Stroke::new(1.0_f32, theme.card_border()), egui::StrokeKind::Inside);
                widgets::col_text(ui, rect, name, theme.caption_font(), theme.text, Align::Center);
            }
            if resp.on_hover_text("Show in the event diff").clicked() {
                clicked = Some(name.clone());
            }
        }
    });
    ui.add_space(4.0);
    clicked
}

// ---------------------------------------------------------------------------

fn moves_learned_card(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison) {
    let theme = c.theme;
    #[derive(Default)]
    struct Row {
        source_a: Option<MoveSource>,
        source_b: Option<MoveSource>,
        levels_a: Vec<i64>,
        levels_b: Vec<i64>,
    }
    let mut order: Vec<String> = Vec::new();
    let mut rows: std::collections::HashMap<String, Row> = std::collections::HashMap::new();
    for (is_a, d) in [(true, &cmp.a), (false, &cmp.b)] {
        for m in &d.moves_learned {
            let entry = rows.entry(m.name.clone()).or_insert_with(|| {
                order.push(m.name.clone());
                Row::default()
            });
            if is_a {
                entry.source_a.get_or_insert_with(|| m.source.clone());
                entry.levels_a.push(m.level);
            } else {
                entry.source_b.get_or_insert_with(|| m.source.clone());
                entry.levels_b.push(m.level);
            }
        }
    }
    // Ordered by the earlier of the two levels.
    order.sort_by_key(|n| {
        let r = &rows[n];
        r.levels_a.iter().chain(r.levels_b.iter()).copied().min().unwrap_or(i64::MAX)
    });

    widgets::card(ui, theme, None, |ui| {
        let title = widgets::card_title_colored(ui, theme, "Moves learned", Some("level when learned"), theme.text_strong());
        let cols = shared::moves_col_heads(ui, theme, title);
        if order.is_empty() {
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            widgets::col_text(ui, rect, "Neither route learned a move.", theme.body(), theme.secondary, Align::Min);
        }
        for name in &order {
            let r = &rows[name];
            let fmt = |levels: &Vec<i64>| {
                if levels.is_empty() {
                    "\u{2014}".to_string()
                } else {
                    levels.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ")
                }
            };
            let (la, lb) = (fmt(&r.levels_a), fmt(&r.levels_b));
            let same = la == lb;
            let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
            let (font, color) = if same || !c.highlight {
                (theme.body(), theme.text)
            } else {
                (theme.body_bold(), theme.text_strong())
            };
            widgets::col_text(ui, cols.name(rect), name, font, color, Align::Min);
            let source = match (&r.source_a, &r.source_b) {
                (Some(a), Some(b)) if a == b => a.short_label(),
                (Some(a), Some(b)) => format!("{} / {}", a.short_label(), b.short_label()),
                (Some(a), None) => a.short_label(),
                (None, Some(b)) => b.short_label(),
                (None, None) => String::new(),
            };
            widgets::col_text(ui, cols.source(rect), &source, theme.caption_font(), theme.secondary, Align::Min);
            let value_color = if same { theme.secondary } else { theme.text };
            widgets::col_text(ui, cols.a(rect), &la, theme.body(), value_color, Align::Max);
            widgets::col_text(ui, cols.b(rect), &lb, theme.body(), value_color, Align::Max);
        }
    });
}
