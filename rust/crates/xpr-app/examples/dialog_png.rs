//! Render modal dialogs to PNGs, headless, through the app's software
//! rasteriser: each one over the route list of a test route, cropped to the
//! dialog plus a margin of backdrop. For reviewing dialog styling without a
//! window.
//!
//! ```text
//! cargo run -p xpr-app --example dialog_png -- <out_dir> [name ...]
//! ```
//!
//! Names: message (the unsaved-changes prompt on quit), close_route,
//! new_route_from_current, load_route, new_folder, transfer, custom_dvs, custom_gen,
//! battle_config, color_config, highlight_colors, app_config, shortcuts,
//! final_trainers, matchup_export, assign_move, ev_override, blocked.
//! With no names, all of them.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Id, Pos2, Rect, Vec2};
use xpr_app::controller::MainController;
use xpr_app::dialogs::*;
use xpr_app::route_list::{ListActions, RouteList};
use xpr_app::screenshot::Offscreen;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::theme::Theme;

const ALL: &[&str] = &[
    "message", "close_route", "new_route_from_current", "load_route", "new_folder", "transfer", "custom_dvs", "custom_gen", "battle_config", "color_config",
    "highlight_colors", "app_config", "shortcuts", "final_trainers", "matchup_export", "assign_move", "ev_override", "blocked",
];

enum Shown {
    Dialog(Box<Dialog>, &'static str),
    Message(MessageBox),
    Blocked,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out_dir) = args.first().map(PathBuf::from) else {
        eprintln!("usage: dialog_png <out_dir> [name ...]");
        std::process::exit(2);
    };
    std::fs::create_dir_all(&out_dir).expect("create the output directory");
    let names: Vec<String> = if args.len() > 1 { args[1..].to_vec() } else { ALL.iter().map(|s| s.to_string()).collect() };

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let mut cfg = Config::load(&xpr_app::scratch_config_path());
    let theme = Theme::from_config(&cfg);
    let paths = Paths::new(root.clone());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry.clone(), paths.clone());
    ctrl.load_route(&root.join("tests/test_data/yellow-pinsir-lv10brock.json"));
    let _ = ctrl.take_signals();
    let gen = ctrl.gen().expect("the route loads");
    let size = Vec2::new(1100.0, 780.0);

    for name in &names {
        let mut shown = match name.as_str() {
            "message" => Shown::Message(MessageBox::new(
                "Unsaved Changes",
                "Route has unsaved changes. Save before quitting?",
                MsgButtons::SaveDiscardCancel { save: "Save, then quit", discard: "Quit without saving" },
                MsgTag::QuitUnsaved,
            )),
            "new_route_from_current" => Shown::Message(MessageBox::new(
                "Unsaved Changes",
                "Route has unsaved changes. Save before starting a new route?",
                MsgButtons::SaveDiscardCancel { save: "Save, then start new route", discard: "Start without saving" },
                MsgTag::NewRouteFromCurrentUnsaved,
            )),
            "close_route" => Shown::Message(MessageBox::new(
                "Unsaved Changes",
                "Route has unsaved changes. Save before closing?",
                MsgButtons::SaveDiscardCancel { save: "Save, then close", discard: "Close without saving" },
                MsgTag::CloseRouteUnsaved,
            )),
            "load_route" => Shown::Dialog(Box::new(Dialog::LoadRoute(LoadRouteDialog::new(&paths))), "xpr_load_route"),
            "new_folder" => Shown::Dialog(Box::new(Dialog::NewFolder(NewFolderDialog::new(vec!["Pewter City".into(), "Route 3".into()], None, None))), "xpr_new_folder"),
            "transfer" => Shown::Dialog(Box::new(Dialog::Transfer(TransferDialog::new(vec!["Pewter City".into(), "Route 3".into(), "Mt. Moon".into()], vec!["Route 3".into(), "Mt. Moon".into()], vec![]))), "xpr_transfer"),
            "custom_dvs" => Shown::Dialog(Box::new(Dialog::CustomDvs(CustomDvsDialog::new(&gen, &ctrl))), "xpr_custom_dvs"),
            "custom_gen" => Shown::Dialog(Box::new(Dialog::CustomGen(CustomGenDialog::new(&registry))), "xpr_custom_gen"),
            "battle_config" => Shown::Dialog(Box::new(Dialog::BattleConfig(BattleConfigDialog::new(&cfg))), "xpr_battle_config"),
            "color_config" => Shown::Dialog(Box::new(Dialog::ColorConfig(ColorConfigDialog::new(&cfg))), "xpr_color_config"),
            "highlight_colors" => Shown::Dialog(Box::new(Dialog::HighlightColors(HighlightColorDialog::new())), "xpr_highlight_colors"),
            "app_config" => Shown::Dialog(Box::new(Dialog::AppConfig(AppConfigDialog::new(&cfg, false))), "xpr_app_config"),
            "shortcuts" => Shown::Dialog(Box::new(Dialog::Shortcuts(ShortcutsDialog::new(&cfg))), "xpr_shortcuts"),
            "final_trainers" => Shown::Dialog(Box::new(Dialog::FinalTrainers(FinalTrainersDialog::new(&cfg, &registry, ctrl.get_version().map(|s| s.to_string()).as_deref()))), "xpr_final_trainers"),
            "matchup_export" => Shown::Dialog(Box::new(Dialog::MatchupExport(MatchupExportDialog { mon_idx: 0 })), "xpr_matchup_export"),
            "assign_move" => Shown::Dialog(Box::new(Dialog::AssignMove(AssignMoveDialog::new(&gen, 1, false))), "xpr_assign_move"),
            "ev_override" => Shown::Dialog(Box::new(Dialog::EvOverride(EvOverrideDialog::new(&gen, [0, 4, 0, 12, 0, 8]))), "xpr_ev_override"),
            "blocked" => Shown::Blocked,
            other => {
                eprintln!("unknown dialog {other:?}; known: {}", ALL.join(", "));
                continue;
            }
        };
        let area_id = match &shown {
            Shown::Dialog(_, id) => *id,
            Shown::Message(_) => "xpr_message_box",
            Shown::Blocked => "xpr_secondary_window_blocked",
        };
        // 2 px per point: the picture is as crisp as on a HiDPI screen
        let mut off = Offscreen::new(&theme, 2.0, 8192);
        let mut list = RouteList::new(&cfg);
        let mut dialog_rect = None;
        let prims = off.render(size, 6, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg).inner_margin(8)).show(ctx, |ui| {
                let mut actions = ListActions::default();
                list.ui(ui, &theme, &cfg, &mut ctrl, &mut actions, false);
            });
            let mut message = None;
            match &mut shown {
                Shown::Dialog(d, _) => {
                    let mut dctx = DialogCtx { theme: &theme, cfg: &mut cfg, ctrl: &mut ctrl, registry: &registry, paths: &paths };
                    let _ = d.ui(ctx, &mut dctx, &mut message);
                }
                Shown::Message(m) => {
                    let _ = m.ui(ctx, &theme);
                }
                Shown::Blocked => {
                    // as in a native secondary window
                    ctx.set_embed_viewports(false);
                    block_secondary_window(ctx, &theme);
                }
            }
            dialog_rect = ctx.memory(|m| m.area_rect(Id::new(area_id)));
        });
        let full = Rect::from_min_size(Pos2::ZERO, size);
        let crop = dialog_rect.map(|r| r.expand(28.0).intersect(full)).unwrap_or(full);
        let canvas = off.rasterize(&prims, crop).expect("rasterise");
        let out = out_dir.join(format!("{name}.png"));
        canvas.save(&out).expect("write png");
        println!("wrote {} ({} x {} px)", out.display(), canvas.width, canvas.height);
    }
}
