//! The species detail (Solodex `PokemonDetail.tsx`): a 256 px identity
//! column (artwork, typing, evolution family, base stats, type matchups,
//! meta, weight) and the movepool column beside it.

use std::collections::HashSet;

use egui::{
    Align, Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2,
};
use xpr_dex::{display_name, EvolutionEntry, PokemonData, StatKey};
use xpr_ui_kit::theme::Theme;

use super::common::{self, line_start, line_width, wrap_lines};
use super::context_menu::MenuCtx;
use super::effectiveness::type_effectiveness;
use super::growth::growth_popover;
use super::stats::{base_stats, BaseStatsArgs};
use crate::palette;
use crate::state::DexAction;
use crate::views::MovepoolPane;
use crate::widgets;
use crate::DexCx;

const LEFT_W: f32 = 256.0;
const PAD: f32 = 16.0;
const PINK_400: Color32 = Color32::from_rgb(0xf4, 0x72, 0xb6);

#[derive(Default)]
pub struct DetailState {
    /// "Filter comparison": rank within the species list's filter
    use_filtered: bool,
    /// the artwork lightbox is open
    lightbox: bool,
    last: Option<(String, String)>,
}

/// The sprite's scale by evolution stage (`PokemonDetail.tsx`'s `spriteScale`).
pub fn sprite_scale(p: &PokemonData) -> f32 {
    let family = &p.evolution_family;
    if family.len() <= 1 || xpr_dex::is_mega_form(&p.species) {
        return 1.0;
    }
    let evolved_from: HashSet<&str> = family
        .iter()
        .filter(|e| e.method.is_some())
        .map(|e| e.species.as_str())
        .collect();
    let evolves_into = family
        .iter()
        .any(|e| e.species != p.species && e.method.is_some());
    let is_evolved_from = evolved_from.contains(p.species.as_str());
    if evolves_into && !is_evolved_from {
        0.75
    } else if evolves_into && is_evolved_from {
        0.9
    } else {
        1.0
    }
}

/// The evolution family's arrow label ("Lv.16 \u{2192}", "Thunder Stone \u{2192}", "Mega \u{2192}").
pub fn evo_label(evo: &EvolutionEntry) -> Option<String> {
    let method = evo.method.as_deref()?;
    let text = match method {
        "level" => format!(
            "Lv.{}",
            evo.parameter
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default()
        ),
        "item" => evo
            .parameter
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "mega" => "Mega".to_string(),
        other => other.to_string(),
    };
    Some(format!("{} \u{2192}", text))
}

/// Grass Knot / Low Kick power for a weight in kg.
pub fn weight_power(weight: f64) -> u32 {
    if weight < 10.0 {
        20
    } else if weight < 25.0 {
        40
    } else if weight < 50.0 {
        60
    } else if weight < 100.0 {
        80
    } else if weight < 200.0 {
        100
    } else {
        120
    }
}

impl DetailState {
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx, movepool: Option<&mut dyn MovepoolPane>) {
        let theme = cx.theme;
        let rect = ui.available_rect_before_wrap();
        let Some(sel) = cx.state.selected().map(str::to_string) else {
            widgets::placeholder(ui, theme, "Select a Pok\u{e9}mon");
            return;
        };
        let game = cx.state.game().to_string();
        let Some(data) = xpr_dex::get_pokemon_data(&sel, &game) else {
            widgets::placeholder(
                ui,
                theme,
                &format!("{} is not in {}", display_name(&sel), game),
            );
            return;
        };
        if self.last.as_ref().map(|(s, g)| (s.as_str(), g.as_str()))
            != Some((sel.as_str(), game.as_str()))
        {
            self.lightbox = false;
            self.last = Some((sel.clone(), game.clone()));
        }
        let left =
            Rect::from_min_size(rect.min, Vec2::new(LEFT_W.min(rect.width()), rect.height()));
        let right = Rect::from_min_max(Pos2::new(left.max.x, rect.min.y), rect.max);
        // border-r
        ui.painter().vline(
            left.max.x - 0.5,
            left.y_range(),
            Stroke::new(1.0_f32, palette::GRAY_700),
        );
        let left_inner = Rect::from_min_max(left.min, Pos2::new(left.max.x - 1.0, left.max.y));
        let mut lui = ui.new_child(
            UiBuilder::new()
                .max_rect(left_inner)
                .layout(egui::Layout::top_down(Align::Min)),
        );
        lui.set_clip_rect(left_inner.intersect(ui.clip_rect()));
        self.left_column(&mut lui, cx, &data, &game);

        let mut rui = ui.new_child(
            UiBuilder::new()
                .max_rect(right)
                .layout(egui::Layout::top_down(Align::Min)),
        );
        rui.set_clip_rect(right.intersect(ui.clip_rect()));
        match movepool {
            Some(m) => m.movepool_ui(&mut rui, cx, &data, &game),
            None => widgets::placeholder(&mut rui, theme, "Movepool"),
        }
        ui.allocate_rect(rect, Sense::hover());
        if self.lightbox {
            self.lightbox_modal(ui.ctx(), cx, &data);
        }
    }

    fn lightbox_modal(&mut self, ctx: &egui::Context, cx: &mut DexCx, data: &PokemonData) {
        let screen = ctx.content_rect();
        let size = (screen.height().min(screen.width()) * 0.8)
            .min(560.0)
            .max(128.0);
        let modal = egui::Modal::new(Id::new("dex_artwork_lightbox"))
            .backdrop_color(Color32::from_black_alpha(204))
            .frame(egui::Frame::NONE);
        let resp = modal.show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            if let Some(tex) = cx.images.sprite(
                ctx,
                &data.species,
                data.national_dex_number,
                crate::SpriteSize::Full,
            ) {
                crate::images::paint_fit(ui, &tex, rect, Color32::WHITE);
            }
        });
        if resp.should_close() {
            self.lightbox = false;
        }
    }

    fn left_column(&mut self, ui: &mut Ui, cx: &mut DexCx, data: &PokemonData, game: &str) {
        let theme = cx.theme;
        let species = data.species.clone();
        let abilities: Vec<String> = {
            let mut seen = HashSet::new();
            data.abilities
                .iter()
                .filter(|a| seen.insert((*a).clone()))
                .cloned()
                .collect()
        };
        egui::ScrollArea::vertical()
            .id_salt("dex_detail_left")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let outer = ui.available_rect_before_wrap();
                let inner = Rect::from_min_max(
                    Pos2::new(outer.min.x + PAD, outer.min.y + PAD),
                    Pos2::new(outer.max.x - PAD, f32::INFINITY),
                );
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(inner)
                        .id_salt("dex_detail_left_inner"),
                    |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        self.identity(ui, cx, data, game);

                        // evolution family
                        if data.evolution_family.len() > 1 {
                            ui.add_space(PAD);
                            self.evolution_family(ui, cx, data, game);
                        }

                        // base stats
                        section_break(ui);
                        self.stats_section(ui, cx, data, game);

                        // effectiveness
                        section_break(ui);
                        heading_row(ui, theme, "Effectiveness");
                        ui.add_space(8.0);
                        type_effectiveness(ui, cx, &data.type_1, &data.type_2, game, &abilities);

                        // meta
                        section_break(ui);
                        self.meta(ui, cx, data, game, &abilities);

                        // weight
                        if let Some(weight) = data.weight {
                            section_break(ui);
                            heading_row(ui, theme, "Weight");
                            ui.add_space(8.0);
                            kv_text(
                                ui,
                                theme,
                                "Weight",
                                &format!("{} kg", weight),
                                palette::GRAY_300,
                            );
                            ui.add_space(4.0);
                            kv_text(
                                ui,
                                theme,
                                "Grass Knot / Low Kick",
                                &format!("{} BP", weight_power(weight)),
                                palette::GRAY_300,
                            );
                        }
                        ui.add_space(PAD);
                    },
                );
                let _ = species;
            });
    }

    /// Artwork with the name over it, the typing badges, Bulbapedia link.
    fn identity(&mut self, ui: &mut Ui, cx: &mut DexCx, data: &PokemonData, game: &str) {
        let theme = cx.theme;
        let w = ui.available_width();
        let (block, _) = ui.allocate_exact_size(Vec2::new(w, 160.0), Sense::hover());
        let sprite_box = Rect::from_center_size(
            Pos2::new(block.center().x, block.min.y + 72.0),
            Vec2::splat(144.0),
        );
        let resp = ui
            .interact(sprite_box, Id::new("dex_detail_artwork"), Sense::click())
            .on_hover_text("View full artwork")
            .on_hover_cursor(egui::CursorIcon::ZoomIn);
        if let Some(tex) = cx.images.sprite(
            ui.ctx(),
            &data.species,
            data.national_dex_number,
            crate::SpriteSize::Full,
        ) {
            let scale = sprite_scale(data);
            let r = Rect::from_center_size(sprite_box.center(), sprite_box.size() * scale);
            crate::images::paint_fit(
                ui,
                &tex,
                r,
                if resp.hovered() {
                    Color32::from_white_alpha(204)
                } else {
                    Color32::WHITE
                },
            );
        } else {
            ui.painter().text(
                sprite_box.center(),
                Align2::CENTER_CENTER,
                "?",
                palette::px(theme, 48.0),
                palette::GRAY_700,
            );
        }
        if resp.clicked() {
            self.lightbox = true;
        }
        // the name overlaps the top of the artwork (absolute -top-2, text-xl bold, centred)
        let name = display_name(&data.species);
        let mut job = egui::text::LayoutJob::single_section(
            name.to_string(),
            egui::TextFormat {
                font_id: palette::px_bold(theme, 20.0),
                color: Color32::WHITE,
                ..Default::default()
            },
        );
        job.wrap.max_width = w;
        job.halign = Align::Center;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        let name_pos = Pos2::new(block.center().x - g.size().x / 2.0, block.min.y - 8.0);
        let anchor = Pos2::new(block.center().x, block.min.y - 8.0);
        let _ = name_pos;
        // soft shadow, then the text
        let shadow = egui::text::LayoutJob::single_section(
            name.to_string(),
            egui::TextFormat {
                font_id: palette::px_bold(theme, 20.0),
                color: Color32::from_black_alpha(190),
                ..Default::default()
            },
        );
        let mut shadow = shadow;
        shadow.wrap.max_width = w;
        shadow.halign = Align::Center;
        let sg = ui.fonts_mut(|f| f.layout_job(shadow));
        let origin = Pos2::new(anchor.x - sg.rect.center().x, anchor.y);
        ui.painter()
            .galley(origin + Vec2::new(0.0, 1.0), sg.clone(), Color32::BLACK);
        ui.painter()
            .galley(origin + Vec2::new(0.0, 2.0), sg, Color32::BLACK);
        ui.painter().galley(origin, g, Color32::WHITE);
        // Bulbapedia (top right)
        let link = Rect::from_min_size(
            Pos2::new(block.max.x - 20.0, block.min.y),
            Vec2::splat(20.0),
        );
        let lresp = ui
            .interact(link, Id::new("dex_detail_bulbapedia"), Sense::click())
            .on_hover_text("Open on Bulbapedia")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        common::paint_external_link(
            ui.painter(),
            link,
            if lresp.hovered() {
                palette::GRAY_300
            } else {
                palette::GRAY_600
            },
        );
        if lresp.clicked() {
            cx.actions
                .push(DexAction::OpenUrl(common::species_url(&data.species)));
        }
        // typing, pulled up over the artwork's bottom edge (-mt-6)
        let types = data.types();
        let total = types.len() as f32 * 80.0 + (types.len() as f32 - 1.0) * 6.0;
        let mut x = block.center().x - total / 2.0;
        for t in types {
            let r = Rect::from_min_size(Pos2::new(x, block.min.y + 136.0), Vec2::new(80.0, 24.0));
            common::in_rect(ui, r, ("dex_detail_type", t), |ui| {
                widgets::type_badge(ui, theme, t, false, Some(game));
            });
            x += 86.0;
        }
    }

    fn evolution_family(&mut self, ui: &mut Ui, cx: &mut DexCx, data: &PokemonData, _game: &str) {
        let theme = cx.theme;
        let w = ui.available_width();
        let small = palette::px(theme, 12.0);
        let bold = palette::px_bold(theme, 12.0);
        struct Item<'a> {
            evo: &'a EvolutionEntry,
            label: Option<String>,
            label_w: f32,
            btn_w: f32,
            current: bool,
        }
        let items: Vec<Item> = data
            .evolution_family
            .iter()
            .enumerate()
            .map(|(i, evo)| {
                let label = if i > 0 { evo_label(evo) } else { None };
                let current = evo.species == data.species;
                let label_w = label
                    .as_ref()
                    .map(|l| widgets::text_w(ui, l, &small) + 4.0)
                    .unwrap_or(0.0);
                let btn_w = widgets::text_w(
                    ui,
                    display_name(&evo.species),
                    if current { &bold } else { &small },
                ) + 16.0;
                Item {
                    evo,
                    label,
                    label_w,
                    btn_w,
                    current,
                }
            })
            .collect();
        let widths: Vec<f32> = items.iter().map(|it| it.label_w + it.btn_w).collect();
        let lines = wrap_lines(&widths, w, 4.0);
        let height = lines.len() as f32 * 20.0 + (lines.len() as f32 - 1.0) * 4.0;
        let (block, _) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
        let mc = MenuCtx::of(cx.state);
        let mut select: Option<String> = None;
        for (li, line) in lines.iter().enumerate() {
            let y = block.min.y + li as f32 * 24.0;
            let mut x = line_start(
                block.min.x,
                w,
                line_width(&widths, line, 4.0),
                Align::Center,
            );
            for &i in line {
                let it = &items[i];
                if let Some(l) = &it.label {
                    ui.painter().text(
                        Pos2::new(x, y + 10.0),
                        Align2::LEFT_CENTER,
                        l,
                        small.clone(),
                        palette::GRAY_600,
                    );
                }
                let btn =
                    Rect::from_min_size(Pos2::new(x + it.label_w, y), Vec2::new(it.btn_w, 20.0));
                let resp = ui.interact(btn, Id::new(("dex_evo", i)), Sense::click());
                let (fill, fg) = if it.current {
                    (palette::GRAY_600, Color32::WHITE)
                } else if resp.hovered() {
                    (palette::GRAY_700, Color32::WHITE)
                } else {
                    (palette::GRAY_800, palette::GRAY_400)
                };
                ui.painter().rect_filled(btn, CornerRadius::same(4), fill);
                ui.painter().text(
                    btn.center(),
                    Align2::CENTER_CENTER,
                    display_name(&it.evo.species),
                    if it.current {
                        bold.clone()
                    } else {
                        small.clone()
                    },
                    fg,
                );
                if !it.current {
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        select = Some(it.evo.species.clone());
                    }
                    let target = it.evo.species.clone();
                    let state = &mut *cx.state;
                    let cur = display_name(&data.species);
                    let text = format!("Compare {} to {}", cur, display_name(&target));
                    widgets::context_menu(theme, &resp, |ui| {
                        if blue_menu_entry(ui, theme, &text) {
                            state.compare_with(&target);
                            ui.close();
                        }
                    });
                }
                x += it.label_w + it.btn_w + 4.0;
            }
        }
        let _ = mc;
        if let Some(s) = select {
            cx.state.select_species(&s);
        }
    }

    fn stats_section(&mut self, ui: &mut Ui, cx: &mut DexCx, data: &PokemonData, game: &str) {
        let theme = cx.theme;
        let w = ui.available_width();
        // header: "Base Stats" and the filter-comparison switch
        let (head, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
        common::paint_heading(ui, theme, head.min, "Base Stats", palette::GRAY_600);
        if !cx.state.filtered_names.is_empty() {
            let font = palette::px(theme, 12.0);
            let label = "Filter comparison";
            let lw = widgets::text_w(ui, label, &font);
            let total = 28.0 + 6.0 + lw;
            let r = Rect::from_min_size(
                Pos2::new(head.max.x - total, head.min.y),
                Vec2::new(total, 16.0),
            );
            let resp = ui
                .interact(r, Id::new("dex_filter_comparison"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let track = Rect::from_min_size(r.min, Vec2::new(28.0, 16.0));
            ui.painter().rect_filled(
                track,
                CornerRadius::same(8),
                if self.use_filtered {
                    palette::BLUE_500
                } else {
                    palette::GRAY_600
                },
            );
            let kx = if self.use_filtered {
                track.min.x + 14.0 + 6.0
            } else {
                track.min.x + 2.0 + 6.0
            };
            ui.painter()
                .circle_filled(Pos2::new(kx, track.center().y), 6.0, Color32::WHITE);
            ui.painter().text(
                Pos2::new(r.min.x + 34.0, r.center().y),
                Align2::LEFT_CENTER,
                label,
                font,
                palette::GRAY_500,
            );
            if resp.clicked() {
                self.use_filtered = !self.use_filtered;
            }
        }
        ui.add_space(8.0);
        let args = BaseStatsArgs {
            id: Id::new("dex_detail_stats"),
            stats: &data.base_stats,
            game,
            pokemon: &data.species,
            use_filtered: self.use_filtered,
            menu: MenuCtx::of(cx.state),
        };
        if let Some(name) = base_stats(ui, cx, &args) {
            cx.state.select_species(&name);
        }
    }

    fn meta(
        &mut self,
        ui: &mut Ui,
        cx: &mut DexCx,
        data: &PokemonData,
        game: &str,
        abilities: &[String],
    ) {
        let theme = cx.theme;
        let w = ui.available_width();
        let font = palette::px(theme, 14.0);
        let mut first = true;
        let mut gap = |ui: &mut Ui| {
            if !first {
                ui.add_space(4.0);
            }
            first = false;
        };

        // growth rate (click: the experience table)
        gap(ui);
        let (row, _) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::hover());
        ui.painter().text(
            row.left_center(),
            Align2::LEFT_CENTER,
            "Growth Rate",
            font.clone(),
            palette::GRAY_600,
        );
        let gw = widgets::text_w(ui, &data.growth_rate, &font);
        let grect = Rect::from_min_size(Pos2::new(row.max.x - gw, row.min.y), Vec2::new(gw, 20.0));
        let gresp = ui
            .interact(grect, Id::new("dex_growth_rate"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter().text(
            grect.left_center(),
            Align2::LEFT_CENTER,
            &data.growth_rate,
            font.clone(),
            if gresp.hovered() {
                Color32::WHITE
            } else {
                palette::GRAY_300
            },
        );
        growth_popover(theme, &gresp, &data.growth_rate);

        // abilities
        if !data.abilities.is_empty() {
            gap(ui);
            let label = if abilities.len() == 1 {
                "Ability"
            } else {
                "Abilities"
            };
            let lw = widgets::text_w(ui, label, &font);
            let avail = w - lw - 8.0;
            let comma_w = widgets::text_w(ui, ",", &font);
            let widths: Vec<f32> = abilities
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    widgets::text_w(ui, a, &font)
                        + if i + 1 < abilities.len() {
                            comma_w
                        } else {
                            0.0
                        }
                        + 0.0
                })
                .collect();
            // `gap-x-1` between the spans
            let lines = wrap_lines(&widths, avail, 4.0);
            let mut height = lines.len() as f32 * 20.0;
            if data.hidden_ability.is_some() {
                height += 2.0 + 20.0;
            }
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
            ui.painter().text(
                Pos2::new(row.min.x, row.min.y + 10.0),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                palette::GRAY_600,
            );
            for (li, line) in lines.iter().enumerate() {
                let y = row.min.y + li as f32 * 20.0;
                let mut x = line_start(
                    row.max.x - avail,
                    avail,
                    line_width(&widths, line, 4.0),
                    Align::Max,
                );
                for &i in line {
                    let a = &abilities[i];
                    let aw = widgets::text_w(ui, a, &font);
                    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(aw, 20.0));
                    let resp = ui
                        .interact(r, Id::new(("dex_ability", a.as_str())), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    ui.painter().text(
                        r.left_center(),
                        Align2::LEFT_CENTER,
                        a,
                        font.clone(),
                        if resp.hovered() {
                            palette::BLUE_400
                        } else {
                            palette::GRAY_300
                        },
                    );
                    ability_popover(cx, &resp, a);
                    if i + 1 < abilities.len() {
                        ui.painter().text(
                            Pos2::new(x + aw, y + 10.0),
                            Align2::LEFT_CENTER,
                            ",",
                            font.clone(),
                            palette::GRAY_600,
                        );
                    }
                    x += widths[i] + 4.0;
                }
            }
            if let Some(h) = &data.hidden_ability {
                let y = row.min.y + lines.len() as f32 * 20.0 + 2.0;
                let aw = widgets::text_w(ui, h, &font);
                let suffix = " (hidden)";
                let sw = widgets::text_w(ui, suffix, &font);
                let x = row.max.x - aw - sw;
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(aw, 20.0));
                let resp = ui
                    .interact(
                        r,
                        Id::new(("dex_hidden_ability", h.as_str())),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                ui.painter().text(
                    r.left_center(),
                    Align2::LEFT_CENTER,
                    h,
                    font.clone(),
                    if resp.hovered() {
                        palette::BLUE_400
                    } else {
                        palette::GRAY_500
                    },
                );
                ui.painter().text(
                    Pos2::new(x + aw, y + 10.0),
                    Align2::LEFT_CENTER,
                    suffix,
                    font.clone(),
                    palette::GRAY_500,
                );
                ability_popover(cx, &resp, h);
            }
        }

        // egg groups
        if let Some(g1) = data
            .egg_group_1
            .as_deref()
            .filter(|g| *g != "NoEggsDiscovered")
        {
            gap(ui);
            let mut groups: Vec<&str> = [Some(g1), data.egg_group_2.as_deref()]
                .into_iter()
                .flatten()
                .filter(|g| !g.is_empty() && *g != "NoEggsDiscovered")
                .collect();
            // a single-group species repeats its group in the data
            groups.dedup();
            kv_text(
                ui,
                theme,
                "Egg Groups",
                &groups.join(", "),
                palette::GRAY_300,
            );
        }
        if let Some(f) = data.base_friendship {
            gap(ui);
            kv_text(
                ui,
                theme,
                "Base Friendship",
                &f.to_string(),
                palette::GRAY_300,
            );
        }
        if let Some(ratio) = data.gender_ratio {
            gap(ui);
            let segs: Vec<(String, Color32)> = if ratio == 255.0 {
                vec![("Genderless".into(), palette::GRAY_300)]
            } else if ratio == 0.0 {
                vec![("100% \u{2642}".into(), palette::BLUE_400)]
            } else if ratio == 254.0 {
                vec![("100% \u{2640}".into(), PINK_400)]
            } else {
                vec![
                    (
                        format!("{:.1}% \u{2642}", (256.0 - ratio) / 256.0 * 100.0),
                        palette::BLUE_400,
                    ),
                    (" / ".into(), palette::GRAY_300),
                    (format!("{:.1}% \u{2640}", ratio / 256.0 * 100.0), PINK_400),
                ]
            };
            kv_segments(ui, theme, "Gender", &segs);
        }
        if xpr_dex::game_gen(game) >= 3 {
            gap(ui);
            let mut segs: Vec<(String, Color32)> = Vec::new();
            for key in StatKey::ALL {
                let v = data.ev_yield.get(key);
                if v > 0 {
                    if !segs.is_empty() {
                        segs.push((", ".into(), palette::GRAY_600));
                    }
                    segs.push((
                        format!("{} {}", v, palette::stat_label(key, false)),
                        palette::stat_color(key, false),
                    ));
                }
            }
            if segs.is_empty() {
                segs.push(("None".into(), palette::GRAY_500));
            }
            kv_segments(ui, theme, "EV Yield", &segs);
        }
        if let Some(item) = data.common_item.as_deref().filter(|i| !i.is_empty()) {
            gap(ui);
            kv_text(ui, theme, "Held Item", item, palette::GRAY_300);
        }
    }
}

/// `border-t border-gray-800` after a 16 px gap, then the `pt-3` padding.
fn section_break(ui: &mut Ui) {
    ui.add_space(PAD);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        r.x_range(),
        r.center().y,
        Stroke::new(1.0_f32, palette::GRAY_800),
    );
    ui.add_space(12.0);
}

/// A section heading row (16 px tall).
fn heading_row(ui: &mut Ui, theme: &Theme, text: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
    common::paint_heading(ui, theme, r.min, text, palette::GRAY_600);
}

/// A `flex justify-between` row of a gray label and a right-aligned value;
/// a value too wide for the row wraps below the label.
fn kv_text(ui: &mut Ui, theme: &Theme, label: &str, value: &str, color: Color32) {
    kv_segments(ui, theme, label, &[(value.to_string(), color)]);
}

fn kv_segments(ui: &mut Ui, theme: &Theme, label: &str, segs: &[(String, Color32)]) {
    let w = ui.available_width();
    let font = palette::px(theme, 14.0);
    let lw = widgets::text_w(ui, label, &font);
    let avail = (w - lw - 8.0).max(40.0);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = avail;
    for (t, c) in segs {
        job.append(
            t,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: *c,
                ..Default::default()
            },
        );
    }
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let h = g.size().y.max(20.0);
    let (row, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().text(
        Pos2::new(row.min.x, row.min.y + 10.0),
        Align2::LEFT_CENTER,
        label,
        font,
        palette::GRAY_600,
    );
    let pos = Pos2::new(row.max.x - g.size().x, row.min.y + (h - g.size().y) / 2.0);
    ui.painter().galley(pos, g, Color32::WHITE);
}

/// A context-menu entry that highlights blue (the evolution family's menu).
fn blue_menu_entry(ui: &mut Ui, theme: &Theme, text: &str) -> bool {
    let font = palette::px(theme, 14.0);
    let w = (widgets::text_w(ui, text, &font) + 24.0).max(180.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    let hot = resp.hovered();
    if hot {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(2), palette::BLUE_600);
    }
    ui.painter().text(
        Pos2::new(rect.min.x + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        if hot {
            Color32::WHITE
        } else {
            palette::GRAY_200
        },
    );
    resp.clicked()
}

/// Solodex's `WikiPopover` for an ability, minus the article text: the
/// name and a Bulbapedia link.
fn ability_popover(cx: &mut DexCx, anchor: &egui::Response, name: &str) {
    let theme = cx.theme;
    let actions = &mut *cx.actions;
    egui::Popup::from_toggle_button_response(anchor)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(widgets::popover_frame(theme))
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(16.0, 0.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(name)
                        .font(palette::px_bold(theme, 16.0))
                        .color(Color32::WHITE),
                );
                let font = palette::px(theme, 12.0);
                let label = "Bulbapedia";
                let lw = widgets::text_w(ui, label, &font);
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(lw + 16.0, 18.0), Sense::click());
                let resp = resp
                    .on_hover_text("Open on Bulbapedia")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let color = if resp.hovered() {
                    palette::BLUE_400
                } else {
                    palette::GRAY_500
                };
                ui.painter()
                    .text(rect.left_center(), Align2::LEFT_CENTER, label, font, color);
                common::paint_external_link(
                    ui.painter(),
                    Rect::from_center_size(
                        Pos2::new(rect.max.x - 6.0, rect.center().y),
                        Vec2::splat(12.0),
                    ),
                    color,
                );
                if resp.clicked() {
                    actions.push(DexAction::OpenUrl(common::ability_url(name)));
                    ui.close();
                }
            });
        });
}
