//! The world-map viewer (`docs/rust_port/design/world_map/SPEC.md`): a
//! pannable, zoomable view of the game's maps fed by `xpr-map`, with
//! object markers, info cards, route actions ("add to route") and
//! "show on map" focusing. Hosted either as the Map tab of the right pane
//! or in its own window (`app.rs`).

pub mod camera;
pub mod cards;
pub mod chunks;
pub mod export;
pub mod finder;
pub mod item_search;
pub mod keynav;
pub mod layers;
pub mod layers_menu;
pub mod list;
pub mod navigator;
pub mod overlay;
pub mod route_path;
pub mod sprites;
pub mod state;
pub mod tools;

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Pos2, Rect, Sense, Ui, Vec2};
use serde_json::Value;
use xpr_core::Config;
use xpr_data::GenData;
use xpr_engine::NodeId;
use xpr_map::{game_for_version, geom, Anchor, Compositor, LinkQuery, MapId, MapKind, MapPack, ObjectGrid, ObjectKind, PackSource, Precision, Scope};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Entry};

use crate::assets::Assets;
use crate::controller::MainController;

use camera::Camera;
use cards::{Card, CardCtx};
use chunks::ChunkCache;
use list::MapList;
use sprites::SpriteAtlas;
use state::RouteMapState;

pub const TOOLBAR_H: f32 = 30.0;
pub const STATUS_H: f32 = 18.0;

/// What the map asks the window to do.
#[derive(Clone, Debug, PartialEq)]
pub enum MapAction {
    AddTrainer { name: String },
    AddItem { name: String },
    AddWild { species: String, level: i64 },
    AddAllTrainers { map: MapId },
    /// Trainers by router name into a new folder (the marquee selection's "add trainers").
    AddTrainersNamed { folder: String, names: Vec<String> },
    SelectEvent(NodeId),
    /// A map image export finished; the window's toast offers the folder.
    Exported(PathBuf),
    ExportFailed(String),
    /// The map view / selection was copied to the clipboard.
    CopiedImage,
    Undock,
    Dock,
    Close,
}

#[derive(Clone, Copy, Debug)]
pub struct Toggles {
    pub trainers: bool,
    pub items: bool,
    pub hidden: bool,
    pub warps: bool,
    pub signs: bool,
    pub berries: bool,
    pub npcs: bool,
    /// draw overworld sprites instead of circles (when zoomed in enough)
    pub sprites: bool,
    /// step / block grid lines and map outlines (`overlay::draw_grid`)
    pub grid: bool,
    /// map names over the world (`overlay::draw_map_labels`)
    pub labels: bool,
    /// the route's order drawn over its anchors (`route_path::draw`)
    pub path: bool,
    /// the navigator minimap (`navigator`)
    pub navigator: bool,
    /// the camera follows the selected route event (`keynav::on_route_synced`)
    pub follow: bool,
}

impl Default for Toggles {
    fn default() -> Self {
        Toggles { trainers: true, items: true, hidden: true, warps: true, signs: false, berries: true, npcs: false, sprites: true, grid: false, labels: true, path: false, navigator: true, follow: false }
    }
}

impl Toggles {
    pub fn visible(&self, k: ObjectKind) -> bool {
        match k {
            ObjectKind::Trainer => self.trainers,
            ObjectKind::Item => self.items,
            ObjectKind::HiddenItem => self.hidden,
            ObjectKind::Warp => self.warps,
            ObjectKind::Sign => self.signs,
            ObjectKind::Berry => self.berries,
            ObjectKind::Npc => self.npcs,
        }
    }
    fn to_json(self) -> Value {
        serde_json::json!({"trainers": self.trainers, "items": self.items, "hidden": self.hidden, "warps": self.warps, "signs": self.signs, "berries": self.berries, "npcs": self.npcs, "sprites": self.sprites, "grid": self.grid, "labels": self.labels, "path": self.path, "navigator": self.navigator, "follow": self.follow})
    }
    fn from_json(v: &Value) -> Toggles {
        let d = Toggles::default();
        let g = |k: &str, def: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(def);
        Toggles { trainers: g("trainers", d.trainers), items: g("items", d.items), hidden: g("hidden", d.hidden), warps: g("warps", d.warps), signs: g("signs", d.signs), berries: g("berries", d.berries), npcs: g("npcs", d.npcs), sprites: g("sprites", d.sprites), grid: g("grid", d.grid), labels: g("labels", d.labels), path: g("path", d.path), navigator: g("navigator", d.navigator), follow: g("follow", d.follow) }
    }
}

struct Focus {
    anchors: Vec<Anchor>,
    idx: usize,
    label: String,
    started: Instant,
}

struct Loaded {
    pack: Arc<MapPack>,
    comp: Arc<Compositor>,
    grid: Arc<ObjectGrid>,
}

pub struct MapView {
    /// the map is shown (tab or window)
    pub open: bool,
    pub docked: bool,
    game: Option<String>,
    loaded: Option<Loaded>,
    loading: Option<(String, Receiver<Result<Loaded, String>>)>,
    load_error: Option<String>,
    source: PackSource,
    camera: Camera,
    scope: Scope,
    chunks: ChunkCache,
    atlas: SpriteAtlas,
    pub toggles: Toggles,
    pub night: bool,
    hover: Option<u32>,
    selected: Option<u32>,
    card: Option<(Card, Vec2)>,
    focus: Option<Focus>,
    /// `pub` so headless tests can drive the search popup directly
    /// (`tests/map_navigation.rs`, WP-E gap G10) without synthetic text
    /// events into the toolbar's search box.
    pub list: MapList,
    /// the toolbar's "Find an item…" box (`item_search.rs`)
    pub item_search: item_search::ItemSearch,
    pub state: RouteMapState,
    view_states: Value,
    view_dirty: Option<Instant>,
    last_vp: Rect,
    pointer_world: Option<(MapId, i32, i32, bool, bool)>,
    /// how far the pointer has strayed from the press during the current
    /// primary drag on the viewport; a drag that never left the click radius
    /// is still a click when the button comes up
    drag_travel: Option<f32>,
    /// where the open card was drawn last frame
    card_rect: Option<Rect>,
    frame_log: bool,
    last_frame_ms: f32,
    pending_fit: bool,
    /// a "show on map" request made before the pack finished loading
    pending_focus: Option<(LinkQuery, String)>,
    /// a transient message in the status strip
    notice: Option<(String, Instant)>,
    /// marquee selection / measure tools (`tools.rs`)
    pub tools: tools::ToolState,
    /// the navigator minimap (`navigator.rs`)
    pub navigator: navigator::Navigator,
    /// the export dialog and its worker (`export.rs`)
    pub export: export::ExportUi,
}

impl MapView {
    pub fn new(cfg: &Config, raw_pkmn_data: &std::path::Path) -> MapView {
        let dir = MapPack::default_dir(raw_pkmn_data);
        MapView {
            open: cfg.get_map_open(),
            docked: cfg.get_map_docked(),
            game: None,
            loaded: None,
            loading: None,
            load_error: None,
            source: PackSource::new(dir),
            camera: Camera::default(),
            scope: Scope::World,
            chunks: ChunkCache::new(cfg.get_map_texture_budget_mb()),
            atlas: SpriteAtlas::default(),
            toggles: Toggles::from_json(&cfg.get_map_toggles()),
            night: cfg.get_map_night(),
            hover: None,
            selected: None,
            card: None,
            focus: None,
            list: MapList::default(),
            item_search: item_search::ItemSearch::default(),
            state: RouteMapState::default(),
            view_states: cfg.get_map_view_state(),
            view_dirty: None,
            last_vp: Rect::NOTHING,
            pointer_world: None,
            drag_travel: None,
            card_rect: None,
            frame_log: std::env::var_os("XPR_FRAME_LOG").is_some(),
            last_frame_ms: 0.0,
            pending_fit: false,
            pending_focus: None,
            notice: None,
            tools: tools::ToolState::default(),
            navigator: navigator::Navigator::default(),
            export: export::ExportUi::default(),
        }
    }

    pub fn pack(&self) -> Option<&Arc<MapPack>> {
        self.loaded.as_ref().map(|l| &l.pack)
    }

    pub fn is_ready(&self) -> bool {
        self.loaded.is_some()
    }

    pub fn source_dir(&self) -> Option<&PathBuf> {
        self.source.dir.as_ref()
    }

    /// The game to show for a controller's version (custom gens use their base).
    pub fn game_of(ctrl: &MainController) -> Option<&'static str> {
        let gen: Option<Arc<GenData>> = ctrl.gen();
        match gen {
            Some(g) => Self::game_of_gen(&g),
            None => game_for_version(ctrl.get_version()?),
        }
    }

    /// The game to show for a gen (custom gens use their base); `None` when
    /// there is no map pack for it (gens 1-3 only), which hides the Map tab.
    pub fn game_of_gen(gen: &GenData) -> Option<&'static str> {
        game_for_version(gen.base_version_name().unwrap_or(gen.version_name()))
    }

    /// Start loading the game's pack if it is not the current one.
    pub fn ensure_game(&mut self, ctx: &egui::Context, game: Option<&str>) {
        let Some(game) = game else { return };
        if self.game.as_deref() == Some(game) {
            return;
        }
        if let Some((g, _)) = &self.loading {
            if g == game {
                return;
            }
        }
        self.save_view_state_now();
        self.game = Some(game.to_string());
        self.loaded = None;
        self.load_error = None;
        self.card = None;
        self.focus = None;
        self.hover = None;
        self.selected = None;
        self.chunks.clear();
        self.atlas.clear();
        let (tx, rx) = channel();
        let source = self.source.clone();
        let game_s = game.to_string();
        let wake = ctx.clone();
        std::thread::spawn(move || {
            let r = MapPack::load(&game_s, &source).map(|p| {
                let pack = Arc::new(p);
                let comp = Arc::new(Compositor::new(pack.clone()));
                let grid = Arc::new(ObjectGrid::build(&pack));
                Loaded { pack, comp, grid }
            });
            let _ = tx.send(r.map_err(|e| e.to_string()));
            wake.request_repaint();
        });
        self.loading = Some((game.to_string(), rx));
    }

    fn poll_load(&mut self, ctrl: &MainController) {
        let Some((_, rx)) = &self.loading else { return };
        match rx.try_recv() {
            Ok(Ok(loaded)) => {
                self.chunks.set_compositor(loaded.comp.clone());
                self.loaded = Some(loaded);
                self.loading = None;
                self.scope = Scope::World;
                self.pending_fit = true;
                let pack = self.loaded.as_ref().map(|l| l.pack.clone());
                self.state.sync(ctrl, pack.as_deref());
                // warm the overview levels so zooming out never shows holes
                if let Some(p) = self.pack() {
                    let keys = layers::overview_keys(p, Scope::World, self.night, 3);
                    self.chunks.request(keys);
                }
                if let Some((q, label)) = self.pending_focus.take() {
                    self.request_focus(q, label);
                }
            }
            Ok(Err(e)) => {
                self.load_error = Some(e);
                self.loading = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(_) => {
                self.loading = None;
            }
        }
    }

    /// Route / selection changed: refresh the routed-state overlay.
    pub fn sync_route(&mut self, ctrl: &MainController) {
        let pack = self.loaded.as_ref().map(|l| l.pack.clone());
        self.state.sync(ctrl, pack.as_deref());
        keynav::on_route_synced(self);
    }

    // ---- navigation ------------------------------------------------------------------

    fn scope_of_map(pack: &MapPack, id: MapId) -> Scope {
        if pack.map(id).map(|m| m.kind == MapKind::Outdoor && m.world_pos.is_some()).unwrap_or(false) {
            Scope::World
        } else {
            Scope::Map(id)
        }
    }

    fn set_scope(&mut self, scope: Scope) {
        if self.scope != scope {
            self.scope = scope;
            self.card = None;
            self.hover = None;
        }
    }

    /// Go to a map: centre on it in the world, or open it on its own.
    pub fn navigate_to(&mut self, id: MapId, animate: bool) {
        let Some(pack) = self.pack().cloned() else { return };
        let vp = self.last_vp;
        let scope = Self::scope_of_map(&pack, id);
        self.set_scope(scope);
        match scope {
            Scope::World => {
                if let Some(r) = geom::map_world_rect(&pack, id) {
                    let zoom = self.camera.zoom.max(1.0);
                    if vp.width() > 0.0 {
                        self.camera.set_min_zoom_for(vp, geom::scope_rect(&pack, Scope::World));
                    }
                    self.camera.go_to(Vec2::new((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0), zoom, animate);
                }
            }
            Scope::Map(_) => {
                let r = geom::scope_rect(&pack, scope);
                if vp.width() > 0.0 {
                    self.camera.set_min_zoom_for(vp, r);
                    self.camera.fit(vp, r, false);
                    if self.camera.zoom > 3.0 {
                        let c = self.camera.center;
                        self.camera.go_to(c, 3.0, false);
                    }
                } else {
                    self.pending_fit = true;
                }
            }
        }
        self.mark_view_dirty();
    }

    pub fn back_to_world(&mut self) {
        let Some(pack) = self.pack().cloned() else { return };
        let vp = self.last_vp;
        self.set_scope(Scope::World);
        if vp.width() > 0.0 {
            self.camera.set_min_zoom_for(vp, geom::scope_rect(&pack, Scope::World));
        }
        self.mark_view_dirty();
    }

    /// "Show on map": focus these anchors (cycled with the banner's arrows).
    pub fn focus_anchors(&mut self, anchors: Vec<Anchor>, label: String) {
        if anchors.is_empty() {
            return;
        }
        self.focus = Some(Focus { anchors, idx: 0, label, started: Instant::now() });
        self.go_to_focus(true);
    }

    fn go_to_focus(&mut self, animate: bool) {
        let Some(pack) = self.pack().cloned() else { return };
        let Some(f) = &self.focus else { return };
        let a = f.anchors[f.idx].clone();
        let scope = Self::scope_of_map(&pack, a.map);
        self.set_scope(scope);
        let vp = self.last_vp;
        if vp.width() > 0.0 {
            self.camera.set_min_zoom_for(vp, geom::scope_rect(&pack, scope));
        }
        let zoom = self.camera.zoom.clamp(2.0, 4.0);
        if a.precision == Precision::Map {
            if let Some(m) = pack.map(a.map) {
                let (ox, oy) = geom::map_origin_px(&pack, scope, a.map).unwrap_or((0, 0));
                let (w, h) = geom::map_px_size(&pack.geom, m);
                let center = Vec2::new(ox as f32 + w as f32 / 2.0, oy as f32 + h as f32 / 2.0);
                self.camera.go_to(center, zoom.min(2.0), animate);
            }
        } else if let Some((wx, wy)) = geom::step_center_px(&pack, scope, a.map, a.x as i32, a.y as i32) {
            self.camera.go_to(Vec2::new(wx as f32, wy as f32), zoom, animate);
        }
        self.selected = a.object;
        self.card = None;
        if let Some(f) = self.focus.as_mut() {
            f.started = Instant::now();
        }
        self.mark_view_dirty();
    }

    pub fn clear_focus(&mut self) {
        self.focus = None;
    }

    /// Focus what a route event refers to. `Some(true)`: focused;
    /// `Some(false)`: the pack knows no position for it; `None`: the pack
    /// is still loading and the request is kept.
    pub fn request_focus(&mut self, q: LinkQuery, label: String) -> Option<bool> {
        let Some(pack) = self.pack().cloned() else {
            self.pending_focus = Some((q, label));
            return None;
        };
        let anchors = pack.resolve(&q);
        if anchors.is_empty() {
            self.notice = Some((format!("No map location known for {}", label), Instant::now()));
            return Some(false);
        }
        self.focus_anchors(anchors, label);
        Some(true)
    }

    // ---- persistence -----------------------------------------------------------------

    fn mark_view_dirty(&mut self) {
        self.view_dirty = Some(Instant::now() + Duration::from_millis(800));
    }

    fn view_state_json(&self) -> Option<Value> {
        let pack = self.pack()?;
        let (scope, map) = match self.scope {
            Scope::World => ("world", Value::Null),
            Scope::Map(id) => ("map", Value::String(pack.map(id)?.const_name.clone())),
        };
        Some(serde_json::json!({"scope": scope, "map": map, "zoom": self.camera.zoom, "cx": self.camera.center.x, "cy": self.camera.center.y}))
    }

    fn save_view_state_now(&mut self) {
        let Some(game) = self.game.clone() else { return };
        if let Some(v) = self.view_state_json() {
            if let Value::Object(o) = &mut self.view_states {
                o.insert(game, v);
            } else {
                let mut o = serde_json::Map::new();
                o.insert(game, v);
                self.view_states = Value::Object(o);
            }
        }
    }

    /// Persist toggles / view state when they changed (call once per frame).
    pub fn persist(&mut self, cfg: &mut Config) {
        if let Some(d) = self.view_dirty {
            if Instant::now() >= d {
                self.view_dirty = None;
                self.save_view_state_now();
                cfg.set_map_view_state(self.view_states.clone());
            }
        }
    }

    fn restore_view_state(&mut self, vp: Rect) -> bool {
        let Some(pack) = self.pack().cloned() else { return false };
        let Some(game) = self.game.clone() else { return false };
        let Some(v) = self.view_states.get(&game).cloned() else { return false };
        let scope = match v.get("scope").and_then(|s| s.as_str()) {
            Some("map") => v.get("map").and_then(|m| m.as_str()).and_then(|c| pack.map_by_const(c)).map(|m| Scope::Map(m.id)).unwrap_or(Scope::World),
            _ => Scope::World,
        };
        let zoom = v.get("zoom").and_then(|z| z.as_f64()).unwrap_or(2.0) as f32;
        let cx = v.get("cx").and_then(|z| z.as_f64()).unwrap_or(0.0) as f32;
        let cy = v.get("cy").and_then(|z| z.as_f64()).unwrap_or(0.0) as f32;
        self.scope = scope;
        self.camera.set_min_zoom_for(vp, geom::scope_rect(&pack, scope));
        self.camera.go_to(Vec2::new(cx, cy), zoom, false);
        true
    }

    // ---- ui --------------------------------------------------------------------------

    /// Draw the whole map pane (toolbar, viewport, status strip).
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &mut Config, ctrl: &MainController, assets: &mut Assets) -> Vec<MapAction> {
        let t0 = Instant::now();
        let mut actions = Vec::new();
        self.poll_load(ctrl);
        self.ensure_game(ui.ctx(), Self::game_of(ctrl));
        self.chunks.begin_frame();
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
        let full = ui.available_rect_before_wrap();
        let toolbar_rect = Rect::from_min_size(full.min, Vec2::new(full.width(), TOOLBAR_H));
        let status_rect = Rect::from_min_size(Pos2::new(full.min.x, full.max.y - STATUS_H), Vec2::new(full.width(), STATUS_H));
        let vp = Rect::from_min_max(Pos2::new(full.min.x, toolbar_rect.max.y), Pos2::new(full.max.x, status_rect.min.y));
        self.last_vp = vp;

        // toolbar
        let mut tui = ui.new_child(egui::UiBuilder::new().max_rect(toolbar_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let (list_anchor, item_anchor) = self.toolbar(&mut tui, theme, cfg, &mut actions);

        // viewport
        ui.painter().rect_filled(vp, 0.0, egui::Color32::from_rgb(10, 10, 24));
        let resp = ui.allocate_rect(vp, Sense::click_and_drag());
        match (&self.loaded, &self.loading, &self.load_error) {
            (Some(_), _, _) => self.viewport(ui, &resp, vp, theme, ctrl, assets, &mut actions),
            (None, Some(_), _) => {
                ui.painter().text(vp.center(), egui::Align2::CENTER_CENTER, "Loading map data…", theme.body(), theme.secondary);
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            (None, None, Some(e)) => {
                let msg = if self.game.is_none() { "No map data for this game".to_string() } else { format!("Map data unavailable: {}", e) };
                ui.painter().text(vp.center(), egui::Align2::CENTER_CENTER, msg, theme.body(), theme.secondary);
            }
            (None, None, None) => {
                let msg = if ctrl.get_version().is_none() { "Open a route to see its map" } else { "No map data for this game (gens 1–3 only)" };
                ui.painter().text(vp.center(), egui::Align2::CENTER_CENTER, msg, theme.body(), theme.secondary);
            }
        }

        // map list popup (over the viewport): a map jumps to it, a trainer
        // / item hit (WP-E gap G10) focuses its anchors through the banner
        if let (Some(anchor), Some(pack)) = (list_anchor, self.pack().cloned()) {
            match self.list.popup(ui, theme, &pack, ctrl, anchor, (vp.height() - 20.0).max(120.0)) {
                Some(list::Chosen::Map(id)) => self.navigate_to(id, true),
                Some(list::Chosen::Focus { label, anchors }) => self.focus_anchors(anchors, label),
                None => {}
            }
        }
        // item search popup: choosing an item loops the banner through its instances
        if let (Some(anchor), Some(pack)) = (item_anchor, self.pack().cloned()) {
            if let Some(name) = self.item_search.popup(ui, theme, &pack, anchor, (vp.height() - 20.0).max(120.0)) {
                self.choose_item(name);
            }
        }

        // status strip
        self.status_strip(ui, status_rect, theme);
        // the export dialog / a running export (`export.rs`; pushes Exported / CopiedImage)
        export::export_ui(self, ui, theme, cfg, ctrl, &mut actions);
        self.persist(cfg);
        if self.frame_log {
            self.last_frame_ms = t0.elapsed().as_secs_f32() * 1000.0;
        }
        actions
    }

    /// Returns the anchors of the map search and the item search popups.
    fn toolbar(&mut self, ui: &mut Ui, theme: &Theme, cfg: &mut Config, actions: &mut Vec<MapAction>) -> (Option<Rect>, Option<Rect>) {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
        ui.add_space(4.0);
        let pack = self.pack().cloned();
        let in_map = matches!(self.scope, Scope::Map(_));
        if widgets::StyledButton::new(theme, "◀ World").enabled(in_map).show(ui).clicked() {
            self.back_to_world();
        }
        let title = match (&pack, self.scope) {
            (Some(p), Scope::Map(id)) => p.map(id).map(|m| m.display.clone()).unwrap_or_default(),
            (Some(p), Scope::World) => match (p.gen, p.game.as_str()) {
                (1, _) => "Kanto".to_string(),
                (2, _) => "Johto & Kanto".to_string(),
                (_, "firered_leafgreen") => "Kanto & Sevii Islands".to_string(),
                _ => "Hoenn".to_string(),
            },
            _ => String::new(),
        };
        widgets::label_font(ui, widgets::elide(ui, &title, &theme.body_bold(), 140.0), theme.body_bold(), theme.text_strong());
        // search / map list
        let er = Entry::new(theme, &mut self.list.search).width(150.0).hint("Find a map…").id(ui.id().with("map_search")).show(ui);
        let anchor = er.response.as_ref().map(|r| r.rect);
        if er.changed || er.has_focus && ui.input(|i| i.pointer.any_click()) {
            self.list.open = true;
        }
        if let Some(r) = &er.response {
            if r.clicked() || r.gained_focus() {
                self.list.open = true;
            }
        }
        if er.escape_pressed {
            self.list.open = false;
        }
        // item search (`item_search.rs`): Enter picks the top match, then steps
        // through its instances
        let ir = Entry::new(theme, &mut self.item_search.query).width(130.0).hint("Find an item…").id(ui.id().with("map_item_search")).show(ui);
        let item_anchor = ir.response.as_ref().map(|r| r.rect);
        if ir.changed || ir.has_focus && ui.input(|i| i.pointer.any_click()) {
            self.item_search.open = true;
            self.list.open = false;
        }
        if let Some(r) = &ir.response {
            if r.clicked() || r.gained_focus() {
                self.item_search.open = true;
                self.list.open = false;
            }
        }
        if ir.escape_pressed {
            self.item_search.open = false;
        }
        if ir.enter_pressed {
            self.item_search_enter();
            if let Some(r) = &ir.response {
                r.request_focus();
            }
        }
        if self.list.open {
            self.item_search.open = false;
        }
        ui.add_space(6.0);
        // tools: pan / marquee / ruler (`tools.rs`)
        self.tools.toolbar(ui, theme);
        ui.add_space(6.0);
        // layer visibility, grid / labels / route-path overlays and the
        // navigator / follow panel toggles, consolidated into one popup
        // (`layers_menu.rs`): the chip row (8 object chips + 5 more for the
        // WP-C overlays) no longer fits the docked pane.
        layers_menu::show(ui, theme, &mut self.toggles, cfg);
        if pack.as_ref().map(|p| p.gen == 2).unwrap_or(false) {
            let r = widgets::StyledButton::new(theme, "Night").checked(self.night).show(ui);
            if r.clicked() {
                self.night = !self.night;
                cfg.set_map_night(self.night);
                self.chunks.clear();
                self.atlas.clear();
                if let Some(c) = self.loaded.as_ref().map(|l| l.comp.clone()) {
                    self.chunks.set_compositor(c);
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            if widgets::StyledButton::new(theme, "×").min_size(Vec2::new(22.0, 22.0)).show(ui).on_hover_text("Close the map").clicked() {
                actions.push(MapAction::Close);
            }
            let (glyph, tip) = if self.docked { ("⤢", "Open the map in its own window") } else { ("⤡", "Dock the map back into the editor") };
            if widgets::StyledButton::new(theme, glyph).min_size(Vec2::new(22.0, 22.0)).show(ui).on_hover_text(tip).clicked() {
                actions.push(if self.docked { MapAction::Undock } else { MapAction::Dock });
            }
            ui.add_space(6.0);
            if widgets::StyledButton::new(theme, "Export").enabled(pack.is_some()).show(ui).on_hover_text("Export the map as a PNG, or copy it").clicked() {
                self.export.request_open();
            }
            ui.add_space(6.0);
            if widgets::StyledButton::new(theme, "Fit").show(ui).clicked() {
                self.pending_fit = true;
            }
            if widgets::StyledButton::new(theme, "+").min_size(Vec2::new(22.0, 22.0)).show(ui).clicked() {
                let vp = self.last_vp;
                self.camera.zoom_at(vp, vp.center(), 1.3);
                self.mark_view_dirty();
            }
            // the zoom readout with its presets menu (`navigator::zoom_control`)
            let vp = self.last_vp;
            if navigator::zoom_control(ui, theme, &mut self.camera, vp) {
                self.mark_view_dirty();
            }
            if widgets::StyledButton::new(theme, "−").min_size(Vec2::new(22.0, 22.0)).show(ui).clicked() {
                let vp = self.last_vp;
                self.camera.zoom_at(vp, vp.center(), 1.0 / 1.3);
                self.mark_view_dirty();
            }
        });
        (anchor, item_anchor)
    }

    /// Enter in the item box: the next instance of the item being looped
    /// through, or else the first item matching the query.
    fn item_search_enter(&mut self) {
        let Some(pack) = self.pack().cloned() else { return };
        let looping = self.item_search.active.as_deref().filter(|a| self.focus_label() == Some(*a) && a.eq_ignore_ascii_case(self.item_search.query.trim()));
        if looping.is_some() {
            self.step_focus(true);
            return;
        }
        let hits = item_search::find(&pack, &self.item_search.query);
        let q = self.item_search.query.trim();
        if let Some(hit) = hits.iter().find(|h| h.name.eq_ignore_ascii_case(q)).or(hits.first()) {
            self.choose_item(hit.name.clone());
        }
    }

    /// Focus every instance of an item (balls and hidden items).
    pub fn choose_item(&mut self, name: String) {
        let Some(pack) = self.pack().cloned() else { return };
        let anchors = item_search::instances(&pack, &name);
        self.item_search.open = false;
        self.item_search.query = name.clone();
        self.item_search.active = Some(name.clone());
        self.focus_anchors(anchors, name);
    }

    /// Move the focus banner to its next (or previous) anchor.
    pub fn step_focus(&mut self, next: bool) {
        let Some(f) = self.focus.as_mut() else { return };
        let n = f.anchors.len();
        f.idx = if next { (f.idx + 1) % n } else { (f.idx + n - 1) % n };
        self.go_to_focus(true);
    }

    #[allow(clippy::too_many_arguments)]
    fn viewport(&mut self, ui: &mut Ui, resp: &egui::Response, vp: Rect, theme: &Theme, ctrl: &MainController, assets: &mut Assets, actions: &mut Vec<MapAction>) {
        let Some(loaded) = self.loaded.as_ref() else { return };
        let pack = loaded.pack.clone();
        let grid = loaded.grid.clone();
        let ctx = ui.ctx().clone();
        // egui's own Tab-driven focus grab (see `keynav::reclaim_hover_tab_focus`)
        // would otherwise hand the toolbar's search box keyboard focus on a
        // bare Tab press before any of the checks below run.
        keynav::reclaim_hover_tab_focus(ui, resp.hovered());
        let text_focused = ctx.memory(|m| m.focused().is_some());
        let scope = self.scope;
        let scope_rect = geom::scope_rect(&pack, scope);
        self.camera.set_min_zoom_for(vp, scope_rect);
        if self.pending_fit {
            self.pending_fit = false;
            if !self.restore_view_state(vp) {
                self.camera.fit(vp, scope_rect, false);
                if scope == Scope::World {
                    // start on the game's first town at a readable zoom
                    if let Some(&first) = pack.layout.draw_order.iter().find(|id| pack.map(**id).map(|m| m.const_name.contains("TOWN")).unwrap_or(false)) {
                        if let Some(r) = geom::map_world_rect(&pack, first) {
                            self.camera.go_to(Vec2::new((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0), 2.0, false);
                        }
                    }
                }
            }
        }
        if self.camera.tick() {
            ctx.request_repaint();
        }

        // ---- input ----
        // the navigator and the tools see the pointer first; when one of
        // them takes it the map neither pans nor hit-tests this frame
        let on_navigator = navigator::Navigator::handle_input(self, ui, resp, vp);
        let tool_used = !on_navigator && tools::ToolState::handle_input(self, ui, resp, vp);
        let pointer_taken = on_navigator || tool_used;
        // cards open when the button comes up, and only if the press didn't
        // pan the map. egui reports a click only for presses shorter than
        // 0.8 s and turns a longer hold into a drag, so a held press that
        // never moved counts as a click here too.
        if resp.drag_started_by(egui::PointerButton::Primary) {
            self.drag_travel = Some(0.0);
        }
        if let (Some(travel), Some(origin), Some(p)) = (self.drag_travel.as_mut(), ctx.input(|i| i.pointer.press_origin()), resp.interact_pointer_pos()) {
            *travel = travel.max(origin.distance(p));
        }
        let held_click = resp.drag_stopped_by(egui::PointerButton::Primary)
            && self.drag_travel.take().is_some_and(|t| t <= ctx.options(|o| o.input_options.max_click_dist));
        let clicked = resp.clicked() || held_click;
        if !pointer_taken && resp.dragged() && resp.drag_delta() != Vec2::ZERO {
            self.camera.pan(resp.drag_delta());
            self.list.open = false;
            self.mark_view_dirty();
        }
        if resp.hovered() {
            let (scroll, pinch, modifiers) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers));
            let pos = resp.hover_pos().unwrap_or(vp.center());
            if (pinch - 1.0).abs() > 1e-4 {
                self.camera.zoom_at(vp, pos, pinch);
                self.mark_view_dirty();
            } else if scroll.y.abs() > 0.0 {
                if modifiers.shift {
                    self.camera.pan(Vec2::new(scroll.y, scroll.x));
                } else {
                    let factor = (scroll.y * 0.004).exp().clamp(0.5, 2.0);
                    self.camera.zoom_at(vp, pos, factor);
                }
                self.mark_view_dirty();
            } else if scroll.x.abs() > 0.0 {
                self.camera.pan(Vec2::new(scroll.x, 0.0));
                self.mark_view_dirty();
            }
            if !text_focused && !behind_modal(ui) {
                // tool keys (H / M / R, `tools.rs`) and marker navigation (`keynav.rs`)
                self.tools.handle_keys(&ctx);
                keynav::handle_keys(self, ui, vp);
                // zoom presets: `1` = 100 % (`navigator.rs`, WP-D gap G8)
                navigator::Navigator::handle_keys(self, ui);
                let mut pan = Vec2::ZERO;
                ctx.input(|i| {
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        pan.x += 60.0;
                    }
                    if i.key_pressed(egui::Key::ArrowRight) {
                        pan.x -= 60.0;
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        pan.y += 60.0;
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        pan.y -= 60.0;
                    }
                });
                if pan != Vec2::ZERO {
                    self.camera.pan(pan);
                    self.mark_view_dirty();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
                    self.camera.zoom_at(vp, vp.center(), 1.3);
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
                    self.camera.zoom_at(vp, vp.center(), 1.0 / 1.3);
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Num0)) {
                    self.camera.fit(vp, scope_rect, true);
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) && matches!(scope, Scope::Map(_)) {
                    self.back_to_world();
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !text_focused && !behind_modal(ui) {
            // a selection / measurement goes first; the card and the focus banner on the next Escape
            if !self.tools.on_escape() {
                self.card = None;
                self.focus = None;
            }
        }
        self.camera.clamp_to(vp, scope_rect);

        // pointer -> world / object
        let pointer = if on_navigator { None } else { resp.hover_pos().map(|p| self.camera.screen_to_world(vp, p)) };
        self.pointer_world = None;
        self.hover = None;
        if let Some(w) = pointer {
            let (wx, wy) = (w.x.floor() as i32, w.y.floor() as i32);
            if let Some((map, lx, ly)) = geom::map_at_world_px(&pack, scope, wx, wy) {
                let bp = pack.geom.block_px as i32;
                let (grass, water) = pack.terrain_at(map, (lx / bp).max(0) as u32, (ly / bp).max(0) as u32);
                let s = pack.geom.step_px as i32;
                self.pointer_world = Some((map, lx / s, ly / s, grass, water));
            }
            let toggles = self.toggles;
            self.hover = grid.hit(&pack, scope, wx, wy, |o| toggles.visible(o.effective_kind()));
        }
        if self.hover.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        } else if resp.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        // clicks (a double-click on a warp follows it; every other click is an ordinary click)
        let mut consumed = false;
        if !pointer_taken && resp.double_clicked() {
            if let Some(idx) = self.hover {
                if let xpr_map::Payload::Warp { dest_map: Some(d), .. } = &pack.objects[idx as usize].payload {
                    if let Some(m) = pack.map_by_const(d) {
                        let id = m.id;
                        self.navigate_to(id, true);
                        consumed = true;
                    }
                }
            }
        }
        if consumed {
            return;
        }
        let path_click = !pointer_taken && route_path::handle_click(self, ui, resp, clicked, vp, actions);
        if !pointer_taken && !path_click && clicked {
            self.list.open = false;
            let hit = match (self.hover, pointer, self.pointer_world) {
                (Some(idx), Some(w), _) => Some((Card::Object { idx }, w)),
                (None, Some(w), Some((map, x, y, grass, water))) => {
                    Some((if grass || water { Card::Tile { map, x, y, water, grass } } else { Card::Map { map } }, w))
                }
                _ => None,
            };
            // clicking what the open card already shows closes it rather than moving it
            let toggle_off = matches!((&hit, &self.card), (Some((new, _)), Some((open, _))) if new.same_table(open));
            match hit {
                Some((card, w)) if !toggle_off => {
                    self.selected = match card {
                        Card::Object { idx } => Some(idx),
                        _ => None,
                    };
                    if self.selected.is_some() {
                        self.focus = None;
                    }
                    self.card = Some((card, w));
                }
                _ => {
                    self.selected = None;
                    self.card = None;
                }
            }
        }
        if !pointer_taken && resp.secondary_clicked() {
            self.card = None;
            self.selected = None;
        }

        // ---- draw ----
        let painter = ui.painter_at(vp);
        let wanted = layers::draw_base(&painter, vp, &self.camera, &pack, scope, self.night, &mut self.chunks);
        self.chunks.request(wanted);
        // overlays under the markers: the grid (`overlay.rs`)
        let oc = overlay::OverlayCtx { theme, pack: &pack, scope, cam: &self.camera, vp, ppp: ctx.pixels_per_point() };
        overlay::draw_grid(&painter, &oc, &self.toggles);
        let mc = layers::MarkerCtx { theme, toggles: &self.toggles, state: &self.state, hover: self.hover, selected: self.selected, night: self.night, ctx: &ctx };
        layers::draw_markers(&painter, vp, &self.camera, &pack, scope, &mc, &mut self.atlas);
        // overlays above the markers: map names (`overlay.rs`), the route path (`route_path.rs`), the tools' selection / measurement (`tools.rs`)
        overlay::draw_map_labels(&painter, &oc, &self.toggles);
        route_path::draw(&painter, &oc, &self.state, &self.toggles);
        self.tools.draw(&painter, &oc);
        // the marquee selection's floating toolbar ("Add trainers") queues
        // actions in `handle_input` rather than pushing them straight into
        // `actions` (it doesn't have that vec); drain them here (`tools.rs`)
        actions.extend(self.tools.take_actions());
        // the selected route event's anchors (when not focusing) ring quietly
        if self.focus.is_none() && !self.state.selected_anchors.is_empty() {
            let list: Vec<(MapId, i32, i32, bool)> = self.state.selected_anchors.iter().map(|a| (a.map, a.x as i32, a.y as i32, a.precision == Precision::Map)).collect();
            layers::draw_focus(&painter, vp, &self.camera, &pack, scope, &list, 0.0, theme);
        }
        if let Some(f) = &self.focus {
            let a = &f.anchors[f.idx];
            let list = vec![(a.map, a.x as i32, a.y as i32, a.precision == Precision::Map)];
            let t = f.started.elapsed().as_secs_f32();
            if layers::draw_focus(&painter, vp, &self.camera, &pack, scope, &list, t, theme) && t < 6.0 {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
        if self.chunks.poll(&ctx) {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // ---- card ----
        self.card_rect = None;
        if let Some((card, world_pos)) = self.card.clone() {
            let anchor = self.camera.world_to_screen(vp, world_pos);
            if !vp.expand(60.0).contains(anchor) {
                // scrolled far away: keep the card but pin it to the edge
            }
            let routed_event = match &card {
                Card::Object { idx } => cards::routed_event_for(ctrl, &pack, *idx),
                _ => None,
            };
            let mut ccx = CardCtx { theme, pack: &pack, gen: ctrl.gen(), version: ctrl.get_version().map(|s| s.to_string()), state: &self.state, ctrl, assets, routed_event };
            // kept off the navigator minimap (last frame's rect; it is drawn below)
            let avoid = self.navigator_rect();
            let out = cards::draw_card(ui, &card, anchor, vp, avoid, &mut ccx);
            self.card_rect = Some(out.rect);
            actions.extend(out.actions);
            if out.close {
                self.card = None;
                self.selected = None;
            }
            if let Some(id) = out.navigate {
                self.navigate_to(id, true);
            }
            if let Some(idx) = out.open_object {
                self.selected = Some(idx);
                keynav::open_selected_card(self);
            }
        }

        // ---- focus banner ----
        if let Some(f) = &self.focus {
            let n = f.anchors.len();
            let label = f.label.clone();
            let a = f.anchors[f.idx].clone();
            let map_name = pack.map(a.map).map(|m| m.display.clone()).unwrap_or_default();
            let mut where_ = match a.precision {
                Precision::Map => format!("{} (somewhere on this map)", map_name),
                _ => map_name,
            };
            match a.object.and_then(|i| pack.objects.get(i as usize)).map(|o| o.kind) {
                Some(ObjectKind::Item) => where_.push_str(" · item ball"),
                Some(ObjectKind::HiddenItem) => where_.push_str(" · hidden item"),
                _ => {}
            }
            let mut prev = false;
            let mut next = false;
            let mut close = false;
            let mut select = false;
            let banner_rect = Rect::from_min_size(Pos2::new(vp.min.x + 8.0, vp.max.y - 40.0), Vec2::new(vp.width() - 16.0, 32.0));
            egui::Area::new(ui.id().with("map_focus_banner")).order(egui::Order::Foreground).fixed_pos(banner_rect.min).show(&ctx, |ui| {
                egui::Frame::new().fill(theme.card_bg()).stroke(theme.border_stroke()).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                    ui.set_width(banner_rect.width() - 20.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                        widgets::label_font(ui, format!("Showing {}", label), theme.body_bold(), theme.text_strong());
                        widgets::label_font(ui, format!("— {} ({} of {})", where_, f.idx + 1, n), theme.body(), theme.secondary);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::StyledButton::new(theme, "×").min_size(Vec2::new(20.0, 20.0)).show(ui).clicked() {
                                close = true;
                            }
                            if let Some(id) = self.state.selected_id {
                                let _ = id;
                                if widgets::StyledButton::new(theme, "Select in list").show(ui).clicked() {
                                    select = true;
                                }
                            }
                            if widgets::StyledButton::new(theme, "▶").enabled(n > 1).min_size(Vec2::new(22.0, 20.0)).show(ui).clicked() {
                                next = true;
                            }
                            if widgets::StyledButton::new(theme, "◀").enabled(n > 1).min_size(Vec2::new(22.0, 20.0)).show(ui).clicked() {
                                prev = true;
                            }
                        });
                    });
                });
            });
            if close {
                self.focus = None;
            } else if prev || next {
                self.step_focus(next);
            }
            if select {
                if let Some(id) = self.state.selected_id {
                    actions.push(MapAction::SelectEvent(id));
                }
            }
        }

        // ---- navigator (`navigator.rs`; top-right, above the layers, cards and banner) ----
        navigator::Navigator::draw(self, ui, vp, theme);
    }

    fn status_strip(&self, ui: &mut Ui, rect: Rect, theme: &Theme) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme.bg_darker);
        let font = theme.caption_font();
        let mut left = String::new();
        if let Some((msg, at)) = &self.notice {
            if at.elapsed() < Duration::from_secs(4) {
                left = msg.clone();
            }
        }
        if left.is_empty() {
            // a selection / measurement readout (`tools.rs`)
            if let Some(t) = self.pack().and_then(|p| self.tools.status_text(p)) {
                left = t;
            }
        }
        if left.is_empty() {
        if let (Some(pack), Some((map, x, y, grass, water))) = (self.pack(), self.pointer_world) {
            let name = pack.map(map).map(|m| m.display.clone()).unwrap_or_default();
            let terrain = if water { " · water" } else if grass { " · grass" } else { "" };
            left = format!("{} · ({}, {}){}", name, x, y, terrain);
        }
        }
        painter.text(Pos2::new(rect.min.x + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, left, font.clone(), theme.secondary);
        let pending = self.chunks.pending_count();
        let mut right = format!("{} chunks · {} sprites", self.chunks.cached_count(), self.atlas.frame_count());
        if pending > 0 {
            right = format!("rendering {} · {}", pending, right);
        }
        if self.frame_log {
            right = format!("{} · {:.1} ms", right, self.last_frame_ms);
        }
        painter.text(Pos2::new(rect.max.x - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, right, font, theme.secondary);
    }

    // ---- read-only accessors (tests, status) -------------------------------------------

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn focus_label(&self) -> Option<&str> {
        self.focus.as_ref().map(|f| f.label.as_str())
    }

    /// The object whose card is open, if any.
    pub fn card_object(&self) -> Option<u32> {
        match &self.card {
            Some((Card::Object { idx }, _)) => Some(*idx),
            _ => None,
        }
    }

    pub fn card_is_map(&self) -> bool {
        matches!(&self.card, Some((Card::Map { .. }, _)))
    }

    pub fn card_is_tile(&self) -> bool {
        matches!(&self.card, Some((Card::Tile { .. }, _)))
    }

    /// Where the open card was drawn last frame (headless tests).
    pub fn card_rect(&self) -> Option<Rect> {
        self.card_rect
    }

    pub fn has_card(&self) -> bool {
        self.card.is_some()
    }

    /// Open the card a click on step `(x, y)` of `map` would open: the
    /// encounter card of a tall-grass / water step, the map card otherwise.
    /// False when the map is not in the current scope.
    pub fn open_card_at_step(&mut self, map: MapId, x: i32, y: i32) -> bool {
        let Some(pack) = self.pack().cloned() else { return false };
        let Some((wx, wy)) = geom::step_center_px(&pack, self.scope, map, x, y) else { return false };
        let spb = pack.geom.steps_per_block().max(1) as i32;
        let (grass, water) = pack.terrain_at(map, (x / spb).max(0) as u32, (y / spb).max(0) as u32);
        let card = if grass || water { Card::Tile { map, x, y, water, grass } } else { Card::Map { map } };
        self.card = Some((card, Vec2::new(wx as f32, wy as f32)));
        self.selected = None;
        self.focus = None;
        true
    }

    pub fn zoom(&self) -> f32 {
        self.camera.zoom
    }

    /// Sprite frames rendered into the atlas so far.
    pub fn sprite_frames(&self) -> usize {
        self.atlas.frame_count()
    }

    /// Where an object is on screen with the current camera (after a frame).
    pub fn object_screen_pos(&self, idx: u32) -> Option<Pos2> {
        let pack = self.pack()?;
        let (wx, wy) = geom::object_center_px(pack, self.scope, idx)?;
        Some(self.camera.world_to_screen(self.last_vp, Vec2::new(wx as f32, wy as f32)))
    }

    /// Where a step of a map is on screen with the current camera.
    pub fn step_screen_pos(&self, map: MapId, x: i32, y: i32) -> Option<Pos2> {
        let pack = self.pack()?;
        let (wx, wy) = geom::step_center_px(pack, self.scope, map, x, y)?;
        Some(self.camera.world_to_screen(self.last_vp, Vec2::new(wx as f32, wy as f32)))
    }

    /// Trainers of a map that are linked and not yet fought, in object order.
    pub fn trainers_to_add(&self, map: MapId) -> Vec<String> {
        let Some(pack) = self.pack() else { return Vec::new() };
        let mut out = Vec::new();
        for o in pack.objects_of(map) {
            if o.effective_kind() != ObjectKind::Trainer {
                continue;
            }
            if let Some(n) = o.trainer_names().first() {
                if !self.state.is_defeated(n) && !out.contains(n) {
                    out.push(n.clone());
                }
            }
        }
        out
    }

    pub fn map_display_name(&self, map: MapId) -> String {
        self.pack().and_then(|p| p.map(map)).map(|m| m.display.clone()).unwrap_or_default()
    }
}
