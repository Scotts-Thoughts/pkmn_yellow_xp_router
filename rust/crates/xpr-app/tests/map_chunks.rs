//! The chunk texture cache (`map/chunks.rs`): a fine chunk that the byte
//! budget evicted must render again when it is next wanted. The worker used
//! to remember every chunk it had rendered and silently skip repeats, so an
//! evicted chunk stayed pending forever and the view kept drawing the
//! blurry coarser-level fallback in its place (seen on Emerald's Route 105,
//! the biggest world, after panning around at full zoom).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use xpr_app::map::chunks::{ChunkCache, ChunkKey};
use xpr_map::{Compositor, MapPack, PackSource, Scope};

fn compositor(game: &str) -> Arc<Compositor> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../map_data");
    let pack = MapPack::load(game, &PackSource::new(Some(root))).expect("load pack");
    Arc::new(Compositor::new(Arc::new(pack)))
}

fn key(cx: u32, cy: u32) -> ChunkKey {
    ChunkKey { scope: Scope::World, level: 0, cx, cy, night: false }
}

/// Keep asking for `wanted` (as the view does every frame) and uploading
/// until all of them are cached, or give up after a few seconds.
fn settle(cache: &mut ChunkCache, ctx: &egui::Context, wanted: &[ChunkKey]) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        cache.begin_frame();
        for k in wanted {
            let _ = cache.get(k);
        }
        cache.request(wanted.to_vec());
        cache.poll(ctx);
        if wanted.iter().all(|k| cache.has(k)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

#[test]
fn evicted_chunk_renders_again() {
    let ctx = egui::Context::default();
    // the 32 MB floor: 32 level-0 chunks fit
    let mut cache = ChunkCache::new(32);
    cache.uploads_per_frame = 64;
    cache.set_compositor(compositor("emerald"));

    let first: Vec<ChunkKey> = (0..8).flat_map(|cy| (0..4).map(move |cx| key(cx, cy))).collect();
    assert!(settle(&mut cache, &ctx, &first), "first view never finished");

    // pan somewhere else until the first view's chunks are evicted
    let second: Vec<ChunkKey> = (0..8).flat_map(|cy| (8..12).map(move |cx| key(cx, cy))).collect();
    assert!(settle(&mut cache, &ctx, &second), "second view never finished");
    assert!(first.iter().any(|k| !cache.has(k)), "the budget should have evicted part of the first view");

    // pan back: every chunk of the first view has to come back
    assert!(settle(&mut cache, &ctx, &first), "evicted chunks were never rendered again");
}
