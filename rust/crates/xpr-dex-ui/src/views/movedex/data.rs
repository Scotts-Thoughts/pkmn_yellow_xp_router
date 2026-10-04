//! The Movedex's data logic without any egui: the move table of a game, its
//! filters and sort orders, who learns a move and how, and the generation
//! history of a move (Solodex `MovedexView.tsx` / `MoveDetailView.tsx`).

use std::cmp::Ordering;

use xpr_dex::{text::locale_cmp, MoveData};

use crate::widgets::SortDir;

// ---------------------------------------------------------------------------
// the move table
// ---------------------------------------------------------------------------

/// One row of the table: a move of the game with the extras the columns need.
#[derive(Clone, Debug)]
pub struct MoveRow {
    /// the game's spelling ("Faint Attack" in gen 4)
    pub name: String,
    pub lower: String,
    pub data: MoveData,
    /// in this generation's table but not the previous one's
    pub new_in_gen: bool,
    /// "TM24" / "HM03" in this game
    pub tm: Option<String>,
}

/// Every move of `game`'s generation (Solodex `getMovesForGen(gen, game)`),
/// by name.
pub fn moves_for_game(game: &str) -> Vec<MoveRow> {
    let gen = xpr_dex::game_gen(game);
    if gen == 0 {
        return Vec::new();
    }
    xpr_dex::get_moves_for_gen(gen, Some(game))
        .into_iter()
        .map(|(name, data)| MoveRow { lower: name.to_lowercase(), new_in_gen: xpr_dex::moves::is_move_new_in_gen(&name, gen), tm: xpr_dex::tmhm_code(&name, game), name, data })
        .collect()
}

pub const ALL: &str = "All";
pub const CATEGORIES: [&str; 3] = ["Physical", "Special", "Status"];

/// The filter inputs. The number fields stay strings like the HTML inputs
/// (an empty or unreadable one means no bound).
#[derive(Clone, Debug, PartialEq)]
pub struct Filters {
    pub search: String,
    /// "All" or a type
    pub type_filter: String,
    /// "All", "Physical", "Special" or "Status"
    pub category: String,
    pub min_power: String,
    pub max_power: String,
    pub min_accuracy: String,
    pub max_accuracy: String,
    pub min_pp: String,
    pub max_pp: String,
    pub only_new: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            search: String::new(),
            type_filter: ALL.to_string(),
            category: ALL.to_string(),
            min_power: String::new(),
            max_power: String::new(),
            min_accuracy: String::new(),
            max_accuracy: String::new(),
            min_pp: String::new(),
            max_pp: String::new(),
            only_new: false,
        }
    }
}

fn bound(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        s.parse::<f64>().ok().filter(|v| v.is_finite())
    }
}

/// `min <= v <= max` with an absent value failing any bound that is set.
fn in_range(v: Option<i32>, min: Option<f64>, max: Option<f64>) -> bool {
    if let Some(lo) = min {
        match v {
            Some(v) if (v as f64) >= lo => {}
            _ => return false,
        }
    }
    if let Some(hi) = max {
        match v {
            Some(v) if (v as f64) <= hi => {}
            _ => return false,
        }
    }
    true
}

impl Filters {
    /// Whether any filter is set.
    pub fn any(&self) -> bool {
        *self != Filters::default()
    }

    /// Solodex's `filteredMoves` predicate.
    pub fn matches(&self, row: &MoveRow) -> bool {
        let query = self.search.trim().to_lowercase();
        if !query.is_empty() && !row.lower.contains(&query) {
            return false;
        }
        if self.type_filter != ALL && row.data.move_type != self.type_filter {
            return false;
        }
        if self.category != ALL && row.data.category != self.category {
            return false;
        }
        if !in_range(row.data.power, bound(&self.min_power), bound(&self.max_power)) {
            return false;
        }
        if !in_range(row.data.accuracy, bound(&self.min_accuracy), bound(&self.max_accuracy)) {
            return false;
        }
        if !in_range(row.data.pp, bound(&self.min_pp), bound(&self.max_pp)) {
            return false;
        }
        if self.only_new && !row.new_in_gen {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// sorting
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Move,
    Type,
    Cat,
    Power,
    Accuracy,
    Pp,
    Tm,
}

impl Col {
    pub const ALL: [Col; 7] = [Col::Move, Col::Type, Col::Cat, Col::Power, Col::Accuracy, Col::Pp, Col::Tm];

    pub fn label(self) -> &'static str {
        match self {
            Col::Move => "Move",
            Col::Type => "Type",
            Col::Cat => "Cat",
            Col::Power => "Pwr",
            Col::Accuracy => "Acc",
            Col::Pp => "PP",
            Col::Tm => "TM",
        }
    }

    pub fn right_aligned(self) -> bool {
        matches!(self, Col::Power | Col::Accuracy | Col::Pp)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub col: Col,
    pub dir: SortDir,
}

impl Default for Sort {
    /// Solodex's only order: by name.
    fn default() -> Self {
        Sort { col: Col::Move, dir: SortDir::Asc }
    }
}

impl Sort {
    /// A click on a header: sort by it ascending, or flip when it is the
    /// active column.
    pub fn clicked(self, col: Col) -> Sort {
        if self.col == col {
            Sort { col, dir: self.dir.flip() }
        } else {
            Sort { col, dir: SortDir::Asc }
        }
    }

    pub fn active(self, col: Col) -> Option<SortDir> {
        (self.col == col).then_some(self.dir)
    }
}

/// Absent values sort last whichever way the column runs.
fn cmp_opt<T>(a: Option<T>, b: Option<T>, dir: SortDir, cmp: impl Fn(&T, &T) -> Ordering) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => {
            let o = cmp(&a, &b);
            if dir == SortDir::Desc {
                o.reverse()
            } else {
                o
            }
        }
    }
}

/// Order `idx` (indices into the move table) by `sort`; ties by name.
pub fn sort_rows(table: &[MoveRow], idx: &mut [usize], sort: Sort) {
    let by_name = |a: &MoveRow, b: &MoveRow| locale_cmp(&a.name, &b.name);
    idx.sort_by(|&i, &j| {
        let (a, b) = (&table[i], &table[j]);
        let primary = match sort.col {
            Col::Move => {
                let o = by_name(a, b);
                return if sort.dir == SortDir::Desc { o.reverse() } else { o };
            }
            Col::Type => cmp_opt(Some(crate::palette::type_label(&a.data.move_type)), Some(crate::palette::type_label(&b.data.move_type)), sort.dir, |x, y| locale_cmp(x, y)),
            Col::Cat => cmp_opt(Some(a.data.category.as_str()), Some(b.data.category.as_str()), sort.dir, |x, y| locale_cmp(x, y)),
            Col::Power => cmp_opt(a.data.power, b.data.power, sort.dir, |x, y| x.cmp(y)),
            Col::Accuracy => cmp_opt(a.data.accuracy, b.data.accuracy, sort.dir, |x, y| x.cmp(y)),
            Col::Pp => cmp_opt(a.data.pp, b.data.pp, sort.dir, |x, y| x.cmp(y)),
            Col::Tm => cmp_opt(a.tm.as_deref(), b.tm.as_deref(), sort.dir, |x, y| xpr_dex::compare_tmhm(x, y)),
        };
        primary.then_with(|| by_name(a, b))
    });
}

// ---------------------------------------------------------------------------
// who learns a move
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodKind {
    /// Lv N / Evo
    Level,
    /// TMxx / HMxx
    Tm,
    Egg,
    /// Tutor, Transfer, Reminder (and TRxx)
    Other,
}

/// Solodex colours a method chip by its text.
pub fn method_kind(label: &str) -> MethodKind {
    if label.starts_with("Lv") || label == "Evo" {
        MethodKind::Level
    } else if label.starts_with("TM") || label.starts_with("HM") {
        MethodKind::Tm
    } else if label == "Egg" {
        MethodKind::Egg
    } else {
        MethodKind::Other
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Learner {
    /// the Dex species key
    pub species: String,
    pub dex: i32,
    /// "Lv 12", "Evo", "Reminder", "TM45", "Tutor", "Egg", "Transfer"
    pub methods: Vec<String>,
}

/// Every species of `game` that learns `move_name` (the game's spelling) and
/// how, by national dex number.
pub fn learners_of(move_name: &str, game: &str) -> Vec<Learner> {
    let tm_code = xpr_dex::tmhm_code(move_name, game);
    let mut out = Vec::new();
    for poke in xpr_dex::get_all_pokemon_for_game(game) {
        let mut methods = Vec::new();
        for (level, name) in &poke.level_up_learnset {
            if name == move_name {
                methods.push(match *level {
                    0 => "Evo".to_string(),
                    l if l < 0 => "Reminder".to_string(),
                    l => format!("Lv {}", l),
                });
            }
        }
        if poke.tm_hm_learnset.iter().any(|m| m == move_name) {
            methods.push(tm_code.clone().unwrap_or_else(|| "TM/HM".to_string()));
        }
        if poke.tutor_learnset.iter().any(|m| m == move_name) {
            methods.push("Tutor".to_string());
        }
        if poke.egg_moves.iter().any(|m| m == move_name) {
            methods.push("Egg".to_string());
        }
        if poke.transfer_learnset.iter().any(|m| m == move_name) {
            methods.push("Transfer".to_string());
        }
        if !methods.is_empty() {
            out.push(Learner { species: poke.species.clone(), dex: poke.national_dex_number, methods });
        }
    }
    out.sort_by_key(|l| l.dex);
    out
}

// ---------------------------------------------------------------------------
// generation history (the detail view)
// ---------------------------------------------------------------------------

/// A tracked field's value, for the change callouts.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Text(String),
    Num(Option<i32>),
    Bool(bool),
}

impl Val {
    /// Solodex `fmtVal` (text as it is).
    pub fn shown(&self) -> String {
        match self {
            Val::Bool(true) => "Yes".to_string(),
            Val::Bool(false) => "No".to_string(),
            Val::Num(Some(n)) => n.to_string(),
            Val::Num(None) => "\u{2014}".to_string(),
            Val::Text(s) => s.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Type,
    Category,
    Power,
    Accuracy,
    Pp,
    Priority,
    EffectChance,
    Target,
    Contact,
    Protect,
    MagicCoat,
    Snatch,
    MirrorMove,
    KingsRock,
}

/// Solodex's `CHANGE_KEYS` (effect text is left out: its schema differs by gen).
pub const CHANGE_FIELDS: [Field; 14] = [
    Field::Type,
    Field::Category,
    Field::Power,
    Field::Accuracy,
    Field::Pp,
    Field::Priority,
    Field::EffectChance,
    Field::Target,
    Field::Contact,
    Field::Protect,
    Field::MagicCoat,
    Field::Snatch,
    Field::MirrorMove,
    Field::KingsRock,
];

impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Field::Type => "Type",
            Field::Category => "Category",
            Field::Power => "Power",
            Field::Accuracy => "Accuracy",
            Field::Pp => "PP",
            Field::Priority => "Priority",
            Field::EffectChance => "Effect %",
            Field::Target => "Target",
            Field::Contact => "Contact",
            Field::Protect => "Protect",
            Field::MagicCoat => "Magic Coat",
            Field::Snatch => "Snatch",
            Field::MirrorMove => "Mirror Move",
            Field::KingsRock => "King's Rock",
        }
    }

    pub fn get(self, m: &MoveData) -> Val {
        match self {
            Field::Type => Val::Text(m.move_type.clone()),
            Field::Category => Val::Text(m.category.clone()),
            Field::Power => Val::Num(m.power),
            Field::Accuracy => Val::Num(m.accuracy),
            Field::Pp => Val::Num(m.pp),
            Field::Priority => Val::Num(Some(m.priority)),
            Field::EffectChance => Val::Num(m.effect_chance),
            Field::Target => Val::Text(m.target.clone()),
            Field::Contact => Val::Bool(m.makes_contact),
            Field::Protect => Val::Bool(m.affected_by_protect),
            Field::MagicCoat => Val::Bool(m.affected_by_magic_coat),
            Field::Snatch => Val::Bool(m.affected_by_snatch),
            Field::MirrorMove => Val::Bool(m.affected_by_mirror_move),
            Field::KingsRock => Val::Bool(m.affected_by_kings_rock),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub field: Field,
    pub old: Val,
    pub new: Val,
}

pub fn changes_between(prev: &MoveData, curr: &MoveData) -> Vec<Change> {
    CHANGE_FIELDS
        .iter()
        .filter_map(|f| {
            let (a, b) = (f.get(prev), f.get(curr));
            (a != b).then_some(Change { field: *f, old: a, new: b })
        })
        .collect()
}

/// Consecutive generations without a tracked change, as one card.
#[derive(Clone, Debug)]
pub struct GenGroup {
    pub start: u8,
    pub end: u8,
    pub data: MoveData,
    /// what changed from the previous group (empty for the first)
    pub changes: Vec<Change>,
}

/// Solodex `collapseGens` over `move_across_gens(name)`.
pub fn collapse_gens(history: &[(u8, &MoveData)]) -> Vec<GenGroup> {
    let mut out: Vec<GenGroup> = Vec::new();
    let Some((first_gen, first)) = history.first() else { return out };
    let mut current = GenGroup { start: *first_gen, end: *first_gen, data: (*first).clone(), changes: Vec::new() };
    for (gen, data) in history.iter().skip(1) {
        let changes = changes_between(&current.data, data);
        if changes.is_empty() {
            current.end = *gen;
        } else {
            out.push(current);
            current = GenGroup { start: *gen, end: *gen, data: (*data).clone(), changes };
        }
    }
    out.push(current);
    out
}

/// Solodex `formatEffect`: `may_paralyze` -> `May Paralyze`.
pub fn format_effect(effect: &str) -> String {
    let spaced = effect.replace('_', " ");
    let mut out = String::with_capacity(spaced.len());
    let mut prev_word = false;
    for c in spaced.chars() {
        let word = c.is_alphanumeric() || c == '_';
        if word && !prev_word {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        prev_word = word;
    }
    out
}

/// "https://bulbapedia.bulbagarden.net/wiki/Thunderbolt_(move)" for any
/// spelling of a move (older spellings are mapped to the modern article).
pub fn bulbapedia_url(move_name: &str) -> String {
    let modern = xpr_dex::move_name_for_gen(move_name, 9);
    let mut page = String::new();
    for c in modern.chars() {
        match c {
            ' ' => page.push('_'),
            c if c.is_ascii_alphanumeric() || "-_.~!*'(),:@".contains(c) => page.push(c),
            c => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    page.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    format!("https://bulbapedia.bulbagarden.net/wiki/{}_(move)", page)
}

/// Move search (the shell's Ctrl+Shift+Space overlay): every move of every
/// generation whose name contains `query`.
pub fn search_names(query: &str) -> Vec<&'static str> {
    use std::sync::OnceLock;
    static NAMES: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    let names = NAMES.get_or_init(|| xpr_dex::all_move_names().iter().map(|n| (n.as_str(), n.to_lowercase())).collect());
    let q = query.trim().to_lowercase();
    names.iter().filter(|(_, lower)| q.is_empty() || lower.contains(&q)).map(|(n, _)| *n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_text() {
        assert_eq!(format_effect("may_paralyze"), "May Paralyze");
        assert_eq!(format_effect(""), "");
    }

    #[test]
    fn urls() {
        assert_eq!(bulbapedia_url("Thunderbolt"), "https://bulbapedia.bulbagarden.net/wiki/Thunderbolt_(move)");
        assert_eq!(bulbapedia_url("Faint Attack"), "https://bulbapedia.bulbagarden.net/wiki/Feint_Attack_(move)");
        assert_eq!(bulbapedia_url("Double-Edge"), "https://bulbapedia.bulbagarden.net/wiki/Double-Edge_(move)");
    }
}
