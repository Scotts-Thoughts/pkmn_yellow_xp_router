//! World-map data, compositing and links for the router
//! (`docs/rust_port/design/world_map/SPEC.md`). No egui here: the crate
//! loads the map pack exported by pokemap's pipeline, composites tile maps
//! into RGBA chunks, indexes objects for hit-testing and resolves route
//! identities to map anchors. The viewer lives in `xpr-app/src/map/`.

pub mod compose;
pub mod embedded;
pub mod export;
pub mod geom;
pub mod links;
pub mod lod;
pub mod model;
pub mod pack;
pub mod spatial;
pub mod sprites;

pub use compose::{Compositor, RenderOpts, CHUNK_PX};
pub use export::{ExportError, ExportRequest};
pub use geom::{IRect, Scope};
pub use links::{LinkQuery, Links};
pub use lod::Pixmap;
pub use model::*;
pub use pack::{game_for_version, MapError, MapPack, PackSource};
pub use spatial::ObjectGrid;
pub use sprites::{Dir, FrameKey, SpriteCache};
