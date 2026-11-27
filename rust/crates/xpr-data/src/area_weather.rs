//! Persistent area weather: the battle weather a trainer fight starts with
//! because of the map it is fought on (Mt. Silver's hail in HG/SS, Route
//! 119's rain in gen 3, ...). Only weather active more than half the time
//! there is listed; the research behind each entry is in
//! `docs/rust_port/design/area_weather.md`.
//!
//! The table (`area_weather.json`, compiled in) maps game groups to
//! `locations` (a trainer's `trainer_location`) and `trainers` (an exact
//! trainer name, which wins over its location; `"None"` excludes one).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;
use xpr_core::consts;

static TABLE_JSON: &str = include_str!("area_weather.json");

#[derive(Default)]
struct GameWeather {
    locations: HashMap<String, &'static str>,
    trainers: HashMap<String, &'static str>,
}

/// The weather constant a table value names (`None` for an unknown name).
fn weather_const(name: &str) -> Option<&'static str> {
    [
        consts::WEATHER_NONE,
        consts::WEATHER_RAIN,
        consts::WEATHER_SUN,
        consts::WEATHER_SANDSTORM,
        consts::WEATHER_HAIL,
        consts::WEATHER_FOG,
    ]
    .into_iter()
    .find(|w| *w == name)
}

fn parse_table(text: &str) -> Result<HashMap<String, GameWeather>, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let groups = root.get("groups").and_then(Value::as_array).ok_or("area_weather.json: no `groups` array")?;
    let mut result: HashMap<String, GameWeather> = HashMap::new();
    for group in groups {
        let mut entry = GameWeather::default();
        for (key, target) in [("locations", &mut entry.locations), ("trainers", &mut entry.trainers)] {
            if let Some(map) = group.get(key).and_then(Value::as_object) {
                for (name, w) in map {
                    let w = w.as_str().unwrap_or_default();
                    let w = weather_const(w).ok_or_else(|| format!("area_weather.json: unknown weather {:?} for {:?}", w, name))?;
                    target.insert(name.clone(), w);
                }
            }
        }
        let games = group.get("games").and_then(Value::as_array).ok_or("area_weather.json: group without `games`")?;
        for game in games {
            let game = game.as_str().ok_or("area_weather.json: game names must be strings")?;
            let e = result.entry(game.to_string()).or_default();
            e.locations.extend(entry.locations.iter().map(|(k, v)| (k.clone(), *v)));
            e.trainers.extend(entry.trainers.iter().map(|(k, v)| (k.clone(), *v)));
        }
    }
    Ok(result)
}

fn table() -> &'static HashMap<String, GameWeather> {
    static TABLE: OnceLock<HashMap<String, GameWeather>> = OnceLock::new();
    TABLE.get_or_init(|| {
        parse_table(TABLE_JSON).unwrap_or_else(|e| {
            log::error!("{}", e);
            HashMap::new()
        })
    })
}

/// The persistent weather of a trainer fight in `version` (a built-in
/// version name), or `None` when the area has none.
pub fn area_weather(version: &str, trainer_location: &str, trainer_name: &str) -> Option<&'static str> {
    let game = table().get(version)?;
    let w = game.trainers.get(trainer_name).or_else(|| game.locations.get(trainer_location)).copied()?;
    (w != consts::WEATHER_NONE).then_some(w)
}

/// Whether any area of `version` has persistent weather (not gen 1/2 or
/// FireRed/LeafGreen); the battle page hides its weather dropdown otherwise.
pub fn has_area_weather(version: &str) -> bool {
    table().get(version).is_some_and(|g| g.locations.values().chain(g.trainers.values()).any(|w| *w != consts::WEATHER_NONE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_parses() {
        parse_table(TABLE_JSON).unwrap();
    }

    #[test]
    fn trainer_entry_wins_over_location() {
        let t = parse_table(
            r#"{"groups": [{"games": ["X"], "locations": {"Peak": "Hail"}, "trainers": {"Hiker A": "None", "Hiker B": "Fog"}}]}"#,
        )
        .unwrap();
        let g = &t["X"];
        assert_eq!(g.locations["Peak"], consts::WEATHER_HAIL);
        assert_eq!(g.trainers["Hiker A"], consts::WEATHER_NONE);
        assert_eq!(g.trainers["Hiker B"], consts::WEATHER_FOG);
    }

    #[test]
    fn only_games_with_weathered_areas_have_it() {
        for v in [consts::CRYSTAL_VERSION, consts::GOLD_VERSION, consts::YELLOW_VERSION, consts::FIRE_RED_VERSION, consts::LEAF_GREEN_VERSION] {
            assert!(!has_area_weather(v), "{v}");
        }
        for v in [consts::RUBY_VERSION, consts::EMERALD_VERSION, consts::PLATINUM_VERSION, consts::HEART_GOLD_VERSION, consts::BLACK_VERSION, consts::BLACK_2_VERSION] {
            assert!(has_area_weather(v), "{v}");
        }
    }

    #[test]
    fn red_fights_in_hail() {
        for v in [consts::HEART_GOLD_VERSION, consts::SOUL_SILVER_VERSION] {
            assert_eq!(area_weather(v, "Mt. Silver Cave", "Pokemon Trainer Red"), Some(consts::WEATHER_HAIL));
        }
    }
}
