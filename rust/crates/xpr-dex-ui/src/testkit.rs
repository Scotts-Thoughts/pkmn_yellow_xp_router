//! Headless helpers for tests and the `dex_png` example: a theme, the
//! router registry, and a driver that runs the Dex page in an offscreen
//! egui context, feeds it clicks / keys, collects the drawn text and saves
//! PNGs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Shape, Vec2};
use xpr_core::Config;
use xpr_data::Registry;
use xpr_ui_kit::offscreen::Offscreen;
use xpr_ui_kit::theme::Theme;

use crate::{DexAction, DexHost, DexView, NoHost, RouteContext};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// A config nobody else uses (the router's `Config` writes on every setter).
pub fn scratch_config() -> Config {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = repo_root().join("rust/target/xpr-scratch-config");
    let _ = std::fs::create_dir_all(&dir);
    Config::load(&dir.join(format!(
        "dex-{}-{}.json",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )))
}

pub fn test_theme() -> Theme {
    Theme::from_config(&scratch_config())
}

/// The router's built-in versions (no custom gens).
pub fn test_registry() -> Arc<Registry> {
    Arc::new(Registry::new(
        repo_root().join("raw_pkmn_data"),
        PathBuf::new(),
    ))
}

/// The Dex page in an offscreen context.
pub struct DexDriver {
    pub view: DexView,
    pub theme: Theme,
    pub registry: Arc<Registry>,
    pub route: RouteContext,
    pub host: Box<dyn DexHost>,
    pub size: Vec2,
    pub off: Offscreen,
    /// every text drawn in the last frame
    pub texts: Vec<String>,
    /// actions of every frame since the last `take_actions`
    pub actions: Vec<DexAction>,
    last_prims: Vec<egui::epaint::ClippedPrimitive>,
}

impl DexDriver {
    /// `settings` is a `dex_settings` JSON (partial is fine).
    pub fn new(settings: serde_json::Value, size: Vec2) -> DexDriver {
        let theme = test_theme();
        let off = Offscreen::new(&theme, 1.0, 8192);
        DexDriver {
            view: DexView::new(settings),
            theme,
            registry: test_registry(),
            route: RouteContext::default(),
            host: Box::new(NoHost),
            size,
            off,
            texts: Vec::new(),
            actions: Vec::new(),
            last_prims: Vec::new(),
        }
    }

    /// Run `passes` frames with `events` delivered in the first.
    pub fn frame_with(&mut self, passes: usize, events: Vec<Event>) {
        let DexDriver {
            view,
            theme,
            registry,
            route,
            host,
            size,
            off,
            actions,
            ..
        } = self;
        let prims = off.render_with_events(*size, passes, events, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(crate::palette::GRAY_900))
                .show(ctx, |ui| {
                    actions.extend(view.ui(ui, theme, registry, route, host.as_mut()));
                });
        });
        let mut texts = Vec::new();
        for cs in self.off.last_shapes() {
            collect_texts(&cs.shape, &mut texts);
        }
        self.texts = texts;
        self.last_prims = prims;
    }

    pub fn frame(&mut self, passes: usize) {
        self.frame_with(passes, Vec::new());
    }

    /// Move the pointer to `pos` and settle.
    pub fn hover(&mut self, pos: Pos2) {
        self.frame_with(2, vec![Event::PointerMoved(pos)]);
    }

    /// A primary click at `pos` (press, then release a frame later).
    pub fn click(&mut self, pos: Pos2) {
        self.click_button(pos, PointerButton::Primary);
    }

    pub fn right_click(&mut self, pos: Pos2) {
        self.click_button(pos, PointerButton::Secondary);
    }

    fn click_button(&mut self, pos: Pos2, button: PointerButton) {
        // hover first: a press in the frame the pointer arrives would start a
        // drag of whatever scroll area is under it
        self.frame_with(1, vec![Event::PointerMoved(pos)]);
        self.frame_with(
            1,
            vec![
                Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        self.frame_with(
            2,
            vec![Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }

    /// Press and release a key.
    pub fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame_with(
            1,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
        );
        self.frame_with(
            2,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            }],
        );
    }

    /// Type text into whatever has focus.
    pub fn type_text(&mut self, text: &str) {
        self.frame_with(2, vec![Event::Text(text.to_string())]);
    }

    /// Whether some drawn text contains `needle`.
    pub fn shows(&self, needle: &str) -> bool {
        self.texts.iter().any(|t| t.contains(needle))
    }

    pub fn take_actions(&mut self) -> Vec<DexAction> {
        std::mem::take(&mut self.actions)
    }

    /// Save the last frame as a PNG.
    pub fn save_png(&mut self, path: &Path) -> Result<(), String> {
        let canvas = self
            .off
            .rasterize(&self.last_prims, Rect::from_min_size(Pos2::ZERO, self.size))?;
        canvas.save(path)
    }
}

fn collect_texts(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.text().to_string()),
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        _ => {}
    }
}
