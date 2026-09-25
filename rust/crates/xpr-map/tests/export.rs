//! `xpr_map::export` (WP-A, `docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md`
//! §3.4 "A"): output-size maths, `check` refusals, banded rendering versus
//! the single-shot reference path, transparency, cancellation and the
//! Emerald world size bound. Follows `tests/pack.rs`'s conventions (packs
//! load from `map_data/<game>` via `PackSource::new`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use xpr_map::compose::BACKGROUND;
use xpr_map::export::{check, output_pixels, output_size, render, upscale_into, ExportError, ExportRequest, MAX_PIXELS, MAX_SCALE};
use xpr_map::geom::map_at_world_px;
use xpr_map::{Compositor, IRect, MapPack, PackSource, RenderOpts, Scope};

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn source() -> PackSource {
    PackSource::new(Some(repo_root().join("map_data")))
}

fn yellow() -> (Arc<MapPack>, Compositor) {
    let pack = Arc::new(MapPack::load("yellow", &source()).unwrap());
    let comp = Compositor::new(pack.clone());
    (pack, comp)
}

// ---- output-size maths and `check` refusals --------------------------------------

#[test]
fn output_size_and_pixels_multiply_by_scale() {
    let rect = IRect::from_size(10, 20, 30, 40);
    assert_eq!(output_size(rect, 1), (30, 40));
    assert_eq!(output_size(rect, 3), (90, 120));
    assert_eq!(output_pixels(rect, 1), 1200);
    assert_eq!(output_pixels(rect, 3), 10800);
    // an empty or inverted rect never yields negative/overflowed sizes
    let empty = IRect::new(5, 5, 5, 5);
    assert_eq!(output_size(empty, 8), (0, 0));
    assert_eq!(output_pixels(empty, 8), 0);
}

#[test]
fn check_refuses_bad_scale() {
    let rect = IRect::from_size(0, 0, 100, 100);
    assert_eq!(check(rect, 0), Err(ExportError::BadScale(0)));
    assert_eq!(check(rect, MAX_SCALE + 1), Err(ExportError::BadScale(MAX_SCALE + 1)));
    assert!(check(rect, MAX_SCALE).is_ok());
    assert!(check(rect, 1).is_ok());
}

#[test]
fn check_refuses_empty_regions() {
    assert_eq!(check(IRect::new(0, 0, 0, 50), 1), Err(ExportError::Empty));
    assert_eq!(check(IRect::new(0, 0, 50, 0), 1), Err(ExportError::Empty));
    assert_eq!(check(IRect::new(10, 10, 5, 5), 1), Err(ExportError::Empty)); // inverted
}

#[test]
fn check_refuses_outputs_past_the_pixel_bound() {
    // 20000x20000 at 1x is 400 Mpx, comfortably past MAX_PIXELS (268,435,456)
    let rect = IRect::from_size(0, 0, 20000, 20000);
    let pixels = output_pixels(rect, 1);
    assert!(pixels > MAX_PIXELS);
    assert_eq!(check(rect, 1), Err(ExportError::TooLarge { pixels, max: MAX_PIXELS }));
    // just under the bound at 1x is fine
    let ok_rect = IRect::from_size(0, 0, 1000, 1000);
    assert!(output_pixels(ok_rect, 1) < MAX_PIXELS);
    assert!(check(ok_rect, 1).is_ok());
}

#[test]
fn check_runs_before_any_allocation() {
    // Yellow's whole world at 1x is 31,334,400 px; at 3x it's 282,009,600,
    // just over MAX_PIXELS (268,435,456). If `render` allocated the output
    // before validating instead of after, it should still refuse instantly
    // rather than spend time on a ~1 GiB allocation first.
    let (pack, comp) = yellow();
    let rect = xpr_map::geom::scope_rect(&pack, Scope::World);
    let pixels = output_pixels(rect, 3);
    assert!(pixels > MAX_PIXELS, "test setup: expected to exceed MAX_PIXELS, got {}", pixels);
    let req = ExportRequest { scope: Scope::World, rect, scale: 3, opts: RenderOpts::default(), transparent: false };
    let t = Instant::now();
    let err = render(&comp, &req, &mut |_| true).unwrap_err();
    assert!(matches!(err, ExportError::TooLarge { .. }));
    assert!(t.elapsed() < Duration::from_millis(500), "check should refuse instantly, took {:?}", t.elapsed());
}

// ---- 1x export equals `render_scope` ----------------------------------------------

#[test]
fn one_x_map_export_equals_render_scope_byte_for_byte() {
    let (pack, comp) = yellow();
    let map = pack.map_by_const("PEWTER_GYM").expect("PEWTER_GYM in the Yellow pack");
    let scope = Scope::Map(map.id);
    let rect = xpr_map::geom::scope_rect(&pack, scope);
    // Pewter Gym is well under one band (224 rows), so this also exercises
    // the single-band path.
    assert!(rect.height() < 256);

    let req = ExportRequest { scope, rect, scale: 1, opts: RenderOpts::default(), transparent: false };
    let mut progress_calls = 0;
    let out = render(&comp, &req, &mut |f| {
        progress_calls += 1;
        assert!((0.0..=1.0).contains(&f));
        true
    })
    .expect("export should succeed");

    let golden = comp.render_scope(scope, RenderOpts::default());
    assert_eq!((out.w, out.h), (golden.w, golden.h));
    assert_eq!(out.data, golden.data, "1x export must match render_scope byte for byte");
    assert!(progress_calls >= 2, "expected an initial call plus at least one band");
}

// ---- 3x export equals nearest-neighbour upscale, across a band boundary -----------

#[test]
fn three_x_world_export_equals_upscaled_render_region_across_band_boundaries() {
    let (pack, comp) = yellow();
    let world = xpr_map::geom::scope_rect(&pack, Scope::World);
    // y 200..900 is 700 rows: three 256-row bands (200..456, 456..712,
    // 712..900), crossing band boundaries at world y=456 and y=712.
    let rect = IRect::new(0, 200, 512.min(world.x1), 900).intersect(&world);
    assert!(rect.height() > 256 * 2, "region should span at least two band boundaries");

    let req = ExportRequest { scope: Scope::World, rect, scale: 3, opts: RenderOpts::default(), transparent: false };
    let out = render(&comp, &req, &mut |_| true).expect("export should succeed");

    // Reference: a single whole-region render_region, upscaled in one shot.
    let base = comp.render_region(Scope::World, rect, RenderOpts::default());
    let (out_w, out_h) = output_size(rect, 3);
    let mut expect = xpr_map::Pixmap::new(out_w as usize, out_h as usize);
    upscale_into(&mut expect, 0, 0, &base, 3);

    assert_eq!((out.w, out.h), (out_w as usize, out_h as usize));
    assert_eq!(out.data, expect.data, "banded 3x export must equal a single render_region + upscale, with no seams at band boundaries");
}

// ---- transparency -------------------------------------------------------------------

/// Find a small world-pixel region that straddles a placed map's edge: one
/// point inside the map (bbox-contained, so it renders opaque even where a
/// map's own art has transparent-looking tiles) and one point just outside
/// every map's rect (open world space, confirmed via `map_at_world_px`
/// rather than `Compositor::covers` so this doesn't just test `covers`
/// against itself). Dynamic rather than hard-coded so it keeps working if
/// the pack's world layout is regenerated.
fn straddling_world_rect(pack: &MapPack) -> IRect {
    let world = xpr_map::geom::scope_rect(pack, Scope::World);
    for (mrect, _id) in &pack.layout.placed_rects {
        if mrect.width() < 8 || mrect.height() < 8 {
            continue;
        }
        let inside = (mrect.x0 + mrect.width() / 2, mrect.y0 + mrect.height() / 2);
        let outside_candidates = [(mrect.x0 - 24, inside.1), (mrect.x1 + 23, inside.1), (inside.0, mrect.y0 - 24), (inside.0, mrect.y1 + 23)];
        for &(ox, oy) in &outside_candidates {
            if !world.contains(ox, oy) {
                continue;
            }
            if map_at_world_px(pack, Scope::World, ox, oy).is_some() {
                continue; // still on some map (an adjoining one) - try another side
            }
            let pad = 8;
            let r = IRect::new(inside.0.min(ox) - pad, inside.1.min(oy) - pad, inside.0.max(ox) + pad, inside.1.max(oy) + pad).intersect(&world);
            return r;
        }
    }
    panic!("no map edge with adjoining open world space found in the Yellow pack");
}

#[test]
fn transparent_export_is_alpha_zero_off_map_and_opaque_on_map() {
    let (pack, comp) = yellow();
    let rect = straddling_world_rect(&pack);

    let req = ExportRequest { scope: Scope::World, rect, scale: 1, opts: RenderOpts::default(), transparent: true };
    let out = render(&comp, &req, &mut |_| true).unwrap();

    let mut saw_on = false;
    let mut saw_off = false;
    for y in 0..out.h {
        for x in 0..out.w {
            let wx = rect.x0 + x as i32;
            let wy = rect.y0 + y as i32;
            let on_map = map_at_world_px(&pack, Scope::World, wx, wy).is_some();
            let px = out.get(x, y);
            if on_map {
                saw_on = true;
                assert_eq!(px[3], 255, "on-map pixel ({}, {}) should be opaque, got {:?}", wx, wy, px);
            } else {
                saw_off = true;
                assert_eq!(px, [0, 0, 0, 0], "off-map pixel ({}, {}) should be alpha 0, got {:?}", wx, wy, px);
            }
        }
    }
    assert!(saw_on && saw_off, "the region should contain both on-map and off-map pixels");
}

#[test]
fn opaque_export_paints_the_world_background_off_map() {
    let (pack, comp) = yellow();
    let rect = straddling_world_rect(&pack);

    let req = ExportRequest { scope: Scope::World, rect, scale: 1, opts: RenderOpts::default(), transparent: false };
    let out = render(&comp, &req, &mut |_| true).unwrap();

    let mut saw_off = false;
    for y in 0..out.h {
        for x in 0..out.w {
            let wx = rect.x0 + x as i32;
            let wy = rect.y0 + y as i32;
            if map_at_world_px(&pack, Scope::World, wx, wy).is_none() {
                saw_off = true;
                assert_eq!(out.get(x, y), BACKGROUND, "off-map pixel ({}, {}) should be BACKGROUND when not transparent", wx, wy);
            }
        }
    }
    assert!(saw_off);
}

// ---- cancel ---------------------------------------------------------------------------

#[test]
fn cancelling_after_the_first_band_stops_promptly() {
    let (pack, comp) = yellow();
    let world = xpr_map::geom::scope_rect(&pack, Scope::World);
    // The full world height (5760 rows -> 23 bands): if cancellation didn't
    // stop the loop, this would composite the whole world at 1x.
    let rect = IRect::new(0, 0, 256.min(world.x1), world.y1);
    assert!(rect.height() as i64 > 256 * 10, "region should span many bands so an early cancel actually saves work");

    let req = ExportRequest { scope: Scope::World, rect, scale: 1, opts: RenderOpts::default(), transparent: false };
    let mut calls = 0;
    let t = Instant::now();
    let result = render(&comp, &req, &mut |_f| {
        calls += 1;
        calls <= 1 // true once (the pre-render call), false from then on
    });
    let elapsed = t.elapsed();

    assert!(matches!(result, Err(ExportError::Cancelled)), "expected Cancelled, got {:?}", result.map(|p| (p.w, p.h)));
    assert!(calls <= 2, "expected cancellation after at most one band, got {} progress calls", calls);
    assert!(elapsed < Duration::from_secs(5), "cancel should return quickly, took {:?}", elapsed);
}

// ---- Emerald's whole world -------------------------------------------------------------

/// 16064x8544 = 137 Mpx, under MAX_PIXELS (268,435,456) at 1x. Slow in an
/// unoptimised debug build, so this is gated behind `--ignored`; run
/// `cargo test -p xpr-map --release -- --ignored emerald_world` to see the
/// timing (reported via `eprintln!`, not an assertion).
#[test]
#[ignore = "137 Mpx full-world composite: slow in debug, run with --release -- --ignored to report timing"]
fn emerald_world_1x_stays_under_the_pixel_bound_and_renders() {
    let pack = Arc::new(MapPack::load("emerald", &source()).unwrap());
    let comp = Compositor::new(pack.clone());
    let rect = xpr_map::geom::scope_rect(&pack, Scope::World);
    assert_eq!((rect.width(), rect.height()), (16064, 8544));
    let pixels = output_pixels(rect, 1);
    eprintln!("emerald world: {} x {} = {} px", rect.width(), rect.height(), pixels);
    assert!(pixels < MAX_PIXELS, "{} px should be under MAX_PIXELS {}", pixels, MAX_PIXELS);

    let req = ExportRequest { scope: Scope::World, rect, scale: 1, opts: RenderOpts::default(), transparent: false };
    let t = Instant::now();
    let out = render(&comp, &req, &mut |_| true).expect("emerald world should export");
    eprintln!("emerald world 1x render: {:.2}s", t.elapsed().as_secs_f64());
    assert_eq!((out.w as i32, out.h as i32), (rect.width(), rect.height()));
}
