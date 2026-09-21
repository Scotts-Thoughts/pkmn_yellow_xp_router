//! Print the comparison of two route files, for checking the engine against
//! `docs/rust_port/design/route_compare/SPEC.md` §9.3 by hand.
//!
//! `cargo run -p xpr-engine --example compare_dump -- a.json b.json`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use xpr_data::Registry;
use xpr_engine::compare::{compare, digest, format_time, text_summary, RouteOrigin};
use xpr_engine::Router;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: compare_dump <route a> <route b>");
        std::process::exit(2);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));

    let mut digests = Vec::new();
    for p in &args[..2] {
        let mut router = Router::new(reg.clone());
        router.load(Path::new(p), false).unwrap_or_else(|e| panic!("{p}: {e}"));
        let label = Path::new(p).file_stem().unwrap().to_string_lossy().to_string();
        digests.push(digest(&router, &label, RouteOrigin::External));
    }
    let (a, b) = (digests.remove(0), digests.remove(0));

    for (tag, d) in [("A", &a), ("B", &b)] {
        println!(
            "{tag} {} — {} {} · {} events in {} folders · {} disabled · final {}",
            d.label,
            d.version,
            d.species,
            d.entries.len(),
            d.folder_count,
            d.disabled_events,
            d.final_time.map(format_time).unwrap_or_else(|| "—".into())
        );
        println!(
            "  {} {:?} (total {}) · HP {:?} · {:?} {:?}/{:?} · {:?}",
            d.dv_text(),
            d.dvs,
            d.dv_total(),
            d.hidden_power,
            d.nature,
            d.nature_up,
            d.nature_down,
            d.ability
        );
        println!(
            "  end: Lv {} · stats {:?} · {} {:?} (total {}) · ${} · held {:?}",
            d.end.level,
            d.end.stats,
            d.ev_text(),
            d.end.evs,
            d.end.ev_total(),
            d.end.money,
            d.end.held_item
        );
        let t = &d.totals;
        println!(
            "  trainers {} (major {}, mons {}) · wild {} ({} species) · candies {} · vitamins {} {:?} · berries {}",
            t.trainers, t.major_fights, t.trainer_pokemon, t.wild_pokemon, t.wild_species, t.rare_candies, t.vitamins, t.vitamins_by_name, t.ev_berries
        );
        println!(
            "  moves {} (lvl {} / tm {} / tutor {}) · items {} · buy {} · sell {} · use {} · hold {} · heals {} · saves {} · blackouts {} · errors {}",
            t.moves_learned(),
            t.moves_level_up,
            t.moves_tm_hm,
            t.moves_tutor,
            t.items_picked_up,
            t.purchases,
            t.sales,
            t.items_used,
            t.held_item_changes,
            t.heals,
            t.saves,
            t.blackouts,
            t.events_with_errors
        );
        println!(
            "  exp trainers/wild/candy/other {}/{}/{}/{} · money trainers/sales/spent/blackouts/other {}/{}/{}/{}/{}",
            t.xp_from_trainers,
            t.xp_from_wild,
            t.xp_from_candies,
            t.xp_from_other,
            t.money_from_trainers,
            t.money_from_sales,
            t.money_spent,
            t.money_lost_blackouts,
            t.money_other
        );
    }

    let cmp = compare(a, b);
    let s = &cmp.trainer_sets;
    println!(
        "\nsets: same order {} · different order {} · only A {} · only B {} · repeat A {} · repeat B {}",
        s.same_order,
        s.different_order,
        s.only_a.len(),
        s.only_b.len(),
        s.repeat_a.len(),
        s.repeat_b.len()
    );
    println!(
        "rows {} · shared fights {} · blocks with differences {} · checkpoints {}",
        cmp.rows.len(),
        cmp.shared_fight_count(),
        cmp.blocks_with_differences(),
        cmp.checkpoints.len()
    );
    println!("repeat A: {:?}", s.repeat_a);
    println!("only A: {:?}", s.only_a);
    println!("only B: {:?}", s.only_b);
    println!("trainer-owned singles: A {} · B {}", cmp.a.totals.trainer_owned_singles, cmp.b.totals.trainer_owned_singles);

    println!("\nmajor-fight checkpoints:");
    for cp in cmp.checkpoints.iter().filter(|c| cmp.a.entries[c.a].is_major) {
        let (ea, eb) = (&cmp.a.entries[cp.a], &cmp.b.entries[cp.b]);
        println!(
            "  {:26} L{:>2} vs L{:>2} | EVs {:>3} vs {:>3} | ${:>6} vs ${:>6} | {:>9} vs {:>9} | {} moves differ{}",
            ea.label,
            ea.before.level,
            eb.before.level,
            ea.before.ev_total(),
            eb.before.ev_total(),
            ea.before.money,
            eb.before.money,
            ea.recorded_secs.map(format_time).unwrap_or_else(|| "—".into()),
            eb.recorded_secs.map(format_time).unwrap_or_else(|| "—".into()),
            cp.moves_differing,
            if cp.same_order { "" } else { " (different order)" }
        );
    }

    println!("\n{}", text_summary(&cmp));
}
