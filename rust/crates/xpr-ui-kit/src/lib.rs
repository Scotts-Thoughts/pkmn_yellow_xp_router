//! egui widget kit mirroring the Qt component set of the Python app: theme
//! tokens derived with the `theme.py` math, the custom controls of
//! `custom_components.py`, Qt key-sequence shortcuts, window geometry
//! persistence, and the notification toast.

pub mod geometry;
pub mod shortcuts;
pub mod theme;
pub mod toast;
pub mod widgets;

pub use geometry::Geometry;
pub use shortcuts::{format_key_sequence, parse_key_sequence, ShortcutMap};
pub use theme::Theme;
pub use toast::{AutoClearingLabel, ToastHost};
