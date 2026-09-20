//! Port of `gui_qt/pkmn_components/route_list.py`: the virtualized tree
//! table of the route (folders / groups / level-up siblings / item children),
//! with selection, checkboxes, expand state, drag-and-drop, the hover "+"
//! button, the inline quantity editor, the inline event creator/editor and
//! the empty-state button.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_core::Config;
use xpr_engine::view::RowValues;
use xpr_engine::{EventDefinition, NodeId, ObjKind};
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets;

use crate::controller::MainController;
use crate::inline_creator::{key_for_event_type, InlineEventCreator, INLINE_ROW_HEIGHT};

pub const ROW_HEIGHT: f32 = 20.0;
/// Height of the pinned column-header strip above the rows.
const HEADER_HEIGHT: f32 = 22.0;
const INDENT: f32 = 16.0;
const CHECK_W: f32 = 16.0;
const BRANCH_W: f32 = 16.0;

/// (header, width; -1 = fit contents, 0 = stretch)
const COLUMN_DEFS: [(&str, f32); 9] = [
    ("Name", 325.0),
    ("Levels Up", 114.0),
    ("Level", 50.0),
    ("% TNL", -1.0),
    ("Exp", 48.0),
    ("Exp/sec", -1.0),
    ("Exp Gain", -1.0),
    ("ToNextLevel", -1.0),
    ("LvlsGained", -1.0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Folder,
    Group,
    Item,
    LevelUpSibling,
    InlineSpacer,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: NodeId,
    pub kind: RowKind,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
    /// the parent row's id (for parent-selected filtering)
    pub parent: Option<NodeId>,
}

#[derive(Clone, Debug)]
struct QuantityEditor {
    group_id: NodeId,
    text: String,
    rect: Rect,
    opened_frame: bool,
}

#[derive(Clone, Debug)]
struct DragState {
    ids: Vec<NodeId>,
    start: Pos2,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DropPos {
    Above,
    Below,
    On,
}

/// What the list asks the window to do this frame.
#[derive(Clone, Debug, Default)]
pub struct ListActions {
    pub suppress_battle_summary: Option<bool>,
    pub quick_add: Option<(Pos2, Option<usize>)>,
    pub refreshed: bool,
}

pub struct RouteList {
    rows: Vec<Row>,
    dirty: bool,
    /// Per-row column values and the fit-to-contents widths, computed when
    /// the rows are rebuilt (they only change with the route) rather than
    /// on every frame: `rows.len()` entries, plus the font they were
    /// measured with.
    row_values: Vec<RowValues>,
    fit_widths: Vec<f32>,
    fit_font: Option<(egui::FontId, egui::FontId)>,
    persistent_expand_state: HashMap<String, bool>,
    group_expanded: HashSet<NodeId>,
    selection: Vec<NodeId>,
    anchor: Option<NodeId>,
    hovered_row: Option<NodeId>,
    plus_visible_for: Option<NodeId>,
    plus_hide_deadline: Option<Instant>,
    scroll_target: Option<ScrollTarget>,
    quantity_editor: Option<QuantityEditor>,
    inline: Option<InlineEventCreator>,
    inline_inside_folder_id: Option<NodeId>,
    editing_group_id: Option<NodeId>,
    editing_original_enabled: Option<bool>,
    drag: Option<DragState>,
    drop_target: Option<(Option<NodeId>, DropPos)>,
    last_used_flatten_filter: bool,
    col_widths: Vec<f32>,
    highlight_colors: HashMap<String, Color32>,
    folder_fg: Option<Color32>,
    pub focused: bool,
    last_click: Option<(NodeId, Instant)>,
    /// The scroll position drawn last frame (`export_ui` reproduces it).
    scroll_offset: Vec2,
    /// Set while `export_ui` draws: the scroll position to draw at.
    export_scroll: Option<Vec2>,
    inline_counter: u64,
}

#[derive(Clone, Copy, Debug)]
enum ScrollTarget {
    Top,
    Bottom,
    Selected,
}

impl RouteList {
    pub fn new(cfg: &Config) -> RouteList {
        let mut rl = RouteList {
            rows: Vec::new(),
            dirty: true,
            row_values: Vec::new(),
            fit_widths: Vec::new(),
            fit_font: None,
            persistent_expand_state: HashMap::new(),
            group_expanded: HashSet::new(),
            selection: Vec::new(),
            anchor: None,
            hovered_row: None,
            plus_visible_for: None,
            plus_hide_deadline: None,
            scroll_target: None,
            quantity_editor: None,
            inline: None,
            inline_inside_folder_id: None,
            editing_group_id: None,
            editing_original_enabled: None,
            drag: None,
            drop_target: None,
            last_used_flatten_filter: false,
            col_widths: COLUMN_DEFS.iter().map(|(_, w)| *w).collect(),
            highlight_colors: HashMap::new(),
            folder_fg: None,
            focused: false,
            last_click: None,
            scroll_offset: Vec2::ZERO,
            export_scroll: None,
            inline_counter: 0,
        };
        rl.update_highlight_colors(cfg);
        rl.update_folder_text_style(cfg);
        rl
    }

    // ---- style caches ----------------------------------------------------------------

    pub fn update_folder_text_style(&mut self, cfg: &Config) {
        self.folder_fg = if cfg.get_fade_folder_text() { Some(Color32::from_rgb(0x66, 0x66, 0x66)) } else { None };
    }

    pub fn update_highlight_colors(&mut self, cfg: &Config) {
        self.highlight_colors.clear();
        for (idx, label) in consts::ALL_HIGHLIGHT_LABELS.iter().enumerate() {
            self.highlight_colors.insert(label.to_string(), theme::parse_hex(&cfg.get_highlight_color(idx as i64 + 1)));
        }
        for (cat, tag) in consts::FIGHT_CATEGORY_TO_TAG {
            self.highlight_colors.insert(tag.to_string(), theme::parse_hex(&cfg.get_fight_category_color(cat)));
        }
        self.highlight_colors.insert(consts::EVENT_TAG_ERRORS.to_string(), theme::parse_hex("#61520f"));
        // applied, but not as written (bag reorder slots): the configured warning
        // colour dimmed to a row tint, brighter than the olive error rows
        self.highlight_colors.insert(consts::EVENT_TAG_WARNINGS.to_string(), theme::darken(theme::parse_hex(cfg.get_warning_color()), 0.42));
        self.highlight_colors.insert(consts::EVENT_TAG_IMPORTANT.to_string(), theme::parse_hex("#1f1f1f"));
        self.highlight_colors.insert(consts::HIGHLIGHT_LABEL.to_string(), theme::parse_hex("#156152"));
        self.highlight_colors.insert(consts::EVENT_TAG_BRANCHED_MANDATORY.to_string(), theme::parse_hex("#5a5142"));
    }

    // ---- selection API ------------------------------------------------------------

    /// `get_all_selected_event_ids(allow_event_items)`: children whose parent is
    /// also selected are dropped.
    pub fn get_all_selected_event_ids(&self, ctrl: &MainController, allow_event_items: bool) -> Vec<NodeId> {
        let selected: HashSet<NodeId> = self.selection.iter().copied().collect();
        let mut result = Vec::new();
        for id in &self.selection {
            let kind = ctrl.router.obj_kind(*id);
            if !allow_event_items && kind == Some(ObjKind::Item) {
                continue;
            }
            let parent = match kind {
                Some(ObjKind::Item) => ctrl.router.item(*id).map(|i| i.parent),
                _ => ctrl.router.parent_of(*id),
            };
            if let Some(p) = parent {
                if selected.contains(&p) {
                    continue;
                }
            }
            result.push(*id);
        }
        result
    }

    pub fn set_all_selected_event_ids(&mut self, ids: &[NodeId]) {
        self.selection = ids.to_vec();
        self.anchor = ids.last().copied();
    }

    /// `scroll_to_selected_events`: Qt's `scrollTo()` is a no-op for a row
    /// hidden inside a collapsed parent, so the Qt list expands every
    /// collapsed ancestor of the (last) selected row first — the `expanded`
    /// signal then flips the folder's `expanded` flag like a click would.
    /// Without this a recorded event lands in a folder the user has collapsed
    /// and never comes into view.
    pub fn scroll_to_selected_events(&mut self, ctrl: &mut MainController) {
        if let Some(target) = self.selection.last().copied() {
            let mut ancestors: Vec<NodeId> = Vec::new();
            let mut cur = ctrl.router.parent_of(target);
            while let Some(a) = cur {
                if a == ctrl.router.root_id {
                    break;
                }
                ancestors.push(a);
                cur = ctrl.router.parent_of(a);
            }
            for anc in ancestors.into_iter().rev() {
                match ctrl.router.obj_kind(anc) {
                    Some(ObjKind::Folder) => {
                        let path = self.path_of_row(ctrl, anc);
                        let already = match self.persistent_expand_state.get(&path) {
                            Some(e) => *e,
                            None => ctrl.router.folder(anc).map(|f| f.is_expanded()).unwrap_or(false),
                        };
                        if !already {
                            if let Some(f) = ctrl.router.node_mut(anc).and_then(|n| n.as_folder_mut()) {
                                f.expanded = Some(true);
                            }
                            self.persistent_expand_state.insert(path, true);
                            self.dirty = true;
                        }
                    }
                    // an item row lives under its (expandable) group row
                    Some(ObjKind::Group) => {
                        if self.group_expanded.insert(anc) {
                            self.dirty = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.scroll_target = Some(ScrollTarget::Selected);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_target = Some(ScrollTarget::Top);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_target = Some(ScrollTarget::Bottom);
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `refresh()`: rebuild the visible rows.
    pub fn refresh(&mut self, ctrl: &MainController) {
        self.quantity_editor = None;
        self.dirty = false;
        let Some(gen) = ctrl.gen() else {
            self.rows.clear();
            return;
        };
        // snapshot expand state keyed by name path
        let mut saved: HashMap<String, bool> = std::mem::take(&mut self.persistent_expand_state);
        for (path, expanded) in self.collect_expand_state(ctrl) {
            saved.insert(path, expanded);
        }
        self.persistent_expand_state = saved;

        let cur_filter = ctrl.get_route_filter_types().map(|f| f.to_vec());
        let search = ctrl.get_route_search_string().map(|s| s.to_string());
        let flatten = cur_filter.as_ref().map(|f| f.iter().any(|t| t == consts::MAJOR_BATTLE_FILTER)).unwrap_or(false);
        self.last_used_flatten_filter = flatten;
        let mut rows: Vec<Row> = Vec::new();
        if flatten {
            for gid in ctrl.router.all_groups() {
                self.push_group_rows(ctrl, &gen, gid, 0, None, search.as_deref(), cur_filter.as_deref(), &mut rows);
            }
        } else {
            let children = ctrl.router.children_of(ctrl.router.root_id);
            self.push_children(ctrl, &gen, &children, 0, None, search.as_deref(), cur_filter.as_deref(), &mut rows, "");
        }
        self.rows = rows;
        self.insert_inline_row();
        self.selection.retain(|id| ctrl.router.contains_id(*id));
    }

    #[allow(clippy::too_many_arguments)]
    fn push_children(&mut self, ctrl: &MainController, gen: &xpr_data::GenData, children: &[NodeId], depth: usize, parent: Option<NodeId>, search: Option<&str>, filter: Option<&[String]>, rows: &mut Vec<Row>, path: &str) {
        for child in children {
            let should_render = ctrl.router.do_render(*child, gen, search, filter);
            if !should_render {
                continue;
            }
            match ctrl.router.obj_kind(*child) {
                Some(ObjKind::Folder) => {
                    let f = ctrl.router.folder(*child).unwrap();
                    let key = format!("{}/{}", path, f.name);
                    let expanded = match self.persistent_expand_state.get(&key) {
                        Some(e) => *e,
                        None => f.is_expanded(),
                    };
                    rows.push(Row { id: *child, kind: RowKind::Folder, depth, expandable: true, expanded, parent });
                    if expanded {
                        let grand = f.children.clone();
                        self.push_children(ctrl, gen, &grand, depth + 1, Some(*child), search, filter, rows, &key);
                    }
                }
                Some(ObjKind::Group) => {
                    self.push_group_rows(ctrl, gen, *child, depth, parent, search, filter, rows);
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_group_rows(&mut self, ctrl: &MainController, gen: &xpr_data::GenData, gid: NodeId, depth: usize, parent: Option<NodeId>, search: Option<&str>, filter: Option<&[String]>, rows: &mut Vec<Row>) {
        if !ctrl.router.do_render(gid, gen, search, filter) {
            return;
        }
        let Some(g) = ctrl.router.group(gid) else { return };
        let mut level_up: Vec<NodeId> = Vec::new();
        let mut other: Vec<NodeId> = Vec::new();
        if g.event_items.len() > 1 {
            for item_id in &g.event_items {
                let is_level_up = ctrl
                    .router
                    .item(*item_id)
                    .and_then(|i| i.event_definition.learn_move.as_ref())
                    .map(|lm| lm.source == consts::MOVE_SOURCE_LEVELUP)
                    .unwrap_or(false);
                if is_level_up {
                    level_up.push(*item_id);
                } else {
                    other.push(*item_id);
                }
            }
        }
        let expandable = !other.is_empty();
        let expanded = expandable && self.group_expanded.contains(&gid);
        rows.push(Row { id: gid, kind: RowKind::Group, depth, expandable, expanded, parent });
        for lu in level_up {
            rows.push(Row { id: lu, kind: RowKind::LevelUpSibling, depth, expandable: false, expanded: false, parent });
        }
        if expanded {
            for o in other {
                rows.push(Row { id: o, kind: RowKind::Item, depth: depth + 1, expandable: false, expanded: false, parent: Some(gid) });
            }
        }
    }

    /// `_collect_expand_state`: folder name paths -> expanded.
    fn collect_expand_state(&self, ctrl: &MainController) -> Vec<(String, bool)> {
        let mut out = Vec::new();
        // reconstruct paths from the current rows
        let mut stack: Vec<(usize, String)> = Vec::new();
        for row in &self.rows {
            if row.kind != RowKind::Folder {
                continue;
            }
            while let Some((d, _)) = stack.last() {
                if *d >= row.depth {
                    stack.pop();
                } else {
                    break;
                }
            }
            let name = ctrl.router.folder(row.id).map(|f| f.name.clone()).unwrap_or_default();
            let parent_path = stack.last().map(|(_, p)| p.clone()).unwrap_or_default();
            let key = format!("{}/{}", parent_path, name);
            out.push((key.clone(), row.expanded));
            stack.push((row.depth, key));
        }
        out
    }

    fn insert_inline_row(&mut self) {
        self.rows.retain(|r| r.kind != RowKind::InlineSpacer);
        let Some(inline) = &self.inline else { return };
        let spacer = Row { id: -1, kind: RowKind::InlineSpacer, depth: 0, expandable: false, expanded: false, parent: None };
        if let Some(fid) = self.inline_inside_folder_id {
            if let Some(pos) = self.rows.iter().position(|r| r.id == fid && r.kind == RowKind::Folder) {
                let depth = self.rows[pos].depth + 1;
                self.rows[pos].expanded = true;
                self.rows.insert(pos + 1, Row { depth, ..spacer });
            } else {
                self.remove_inline_creator_state();
            }
        } else if let Some(after) = inline.insert_after_id {
            if let Some(pos) = self.rows.iter().position(|r| r.id == after && r.kind != RowKind::Item) {
                let depth = self.rows[pos].depth;
                // Qt inserts the spacer as the next sibling row: after the target's
                // subtree (an expanded folder's children, a group's item children),
                // but before a group's level-up sibling rows.
                let mut insert_pos = pos + 1;
                while insert_pos < self.rows.len() && self.rows[insert_pos].depth > depth {
                    insert_pos += 1;
                }
                self.rows.insert(insert_pos, Row { depth, ..spacer });
            } else {
                self.remove_inline_creator_state();
            }
        }
    }

    fn remove_inline_creator_state(&mut self) {
        self.inline = None;
        self.inline_inside_folder_id = None;
        self.rows.retain(|r| r.kind != RowKind::InlineSpacer);
    }

    // ---- inline creator management ------------------------------------------------

    fn show_inline_creator(&mut self, ctrl: &MainController, insert_after: Option<NodeId>, dest_folder_name: Option<String>, inside_folder_id: Option<NodeId>, actions: &mut ListActions) {
        actions.suppress_battle_summary = Some(true);
        self.inline_counter += 1;
        let id = Id::new(("inline_creator", self.inline_counter));
        self.inline_inside_folder_id = inside_folder_id;
        self.inline = Some(InlineEventCreator::new(id, insert_after, dest_folder_name, None, ctrl));
        self.dirty = true;
    }

    /// `start_add_new_event`
    pub fn start_add_new_event(&mut self, ctrl: &mut MainController, actions: &mut ListActions) {
        if ctrl.is_empty() {
            let folder_name = self.unique_unnamed_folder_name(ctrl);
            ctrl.finalize_new_folder(&folder_name, None, None);
            let Some(fid) = ctrl.router.folder_lookup.get(&folder_name).copied() else { return };
            self.restore_editing_state(ctrl, actions);
            self.remove_inline_creator(actions);
            self.show_inline_creator(ctrl, None, Some(folder_name), Some(fid), actions);
            return;
        }
        if let Some(sel) = ctrl.get_single_selected_event_id(true) {
            self.restore_editing_state(ctrl, actions);
            self.remove_inline_creator(actions);
            self.show_inline_creator(ctrl, Some(sel), None, None, actions);
            return;
        }
        let last = ctrl.router.children_of(ctrl.router.root_id).last().copied();
        let Some(last_id) = last else { return };
        self.restore_editing_state(ctrl, actions);
        self.remove_inline_creator(actions);
        self.show_inline_creator(ctrl, Some(last_id), None, None, actions);
    }

    fn unique_unnamed_folder_name(&self, ctrl: &MainController) -> String {
        let existing: HashSet<String> = ctrl.get_all_folder_names().into_iter().collect();
        let base = "New Folder".to_string();
        if !existing.contains(&base) {
            return base;
        }
        let mut n = 2;
        loop {
            let c = format!("{} ({})", base, n);
            if !existing.contains(&c) {
                return c;
            }
            n += 1;
        }
    }

    /// `_start_inline_edit`
    fn start_inline_edit(&mut self, ctrl: &mut MainController, group_id: NodeId, actions: &mut ListActions) {
        self.restore_editing_state(ctrl, actions);
        self.remove_inline_creator(actions);
        actions.suppress_battle_summary = Some(true);
        let Some(def) = ctrl.router.group(group_id).map(|g| g.event_definition.clone()) else { return };
        self.editing_group_id = Some(group_id);
        self.editing_original_enabled = Some(def.is_enabled_flag());
        // disable the event so the route recalculates without it
        let mut disabled = def.clone();
        disabled.enabled = Some(false);
        if let Some(g) = ctrl.router.node_mut(group_id).and_then(|n| n.as_group_mut()) {
            g.set_enabled_status(false);
        }
        ctrl.update_existing_event(group_id, disabled);
        let id = Id::new(("inline_edit", group_id));
        let mut creator = InlineEventCreator::new(id, Some(group_id), None, Some((group_id, &def)), ctrl);
        creator.focus_primary_field();
        self.inline_inside_folder_id = None;
        self.inline = Some(creator);
        self.dirty = true;
    }

    /// `_restore_editing_state`
    fn restore_editing_state(&mut self, ctrl: &mut MainController, actions: &mut ListActions) {
        let Some(gid) = self.editing_group_id.take() else { return };
        let orig = self.editing_original_enabled.take().unwrap_or(true);
        actions.suppress_battle_summary = Some(false);
        let def = ctrl.router.node_mut(gid).and_then(|n| n.as_group_mut()).map(|g| {
            g.set_enabled_status(orig);
            g.event_definition.clone()
        });
        if let Some(d) = def {
            ctrl.update_existing_event(gid, d);
        }
    }

    fn remove_inline_creator(&mut self, actions: &mut ListActions) {
        if self.inline.is_some() {
            self.remove_inline_creator_state();
            self.dirty = true;
        }
        actions.suppress_battle_summary = Some(false);
    }

    fn on_inline_created(&mut self, ctrl: &mut MainController, result: Option<EventDefinition>, actions: &mut ListActions) {
        if let Some(gid) = self.editing_group_id {
            actions.suppress_battle_summary = Some(false);
            match result {
                Some(mut new_def) => {
                    let orig = self.editing_original_enabled.unwrap_or(true);
                    new_def.enabled = Some(orig);
                    if let Some(g) = ctrl.router.node_mut(gid).and_then(|n| n.as_group_mut()) {
                        g.set_enabled_status(orig);
                    }
                    ctrl.update_existing_event(gid, new_def);
                    self.editing_group_id = None;
                    self.editing_original_enabled = None;
                }
                None => self.restore_editing_state(ctrl, actions),
            }
        }
        self.remove_inline_creator(actions);
    }

    // ---- checkbox / expand helpers ---------------------------------------------------

    fn toggle_enabled(&mut self, ctrl: &mut MainController, id: NodeId) {
        match ctrl.router.obj_kind(id) {
            Some(ObjKind::Group) => {
                let def = ctrl.router.node_mut(id).and_then(|n| n.as_group_mut()).map(|g| {
                    let new_val = !g.enabled.unwrap_or(false);
                    g.set_enabled_status(new_val);
                    g.event_definition.clone()
                });
                if let Some(d) = def {
                    ctrl.update_existing_event(id, d);
                }
            }
            Some(ObjKind::Folder) => {
                let def = ctrl.router.node_mut(id).and_then(|n| n.as_folder_mut()).map(|f| {
                    let new_val = !f.enabled.unwrap_or(false);
                    f.set_enabled_status(new_val);
                    f.event_definition.clone()
                });
                if let Some(d) = def {
                    ctrl.update_existing_event(id, d);
                }
            }
            // EventItem has no set_enabled_status in Python (the toggle errors out); no-op here.
            _ => {}
        }
    }

    /// `trigger_checkbox`: toggle every selected row.
    pub fn trigger_checkbox(&mut self, ctrl: &mut MainController) {
        let ids = self.selection.clone();
        for id in ids {
            self.toggle_enabled(ctrl, id);
        }
    }

    fn toggle_expanded(&mut self, ctrl: &mut MainController, row: &Row) {
        match row.kind {
            RowKind::Folder => {
                let new_val = !row.expanded;
                if let Some(f) = ctrl.router.node_mut(row.id).and_then(|n| n.as_folder_mut()) {
                    f.expanded = Some(new_val);
                }
                let path = self.path_of_row(ctrl, row.id);
                self.persistent_expand_state.insert(path, new_val);
                self.dirty = true;
            }
            RowKind::Group => {
                if row.expanded {
                    self.group_expanded.remove(&row.id);
                } else {
                    self.group_expanded.insert(row.id);
                }
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn path_of_row(&self, ctrl: &MainController, id: NodeId) -> String {
        let mut names: Vec<String> = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            if c == ctrl.router.root_id {
                break;
            }
            if let Some(f) = ctrl.router.folder(c) {
                names.push(f.name.clone());
            }
            cur = ctrl.router.parent_of(c);
        }
        names.reverse();
        let mut path = String::new();
        for n in names {
            path.push('/');
            path.push_str(&n);
        }
        path
    }

    // ---- drawing --------------------------------------------------------------------------

    fn row_bg(&self, tags: &[&str], selected: bool, hovered: bool, theme: &Theme) -> Option<Color32> {
        let custom = tags.iter().find_map(|t| self.highlight_colors.get(*t).copied());
        if selected && hovered {
            return Some(theme::lighten(theme.accent, 0.20));
        }
        if selected {
            return Some(theme.accent);
        }
        if hovered {
            return Some(match custom {
                Some(c) => theme::lighten(c, 0.12),
                None => theme::lighten(theme.bg, 0.10),
            });
        }
        custom
    }

    fn quantity_of(ctrl: &MainController, id: NodeId) -> Option<i64> {
        let g = ctrl.router.group(id)?;
        let d = &g.event_definition;
        if let Some(ie) = &d.item_event_def {
            return Some(ie.item_amount);
        }
        if let Some(v) = &d.vitamin {
            return Some(v.amount);
        }
        if let Some(rc) = &d.rare_candy {
            return Some(rc.amount);
        }
        None
    }

    /// Draw the whole list (header + virtualized body). `key_input` is false
    /// while a text field elsewhere owns the keyboard.
    #[allow(clippy::too_many_arguments)]
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &Config, ctrl: &mut MainController, actions: &mut ListActions, key_input: bool) {
        let mut rebuilt = false;
        let t_rebuild_total = Instant::now();
        let t_rebuild = Instant::now();
        if self.dirty {
            self.refresh(ctrl);
            actions.refreshed = true;
            rebuilt = true;
        }
        let t_rebuild = t_rebuild.elapsed();
        let Some(gen) = ctrl.gen() else { return };
        let font = theme.body();
        let bold = theme.body_bold();
        let color_major = cfg.get_color_major_battles();

        // Column values and fit-to-contents widths (the widest value of any
        // row) only change with the rows or the font.
        let fonts = (font.clone(), bold.clone());
        if rebuilt || self.row_values.len() != self.rows.len() || self.fit_font.as_ref() != Some(&fonts) {
            let mut fit: Vec<f32> = COLUMN_DEFS.iter().map(|(h, _)| widgets::text_width(ui, h, &bold) + 10.0).collect();
            self.row_values = Vec::with_capacity(self.rows.len());
            // Only the longest values of a column can be its widest (they are
            // numbers and short labels in a font with tabular digits), so
            // just those are laid out instead of every string of every row.
            let mut longest: Vec<(usize, Vec<String>)> = vec![(0, Vec::new()); COLUMN_DEFS.len()];
            for row in &self.rows {
                let v = if row.kind == RowKind::InlineSpacer { RowValues::default() } else { ctrl.router.row_values(row.id, &gen) };
                for (ci, text) in column_texts(&v).into_iter().enumerate().skip(1) {
                    if COLUMN_DEFS[ci].1 >= 0.0 || text.is_empty() {
                        continue;
                    }
                    let n = text.chars().count();
                    let (best, candidates) = &mut longest[ci];
                    if n > *best {
                        *best = n;
                        candidates.clear();
                    }
                    if n == *best && !candidates.contains(&text) {
                        candidates.push(text);
                    }
                }
                self.row_values.push(v);
            }
            for (ci, (_, candidates)) in longest.iter().enumerate() {
                for text in candidates {
                    fit[ci] = fit[ci].max(widgets::text_width(ui, text, &font) + 10.0);
                }
            }
            self.fit_widths = fit;
            self.fit_font = Some(fonts);
            if frame_log() {
                log::info!("route list: rebuild {:.2} ms, values + widths {:.2} ms ({} rows)", t_rebuild.as_secs_f64() * 1000.0, (t_rebuild_total.elapsed() - t_rebuild).as_secs_f64() * 1000.0, self.rows.len());
            }
        }
        let fit = &self.fit_widths;
        let widths: Vec<f32> = (0..COLUMN_DEFS.len()).map(|i| if COLUMN_DEFS[i].1 < 0.0 { fit[i] } else { self.col_widths[i] }).collect();
        let total_w: f32 = widths.iter().sum::<f32>().max(ui.available_width());

        let outer = egui::Frame::new().fill(theme.bg_lighter).stroke(Stroke::new(1.0_f32, theme.border));
        outer.show(ui, |ui| {
            ui.set_min_size(Vec2::new(ui.available_width(), ui.available_height()));
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            // ---- header ----
            // The header lives outside the scroll area so it stays put while
            // the rows scroll under it; it is painted after the body (below)
            // shifted by the horizontal scroll offset so the columns line up.
            let (header_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER_HEIGHT), Sense::hover());
            let avail_h = ui.available_height();
            let mut area = egui::ScrollArea::both().id_salt("route_list_scroll").auto_shrink([false, false]).max_height(avail_h);
            if let Some(offset) = self.export_scroll {
                area = area.scroll_offset(offset);
            }
            let scroll = widgets::show_scroll(ui, area, |ui| {
                ui.set_min_width(total_w);
                // ---- body ----
                let n = self.rows.len();
                let body_top = ui.cursor().min.y;
                let left_x = ui.cursor().min.x;
                let mut y = body_top;
                let clip = ui.clip_rect();
                let pointer = ui.input(|i| i.pointer.interact_pos());
                let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
                let secondary_clicked = ui.input(|i| i.pointer.secondary_clicked());
                let primary_down = ui.input(|i| i.pointer.primary_down());
                let modifiers = ui.input(|i| i.modifiers);
                let mut hovered_now: Option<NodeId> = None;
                let mut clicked_row: Option<(usize, Rect)> = None;
                let mut right_clicked_row: Option<usize> = None;
                let mut toggle_check: Option<NodeId> = None;
                let mut toggle_expand: Option<usize> = None;
                let mut quantity_click: Option<(NodeId, Rect, i64)> = None;
                let mut plus_rect: Option<(NodeId, Rect)> = None;
                let mut inline_outcome: Option<crate::inline_creator::InlineOutcome> = None;
                let mut scroll_to_rect: Option<Rect> = None;
                let selected_set: HashSet<NodeId> = self.selection.iter().copied().collect();
                let inline_h = INLINE_ROW_HEIGHT;
                // the visible "+" button (drawn after the rows) must not also select its row
                let plus_btn_rect: Option<Rect> = self.plus_visible_for.and_then(|pid| {
                    let row_pos = self.rows.iter().position(|r| r.id == pid && r.kind != RowKind::Item)?;
                    let mut ry = body_top;
                    for r in &self.rows[..row_pos] {
                        ry += if r.kind == RowKind::InlineSpacer { inline_h } else { ROW_HEIGHT };
                    }
                    Some(Rect::from_min_size(Pos2::new(left_x + 2.0, ry + (ROW_HEIGHT - 16.0) / 2.0), Vec2::splat(16.0)))
                });
                for idx in 0..n {
                    let row = self.rows[idx].clone();
                    let h = if row.kind == RowKind::InlineSpacer { inline_h } else { ROW_HEIGHT };
                    let rect = Rect::from_min_size(Pos2::new(left_x, y), Vec2::new(total_w, h));
                    y += h;
                    // scroll targets
                    match self.scroll_target {
                        Some(ScrollTarget::Top) if idx == 0 => scroll_to_rect = Some(rect),
                        Some(ScrollTarget::Bottom) if idx == n - 1 => scroll_to_rect = Some(rect),
                        Some(ScrollTarget::Selected) if Some(row.id) == self.selection.last().copied() => scroll_to_rect = Some(rect),
                        _ => {}
                    }
                    if rect.max.y < clip.min.y - h || rect.min.y > clip.max.y + h {
                        // allocate without painting (virtualized)
                        ui.allocate_rect(rect, Sense::hover());
                        continue;
                    }
                    let _ = ui.allocate_rect(rect, Sense::hover());
                    if row.kind == RowKind::InlineSpacer {
                        let indent = left_x + 2.0 + row.depth as f32 * INDENT;
                        let strip = Rect::from_min_max(Pos2::new(indent, rect.min.y), Pos2::new(rect.max.x.max(indent + 200.0), rect.max.y));
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(strip).layout(egui::Layout::left_to_right(egui::Align::Center)));
                        if let Some(inline) = self.inline.as_mut() {
                            let out = inline.ui(&mut child, theme, ctrl);
                            if out.created || out.discarded {
                                inline_outcome = Some(out);
                            }
                        }
                        continue;
                    }
                    let values = self.row_values.get(idx).cloned().unwrap_or_default();
                    let texts = column_texts(&values);
                    let tags = ctrl.router.get_tags(row.id, &gen, color_major);
                    let is_selected = selected_set.contains(&row.id);
                    let is_hovered = pointer.map(|p| rect.contains(p) && clip.contains(p)).unwrap_or(false) && self.drag.as_ref().map(|d| !d.active).unwrap_or(true);
                    if is_hovered {
                        hovered_now = Some(row.id);
                    }
                    let bg = self.row_bg(&tags, is_selected, is_hovered, theme);
                    if let Some(c) = bg {
                        ui.painter().rect_filled(rect, CornerRadius::ZERO, c);
                    }
                    // foreground
                    let mut fg = theme.text;
                    let mut dimmed = false;
                    if row.kind == RowKind::Folder {
                        if let Some(c) = self.folder_fg {
                            fg = c;
                            dimmed = true;
                        }
                    }
                    if row.kind == RowKind::LevelUpSibling && !dimmed {
                        fg = Color32::from_rgb(0x88, 0x99, 0xaa);
                        dimmed = true;
                    }
                    let checkable = true;
                    if checkable && !values.is_enabled && (row.kind == RowKind::Folder || row.kind == RowKind::Group || matches!(row.kind, RowKind::Item | RowKind::LevelUpSibling)) {
                        // unchecked: grey text (folder's own flag for folders)
                        let own_unchecked = match row.kind {
                            RowKind::Folder => !ctrl.router.folder(row.id).map(|f| f.enabled.unwrap_or(false)).unwrap_or(true),
                            RowKind::Group => !ctrl.router.group(row.id).map(|g| g.enabled.unwrap_or(false)).unwrap_or(true),
                            _ => !values.is_enabled,
                        };
                        if own_unchecked {
                            fg = Color32::from_rgb(0xcb, 0xcb, 0xcb);
                            dimmed = true;
                        }
                    }
                    if is_selected && !dimmed {
                        fg = Color32::WHITE;
                    }
                    // name cell
                    let mut x = rect.min.x + 2.0 + row.depth as f32 * INDENT;
                    let name_cell_right = rect.min.x + widths[0];
                    // branch arrow
                    let branch_rect = Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(BRANCH_W, h));
                    if row.expandable {
                        let c = branch_rect.center();
                        let pts = if row.expanded {
                            vec![Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x + 4.0, c.y - 2.0), Pos2::new(c.x, c.y + 3.0)]
                        } else {
                            vec![Pos2::new(c.x - 2.0, c.y - 4.0), Pos2::new(c.x + 3.0, c.y), Pos2::new(c.x - 2.0, c.y + 4.0)]
                        };
                        ui.painter().add(egui::Shape::convex_polygon(pts, Color32::from_rgb(0xcc, 0xcc, 0xcc), Stroke::NONE));
                        if primary_clicked && pointer.map(|p| branch_rect.contains(p)).unwrap_or(false) {
                            toggle_expand = Some(idx);
                        }
                    }
                    x += BRANCH_W;
                    // checkbox
                    let own_checked = match row.kind {
                        RowKind::Folder => ctrl.router.folder(row.id).map(|f| f.enabled.unwrap_or(false)).unwrap_or(false),
                        RowKind::Group => ctrl.router.group(row.id).map(|g| g.enabled.unwrap_or(false)).unwrap_or(false),
                        _ => values.is_enabled,
                    };
                    let check_rect = Rect::from_center_size(Pos2::new(x + CHECK_W / 2.0, rect.center().y), Vec2::splat(CHECK_W));
                    paint_check(ui, check_rect, theme, own_checked, is_hovered && pointer.map(|p| check_rect.contains(p)).unwrap_or(false));
                    if primary_clicked && pointer.map(|p| check_rect.contains(p)).unwrap_or(false) {
                        toggle_check = Some(row.id);
                    }
                    x += CHECK_W + 4.0;
                    // text with optional quantity suffix
                    let raw_name = values.name.clone();
                    let text = if row.kind == RowKind::LevelUpSibling { format!("  └ {}", raw_name) } else { raw_name.clone() };
                    let quantity = if row.kind == RowKind::Group { RouteList::quantity_of(ctrl, row.id) } else { None };
                    let suffix = quantity.map(|q| format!("x{}", q)).filter(|s| text.ends_with(s.as_str()));
                    let max_text_w = (name_cell_right - x - 4.0).max(10.0);
                    match &suffix {
                        Some(s) => {
                            let prefix = &text[..text.len() - s.len()];
                            let prefix_w = widgets::text_width(ui, prefix, &font);
                            let shown_prefix = widgets::elide(ui, prefix, &font, max_text_w);
                            ui.painter().text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, shown_prefix, font.clone(), fg);
                            let suffix_color = if dimmed { Color32::from_rgb(0x5a, 0x7a, 0x99) } else { Color32::from_rgb(0x4d, 0xa6, 0xff) };
                            let sx = x + prefix_w;
                            let srect = ui.painter().text(Pos2::new(sx, rect.center().y), Align2::LEFT_CENTER, s, font.clone(), suffix_color);
                            ui.painter().line_segment([Pos2::new(srect.min.x, srect.max.y - 1.0), Pos2::new(srect.max.x, srect.max.y - 1.0)], Stroke::new(1.0_f32, suffix_color));
                            let hit = srect.expand2(Vec2::new(6.0, 2.0));
                            if primary_clicked && pointer.map(|p| hit.contains(p)).unwrap_or(false) {
                                quantity_click = Some((row.id, srect, quantity.unwrap_or(1)));
                            }
                        }
                        None => {
                            let shown = widgets::elide(ui, &text, &font, max_text_w);
                            ui.painter().text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, shown, font.clone(), fg);
                        }
                    }
                    // other columns
                    let mut cx = rect.min.x + widths[0];
                    for ci in 1..COLUMN_DEFS.len() {
                        let w = widths[ci];
                        if !texts[ci].is_empty() {
                            let shown = widgets::elide(ui, &texts[ci], &font, w - 6.0);
                            ui.painter().text(Pos2::new(cx + 2.0, rect.center().y), Align2::LEFT_CENTER, shown, font.clone(), fg);
                        }
                        cx += w;
                    }
                    // hover "+" button
                    if is_hovered && row.kind != RowKind::Item && row.kind != RowKind::LevelUpSibling && self.drag.as_ref().map(|d| !d.active).unwrap_or(true) {
                        plus_rect = Some((row.id, Rect::from_min_size(Pos2::new(rect.min.x + 2.0, rect.center().y - 8.0), Vec2::splat(16.0))));
                    }
                    // clicks (not on the checkbox/branch/quantity)
                    if let Some(p) = pointer {
                        if rect.contains(p) && clip.contains(p) {
                            let on_plus = plus_btn_rect.map(|r| r.contains(p)).unwrap_or(false);
                            if primary_clicked && !check_rect.contains(p) && !branch_rect.contains(p) && !on_plus {
                                clicked_row = Some((idx, rect));
                            }
                            if secondary_clicked {
                                right_clicked_row = Some(idx);
                            }
                        }
                    }
                    // drop indicator
                    if let Some(drag) = &self.drag {
                        if drag.active {
                            if let Some(p) = pointer {
                                if rect.contains(p) {
                                    let pos = if row.kind == RowKind::Folder && (p.y - rect.min.y) > h * 0.25 && (rect.max.y - p.y) > h * 0.25 {
                                        DropPos::On
                                    } else if p.y < rect.center().y {
                                        DropPos::Above
                                    } else {
                                        DropPos::Below
                                    };
                                    self.drop_target = Some((Some(row.id), pos));
                                    match pos {
                                        DropPos::On => {
                                            ui.painter().rect_stroke(rect, CornerRadius::ZERO, Stroke::new(2.0_f32, theme.accent), egui::StrokeKind::Inside);
                                        }
                                        DropPos::Above => {
                                            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(rect.min.x, rect.min.y), Vec2::new(rect.width(), 3.0)), CornerRadius::ZERO, theme.accent);
                                        }
                                        DropPos::Below => {
                                            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(rect.min.x, rect.max.y - 3.0), Vec2::new(rect.width(), 3.0)), CornerRadius::ZERO, theme.accent);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let body_end = y;
                // empty-area drop target
                if let Some(drag) = &self.drag {
                    if drag.active {
                        if let Some(p) = pointer {
                            if p.y > body_end && clip.contains(p) {
                                self.drop_target = Some((None, DropPos::Below));
                            }
                        }
                    }
                }
                // scroll requests
                if let Some(r) = scroll_to_rect {
                    // vertical only: keep the horizontal scroll where it is
                    let vr = Rect::from_min_max(Pos2::new(clip.min.x, r.min.y), Pos2::new(clip.min.x + 1.0, r.max.y));
                    ui.scroll_to_rect(vr, Some(egui::Align::Center));
                    self.scroll_target = None;
                } else if self.scroll_target.is_some() && n == 0 {
                    self.scroll_target = None;
                }
                // "+" button
                if let Some((row_id, _prect)) = plus_rect {
                    self.plus_visible_for = Some(row_id);
                    self.plus_hide_deadline = None;
                } else if self.plus_visible_for.is_some() && self.plus_hide_deadline.is_none() {
                    self.plus_hide_deadline = Some(Instant::now() + Duration::from_millis(150));
                }
                if let Some(d) = self.plus_hide_deadline {
                    if Instant::now() >= d {
                        self.plus_visible_for = None;
                        self.plus_hide_deadline = None;
                    } else {
                        ui.ctx().request_repaint_after(d - Instant::now());
                    }
                }
                let mut plus_clicked: Option<NodeId> = None;
                if let (Some(pid), Some(row_pos)) = (self.plus_visible_for, self.rows.iter().position(|r| r.id == self.plus_visible_for.unwrap_or(-2) && r.kind != RowKind::Item)) {
                    // recompute the rect of the row
                    let mut ry = body_top;
                    for r in &self.rows[..row_pos] {
                        ry += if r.kind == RowKind::InlineSpacer { inline_h } else { ROW_HEIGHT };
                    }
                    let prect = Rect::from_min_size(Pos2::new(left_x + 2.0, ry + (ROW_HEIGHT - 16.0) / 2.0), Vec2::splat(16.0));
                    let resp = ui.interact(prect, ui.id().with("plus_btn"), Sense::click());
                    let fill = if resp.hovered() { theme.accent_hover } else { theme.accent };
                    ui.painter().circle_filled(prect.center(), 8.0, fill);
                    ui.painter().text(prect.center(), Align2::CENTER_CENTER, "+", theme.font_bold(9.0), Color32::WHITE);
                    if resp.hovered() {
                        self.plus_hide_deadline = None;
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        plus_clicked = Some(pid);
                    }
                }
                // ---- apply interactions ----
                self.hovered_row = hovered_now;
                if let Some(id) = toggle_check {
                    self.close_quantity_editor(ctrl);
                    self.toggle_enabled(ctrl, id);
                } else if let Some(i) = toggle_expand {
                    self.close_quantity_editor(ctrl);
                    let row = self.rows[i].clone();
                    self.toggle_expanded(ctrl, &row);
                } else if let Some((gid, srect, qty)) = quantity_click {
                    self.close_quantity_editor(ctrl);
                    // select the row too (Qt runs the default press handler first)
                    self.select_row_click(ctrl, gid, modifiers);
                    let w = (srect.width() + 16.0).max(45.0);
                    let erect = Rect::from_min_size(Pos2::new(srect.center().x - w / 2.0, srect.min.y), Vec2::new(w, srect.height()));
                    self.quantity_editor = Some(QuantityEditor { group_id: gid, text: qty.to_string(), rect: erect, opened_frame: true });
                } else if let Some((idx, rect)) = clicked_row {
                    let row = self.rows[idx].clone();
                    self.close_quantity_editor(ctrl);
                    self.focused = true;
                    // double click?
                    let now = Instant::now();
                    let is_double = self.last_click.map(|(id, t)| id == row.id && now.duration_since(t) < Duration::from_millis(400)).unwrap_or(false);
                    self.last_click = Some((row.id, now));
                    if is_double && row.kind == RowKind::Group {
                        let et = ctrl.router.group(row.id).map(|g| g.event_definition.get_event_type()).unwrap_or("");
                        if key_for_event_type(et).is_some() && self.editing_group_id != Some(row.id) {
                            self.start_inline_edit(ctrl, row.id, actions);
                        }
                    } else {
                        self.select_row_click(ctrl, row.id, modifiers);
                        // start a potential drag
                        if row.kind == RowKind::Folder || row.kind == RowKind::Group {
                            let ids = self.get_all_selected_event_ids(ctrl, false);
                            if !ids.is_empty() {
                                self.drag = Some(DragState { ids, start: pointer.unwrap_or(rect.min), active: false });
                            }
                        }
                    }
                } else if let Some(idx) = right_clicked_row {
                    let row = self.rows[idx].clone();
                    self.close_quantity_editor(ctrl);
                    self.toggle_enabled(ctrl, row.id);
                }
                if let Some(pid) = plus_clicked {
                    self.restore_editing_state(ctrl, actions);
                    self.remove_inline_creator(actions);
                    self.show_inline_creator(ctrl, Some(pid), None, None, actions);
                    self.plus_visible_for = None;
                }
                if let Some(out) = inline_outcome {
                    if out.created {
                        self.on_inline_created(ctrl, out.result, actions);
                    } else if out.discarded {
                        self.restore_editing_state(ctrl, actions);
                        self.remove_inline_creator(actions);
                    }
                }
                // drag progress / release
                if let Some(drag) = self.drag.as_mut() {
                    if primary_down {
                        if let Some(p) = pointer {
                            if !drag.active && (p - drag.start).length() > 6.0 {
                                drag.active = true;
                            }
                        }
                    } else {
                        let d = self.drag.take().unwrap();
                        if d.active {
                            if let Some(target) = self.drop_target.take() {
                                self.finish_drop(ctrl, &d.ids, target);
                            }
                        }
                        self.drop_target = None;
                    }
                }
                if self.drag.as_ref().map(|d| d.active).unwrap_or(false) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                // quantity editor overlay
                if let Some(qe) = self.quantity_editor.as_mut() {
                    let id = ui.id().with("quantity_editor");
                    if qe.opened_frame {
                        ui.memory_mut(|m| m.request_focus(id));
                        let mut tes = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                        tes.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(qe.text.chars().count()))));
                        egui::TextEdit::store_state(ui.ctx(), id, tes);
                    }
                    let erect = qe.rect;
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(erect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    let r = widgets::Entry::new(theme, &mut qe.text).id(id).width(erect.width()).centered().show(&mut child);
                    let was_opened = qe.opened_frame;
                    qe.opened_frame = false;
                    if r.enter_pressed {
                        self.accept_quantity_edit(ctrl);
                    } else if r.escape_pressed {
                        self.quantity_editor = None;
                    } else if !was_opened && !r.has_focus {
                        self.accept_quantity_edit(ctrl);
                    }
                }
            });
            self.scroll_offset = scroll.state.offset;
            let painter = ui.painter().with_clip_rect(header_rect);
            let mut x = header_rect.min.x - scroll.state.offset.x;
            for (i, (title, _)) in COLUMN_DEFS.iter().enumerate() {
                let w = widths[i];
                let r = Rect::from_min_size(Pos2::new(x, header_rect.min.y), Vec2::new(w, HEADER_HEIGHT));
                painter.rect(r, CornerRadius::ZERO, theme.bg_darker, Stroke::new(1.0_f32, theme.border), egui::StrokeKind::Inside);
                painter.text(Pos2::new(r.min.x + 4.0, r.center().y), Align2::LEFT_CENTER, *title, bold.clone(), theme.text);
                // resizable name & levels-up columns
                if i <= 1 {
                    let handle = Rect::from_min_size(Pos2::new(r.max.x - 3.0, r.min.y), Vec2::new(6.0, HEADER_HEIGHT)).intersect(header_rect);
                    let resp = ui.interact(handle, ui.id().with(("col_resize", i)), Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if resp.dragged() {
                        self.col_widths[i] = (self.col_widths[i] + resp.drag_delta().x).max(40.0);
                    }
                }
                x += w;
            }
            // header bg past the last column
            if x < header_rect.max.x {
                painter.rect(Rect::from_min_max(Pos2::new(x, header_rect.min.y), header_rect.max), CornerRadius::ZERO, theme.bg_darker, Stroke::new(1.0_f32, theme.border), egui::StrokeKind::Inside);
            }
        });
        // keyboard handling (list focus + no text field)
        if key_input && self.focused {
            self.handle_keys(ui, ctrl, actions);
        }
        // empty-state button
        if ctrl.is_empty() && self.inline.is_none() {
            let full = ui.min_rect();
            let btn_size = Vec2::new(200.0, 44.0);
            let pos = Pos2::new(full.center().x - btn_size.x / 2.0, (full.min.y + full.height() / 3.0).max(full.min.y + 20.0));
            let brect = Rect::from_min_size(pos, btn_size);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(brect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let resp = widgets::StyledButton::new(theme, egui::RichText::new("Add New Event").font(theme.font_bold(10.5)))
                .fill(theme.accent)
                .hover_fill(theme.accent_hover)
                .text_color(Color32::WHITE)
                .stroke(Stroke::new(1.0_f32, theme.accent))
                .corner_radius(CornerRadius::same(4))
                .min_size(btn_size)
                .show(&mut child);
            if resp.clicked() {
                self.start_add_new_event(ctrl, actions);
            }
        }
    }

    /// Draw the list for an export (`event_list.grab()`): the on-screen
    /// scroll position, no pointer or keyboard, and without the hover "+"
    /// button. The transient state the draw touches is put back afterwards
    /// so the real frame that follows is unaffected.
    pub fn export_ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &Config, ctrl: &mut MainController) {
        let saved = (self.scroll_target.take(), self.hovered_row, self.plus_visible_for.take(), self.plus_hide_deadline.take(), self.scroll_offset);
        self.export_scroll = Some(self.scroll_offset);
        let mut actions = ListActions::default();
        self.ui(ui, theme, cfg, ctrl, &mut actions, false);
        self.export_scroll = None;
        (self.scroll_target, self.hovered_row, self.plus_visible_for, self.plus_hide_deadline, self.scroll_offset) = saved;
    }

    fn select_row_click(&mut self, ctrl: &mut MainController, id: NodeId, modifiers: egui::Modifiers) {
        log::debug!("route list row click: {} ({:?})", id, modifiers);
        if modifiers.command {
            if let Some(pos) = self.selection.iter().position(|x| *x == id) {
                self.selection.remove(pos);
            } else {
                self.selection.push(id);
            }
            self.anchor = Some(id);
        } else if modifiers.shift {
            let anchor = self.anchor.unwrap_or(id);
            let a = self.rows.iter().position(|r| r.id == anchor);
            let b = self.rows.iter().position(|r| r.id == id);
            if let (Some(a), Some(b)) = (a, b) {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                self.selection = self.rows[lo..=hi].iter().filter(|r| r.kind != RowKind::InlineSpacer).map(|r| r.id).collect();
            } else {
                self.selection = vec![id];
            }
        } else {
            self.selection = vec![id];
            self.anchor = Some(id);
        }
        let cur = self.get_all_selected_event_ids(ctrl, true);
        if ctrl.get_all_selected_ids(true) != cur {
            ctrl.select_new_events(cur);
        }
    }

    fn handle_keys(&mut self, ui: &mut Ui, ctrl: &mut MainController, actions: &mut ListActions) {
        if self.inline.is_some() || self.quantity_editor.is_some() {
            return;
        }
        let (space, q, w, e, r, t, up, down) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Q),
                i.consume_key(egui::Modifiers::NONE, egui::Key::W),
                i.consume_key(egui::Modifiers::NONE, egui::Key::E),
                i.consume_key(egui::Modifiers::NONE, egui::Key::R),
                i.consume_key(egui::Modifiers::NONE, egui::Key::T),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            )
        });
        if space {
            if let Some(sel) = ctrl.get_single_selected_event_id(true) {
                self.restore_editing_state(ctrl, actions);
                self.remove_inline_creator(actions);
                self.show_inline_creator(ctrl, Some(sel), None, None, actions);
            }
            return;
        }
        let category = if q {
            Some(0)
        } else if w {
            Some(1)
        } else if e {
            Some(2)
        } else if r {
            Some(3)
        } else if t {
            Some(4)
        } else {
            None
        };
        if let Some(cat) = category {
            if ctrl.can_insert_after_current_selection() {
                if let Some(last) = self.selection.last().copied() {
                    let anchor = self.row_anchor_pos(ui, last);
                    actions.quick_add = Some((anchor, Some(cat)));
                }
            }
            return;
        }
        if up || down {
            let cur = self.selection.last().copied().and_then(|id| self.rows.iter().position(|r| r.id == id));
            let next = match cur {
                Some(i) if up && i > 0 => Some(i - 1),
                Some(i) if down && i + 1 < self.rows.len() => Some(i + 1),
                None if !self.rows.is_empty() => Some(0),
                _ => None,
            };
            if let Some(ni) = next {
                let id = self.rows[ni].id;
                if self.rows[ni].kind != RowKind::InlineSpacer {
                    self.selection = vec![id];
                    self.anchor = Some(id);
                    self.scroll_target = Some(ScrollTarget::Selected);
                    let cur = self.get_all_selected_event_ids(ctrl, true);
                    ctrl.select_new_events(cur);
                }
            }
        }
    }

    /// The top-centre of a row (for the quick-add popover anchor).
    fn row_anchor_pos(&self, ui: &Ui, id: NodeId) -> Pos2 {
        let r = ui.min_rect();
        let mut y = r.min.y + HEADER_HEIGHT;
        for row in &self.rows {
            if row.id == id {
                break;
            }
            y += if row.kind == RowKind::InlineSpacer { INLINE_ROW_HEIGHT } else { ROW_HEIGHT };
        }
        Pos2::new(r.center().x, y)
    }

    fn close_quantity_editor(&mut self, _ctrl: &mut MainController) {
        self.quantity_editor = None;
    }

    /// `_accept_quantity_edit`
    fn accept_quantity_edit(&mut self, ctrl: &mut MainController) {
        let Some(qe) = self.quantity_editor.take() else { return };
        let Ok(mut new_qty) = qe.text.trim().parse::<i64>() else { return };
        if new_qty < 1 {
            new_qty = 1;
        }
        let Some(mut def) = ctrl.router.group(qe.group_id).map(|g| g.event_definition.clone()) else { return };
        if let Some(ie) = def.item_event_def.as_mut() {
            ie.item_amount = new_qty;
        } else if let Some(v) = def.vitamin.as_mut() {
            v.amount = new_qty;
        } else if let Some(rc) = def.rare_candy.as_mut() {
            rc.amount = new_qty;
        } else {
            return;
        }
        ctrl.update_existing_event(qe.group_id, def);
    }

    /// `_resolve_drop_target` + `move_events_to_position`
    fn finish_drop(&mut self, ctrl: &mut MainController, ids: &[NodeId], target: (Option<NodeId>, DropPos)) {
        let root = ctrl.router.root_id;
        let (folder_id, after, before) = match target {
            (None, _) => {
                let children = ctrl.router.children_of(root);
                (root, children.last().copied(), None)
            }
            (Some(mut tid), pos) => {
                if ids.contains(&tid) {
                    return;
                }
                if ctrl.router.obj_kind(tid) == Some(ObjKind::Item) {
                    match ctrl.router.item(tid).map(|i| i.parent) {
                        Some(p) => tid = p,
                        None => return,
                    }
                }
                if pos == DropPos::On {
                    if ctrl.router.obj_kind(tid) == Some(ObjKind::Folder) {
                        (tid, None, None)
                    } else {
                        let Some(parent) = ctrl.router.parent_of(tid) else { return };
                        (parent, Some(tid), None)
                    }
                } else {
                    let Some(parent) = ctrl.router.parent_of(tid) else { return };
                    if pos == DropPos::Above {
                        (parent, None, Some(tid))
                    } else {
                        (parent, Some(tid), None)
                    }
                }
            }
        };
        ctrl.move_events_to_position(ids, folder_id, after, before);
    }

    pub fn has_inline_creator(&self) -> bool {
        self.inline.is_some()
    }
}

/// `XPR_FRAME_LOG=1`: log the route-list rebuild cost.
fn frame_log() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("XPR_FRAME_LOG").is_some())
}

fn column_texts(v: &RowValues) -> [String; 9] {
    [
        v.name.clone(),
        v.pkmn_after_levelups.clone(),
        v.pkmn_level.map(|x| x.to_string()).unwrap_or_default(),
        v.percent_xp_to_next_level.clone(),
        v.total_xp.map(|x| x.to_string()).unwrap_or_default(),
        v.experience_per_second.clone(),
        v.xp_gain.map(|x| x.to_string()).unwrap_or_default(),
        v.xp_to_next_level.map(|x| x.to_string()).unwrap_or_default(),
        v.level_gain.clone(),
    ]
}

fn paint_check(ui: &Ui, rect: Rect, theme: &Theme, checked: bool, hovered: bool) {
    let painter = ui.painter();
    let border = if hovered { theme.accent } else { theme.border_focus };
    let fill = if checked {
        if hovered {
            theme.accent_hover
        } else {
            theme.accent
        }
    } else {
        theme.bg_input
    };
    painter.rect(rect, CornerRadius::same(2), fill, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    if checked {
        let c = rect.center();
        painter.add(egui::Shape::line(
            vec![Pos2::new(c.x - 4.0, c.y), Pos2::new(c.x - 1.5, c.y + 3.0), Pos2::new(c.x + 4.5, c.y - 3.5)],
            Stroke::new(2.0_f32, Color32::WHITE),
        ));
    }
}
