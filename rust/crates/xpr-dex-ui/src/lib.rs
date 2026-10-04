//! The Dex page: Solodex's reference views ported to egui and hosted by the
//! router (`xpr-app`), so a route can be planned, recorded and refined
//! without a second program open.
//!
//! [`DexView`] is the whole page (tab bar, game toggle, the tabs, the search
//! overlays). The host draws it into a `Ui` each frame with a [`DexHost`]
//! for what only the host can do (the Damage tab, which is the router's own
//! battle summary), and applies the [`DexAction`]s it returns.
//!
//! Data: the Pokédex tabs read `xpr-dex` (Solodex's data, gens 1-9); the
//! Trainers and Damage tabs read the router's own trainer tables through
//! the `Registry`, so names and numbers match the route editor.

pub mod images;
pub mod palette;
pub mod shell;
pub mod state;
#[doc(hidden)]
pub mod testkit;
pub mod views;
pub mod widgets;

use std::sync::Arc;

pub use images::{DexImages, SpriteSize};
pub use shell::DexView;
pub use state::{DamageRequest, DexAction, DexSettings, DexState, DexTab, RouteContext, Spotlight};

use xpr_data::Registry;
use xpr_ui_kit::theme::Theme;

/// What the tabs get each frame.
pub struct DexCx<'a> {
    pub theme: &'a Theme,
    /// router data (trainers, versions, custom gens)
    pub registry: &'a Arc<Registry>,
    /// what the route editor has open
    pub route: &'a RouteContext,
    pub state: &'a mut DexState,
    pub images: &'a mut DexImages,
    pub actions: &'a mut Vec<DexAction>,
}

/// What the Dex needs from the app hosting it.
pub trait DexHost {
    /// Draw the Damage tab into `ui`. `cx.state.damage_request` carries what
    /// other tabs asked it to load (compare its `nonce` with the last one seen).
    fn damage_ui(&mut self, ui: &mut egui::Ui, cx: &mut DexCx);

    /// Whether "Add to Route" can insert a fight of `version` now (a route of
    /// that version is open).
    fn can_add_to_route(&self, version: &str) -> bool;
}

/// A host with no route and no Damage tab (tests, previews).
pub struct NoHost;

impl DexHost for NoHost {
    fn damage_ui(&mut self, ui: &mut egui::Ui, cx: &mut DexCx) {
        widgets::placeholder(ui, cx.theme, "The Damage tab needs the router.");
    }

    fn can_add_to_route(&self, _version: &str) -> bool {
        false
    }
}
