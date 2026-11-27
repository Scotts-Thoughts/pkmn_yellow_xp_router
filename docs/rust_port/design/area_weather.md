# Persistent area weather

The battle page's **Weather** dropdown (controls bar, beside **Held**) holds the
weather that lasts the whole battle because of where it is fought. It only shows
in games that have such an area (not gens 1-2 or FireRed/LeafGreen). Trainer
fights in the areas below get it by default; `xpr_data::area_weather` reads the
table from `rust/crates/xpr-data/src/area_weather.json`.

- A route stores `field_weather` only when the user's pick differs from the
  area default, so existing routes pick up the defaults, and a table fix
  reaches them too.
- A weather move's checkbox (Sunny Day, ...) overrides the area weather from
  its matchup on. Turning it off brings the area weather back.
- **Rule:** an area is listed only when its battle weather is active more than
  50% of the time when the trainer is fought. Interiors never have weather.
- Researched 2026-11-25 from the decomps (gen 3, Platinum, HG/SS; DP from
  upstream pret/pokediamond) and from web sources for gen 5.

## How each generation turns overworld weather into battle weather

- **Gen 2:** none. Overworld weather doesn't exist.
- **Gen 3** (`battle_util.c` switch-in weather, all three decomps): rain,
  thunderstorm and downpour give permanent Rain; sandstorm gives Sandstorm;
  drought gives sun. Snow, ash, fog and shade give nothing; there is no hail.
- **Gen 4** (Platinum `battle_lib.c:3660-3710`, HG/SS
  `overlay_12_0224E4FC.c:3143-3180`): rain types give Rain, snow and blizzard
  give Hail, sandstorm gives Sandstorm, fog gives Fog, all permanent. Diamond
  dust gives nothing. DP's conversion is inferred from Platinum's.
- **Gen 5:** only permanent sandstorms pass the rule. Seasonal rain and winter
  hail fall well under 50%.

## Areas

### Ruby, Sapphire

- Location **Route119**: Rain
- No weather (overrides the location): Aqua Admin Shelly, Magma Admin Courtney, Team Aqua Grunt 16, Team Aqua Grunt 17, Team Aqua Grunt 18, Team Aqua Grunt 25, Team Magma Grunt 16, Team Magma Grunt 17, Team Magma Grunt 18, Team Magma Grunt 25
- Rain: Bird Keeper Colin, Cool Trainer Jennifer, Ninja Boy Tsunao, Parasol Lady Angelica, Pokemon Ranger Carlos, Pokemon Ranger Jenna, Ruin Maniac Chip
- Sandstorm: Camper Cliff, Camper Drew, Picnicker Becky, Picnicker Heidi, Ruin Maniac Dusty, Ruin Maniac Dusty Rematch 1, Ruin Maniac Dusty Rematch 2, Ruin Maniac Dusty Rematch 3, Ruin Maniac Dusty Rematch 4

Evidence:

- pokeruby/src/battle_util.c:1792-1830 - switch(GetCurrentWeather()) at battle start (gLastUsedAbility 0xFF): WEATHER_RAIN_LIGHT/RAIN_MED/RAIN_HEAVY -> rain, WEATHER_SANDSTORM -> sandstorm, WEATHER_DROUGHT -> sun. Ash, fog, snow, shade, clouds, bubbles give nothing; no hail case.
- pokeruby/src/field_weather_effects.c:2339-2351,2373-2374,2381-2383 - Route119 cycle = [SUNNY, RAIN_LIGHT, RAIN_MED, RAIN_LIGHT] (3 of 4 stages rain = 75%); Route123 cycle = [SUNNY, SUNNY, RAIN_LIGHT, SUNNY] (25% rain, excluded); stage advances per day, % 4.
- pokeruby/data/maps/Route119/scripts.inc:31 + pokeruby/src/field_specials.c:1841-1846 - OnTransition sets saved weather to WEATHER_ROUTE119_CYCLE. map.json weather triggers: cycle triggers on the y 130-133 diagonal and y=13, SUNNY triggers only near Route 118 (y=137 row) and the Fortree end (x 35-36, y 6-9). BFS over the layout: every Route 119 trainer is fought under the cycle weather on the normal south->north walk.
- pokeruby/data/maps/Route119_WeatherInstitute_1F|2F/scripts.inc - the 5 Weather Institute fights are indoors (Ruby: Team Magma Grunt 16/17/18/25 + Magma Admin Courtney; Sapphire: Team Aqua Grunt 16/17/18/25 + Aqua Admin Shelly), so overridden to None although trainer_location is Route119.
- pokeruby/data/maps/Route111/scripts.inc:16-36 (OnTransition: y 34..107 and (y>72 or x>=8) -> setweather SANDSTORM), :221-230 (sun/sandstorm trigger scripts); map.json coord_events: sandstorm triggers (10-14,61-62) and diagonal (11,44)-(17,38); sun triggers NW diagonal (9,37),(10,36),(17,31),(18,32) and SW diagonal (7,63)-(14,69). Deep-sand layout: x 9-35,y 37-60 and x 21-37,y 61-103, lower desert is a dead end (TM Sandstorm at (33,104)). Desert trainers by object_event coords: Heidi (28,51), Drew (24,60), Cliff (21,47), Dusty (27,69), Becky (32,72) = sandstorm. Excluded: Travis (11,71), Irene (10,82) in the west corridor (sunny when entered from Mauville); Daisuke (32,29), Wilton (9,27), Brooke (11,12) north of y=34; Winstrate family inside a house.
- pokeruby/data/maps/Route120/scripts.inc:73-105 (OnTransition: y<=14 sunny; y 15-60: x<=7 rain, x 8-19 sunny, else rain; y>=61 clouds) + map.json coord_events: RAIN_LIGHT at (7,15),(7,16),(22,61),(12,64); SUNNY (28,15-17); CLOUDS (35-38,63),(10-13,75),(32,88-91). BFS from the Fortree end (sunny start): the only way south passes (7,15)/(7,16), so every trainer y>=17 down to y 60 is in rain; from the Route 121 end Chip is rain. Rain: Colin (5,22), Tsunao (19,28), Angelica (18,34), Jennifer (28,35), Jenna (36,45), Carlos (27,51), Chip (9,60). Not included: Clarissa (16,6), Robert (32,14) sunny (y<=14); Brandon (19,80) clouds; Keigo (10,72) rain or clouds depending on walk direction.
- pokeruby Route113/JaggedPass/MtChimney use WEATHER_ASH (no battle weather); Mt Pyre Summit, Cave of Origin, Rusturf, Lavaridge Gym use fog (no effect); underwater maps bubbles (no effect). Route 123 cycle only 25% rain -> excluded.
- pokeruby covers both games; Weather Institute grunts differ (Ruby: Team Magma, Sapphire: Team Aqua) so both sets are overridden to None (the other game's names only exist with trainer_location None, so the overrides are harmless there).

### Emerald

- Location **Route119**: Rain
- No weather (overrides the location): Aqua Admin Shelly, Team Aqua Grunt 11, Team Aqua Grunt 12, Team Aqua Grunt 13, Team Aqua Grunt 19, Team Aqua Grunt 26
- Rain: Battle Girl Callie, Bird Keeper Colin, Cooltrainer Jennifer, Cooltrainer Leonel, Ninja Boy Riley, Parasol Lady Angelica, Pkmn Ranger Jenna, Pkmn Ranger Lorenzo, Ruin Maniac Chip
- Sandstorm: Camper Beau, Camper Branden, Camper Drew, Picnicker Becky, Picnicker Celia, Picnicker Heidi, Ruin Maniac Bryan, Ruin Maniac Dusty, Ruin Maniac Dusty Rematch 1, Ruin Maniac Dusty Rematch 2, Ruin Maniac Dusty Rematch 3, Ruin Maniac Dusty Rematch 4

Evidence:

- pokeemerald/src/battle_util.c:2473-2512 - ABILITYEFFECT_SWITCH_IN_WEATHER: switch(GetCurrentWeather()): WEATHER_RAIN, WEATHER_RAIN_THUNDERSTORM, WEATHER_DOWNPOUR -> B_WEATHER_RAIN; WEATHER_SANDSTORM -> B_WEATHER_SANDSTORM; WEATHER_DROUGHT -> B_WEATHER_SUN. Everything else (volcanic ash, snow, fog, shade, sunny, clouds, underwater bubbles, abnormal) -> none; no hail case. Skipped only for BATTLE_TYPE_RECORDED. Runs for every battle (battle_main.c:3863-3864).
- pokeemerald/src/field_weather_effect.c:2579-2593,2616-2617,2622-2627 - Route119 cycle = [SUNNY, RAIN, RAIN_THUNDERSTORM, RAIN] (75% rain); Route123 cycle = [SUNNY, SUNNY, RAIN, SUNNY] (25%, excluded); stage advances per day (% WEATHER_CYCLE_LENGTH=4).
- pokeemerald/data/maps/Route119/scripts.inc:20 + pokeemerald/src/field_specials.c:1519-1523 - OnTransition SetRoute119Weather. map.json: SUNNY triggers only at (35-36,6-9) near Fortree and the y=137 row near Route 118; cycle triggers at (29-32,13),(34,13) and the y 130-133 diagonal. BFS over layout: all Route 119 trainers are fought under the cycle weather on the normal south->north walk (Hideo/Yasu/Fabian are only sunny when coming back from Fortree).
- pokeemerald/data/maps/Route119_WeatherInstitute_1F|2F/scripts.inc - TRAINER_GRUNT_WEATHER_INST_1..5 + TRAINER_SHELLY_WEATHER_INSTITUTE are indoors -> Team Aqua Grunt 11/12/13/19/26 and Aqua Admin Shelly overridden to None. Rival Brendan/May 4 on Route 119 (Route119/scripts.inc:78-118) are outdoors -> rain via the location entry.
- pokeemerald/data/maps/Route111/scripts.inc:46,65-75 (OnTransition CheckSetSandstorm: y 34..107 and (y>72 or x>=8) -> sandstorm), :238-249 SunTrigger/SandstormTrigger; map.json coord_events: sandstorm triggers (10-14,61-62) + diagonal (11,44)-(17,38); sun triggers NW diagonal (9,37),(10,36),(17,31),(18,32) and SW diagonal (7,63)-(14,69). Deep-sand layout x 9-35,y 37-60 and x 21-37,y 61-103; lower desert is a dead end reached only from the north desert. Sandstorm trainers (object_event coords): Heidi (28,51), Drew (29,37), Beau (21,47), Becky (32,66), Dusty (27,69) + rematches, Bryan (29,77), Celia (22,77), Branden (37,77). Not included: Travis (11,71), Irene (10,82) in the sunny west corridor; Daisuke (32,29), Wilton (9,27), Brooke (11,11) north of y=34; Tyron, Celina, Bianca, Hayden (y>=119) south of y=107; Winstrate family (house).
- pokeemerald/data/maps/Route120/scripts.inc:56-85 (OnTransition SetWeather: y<=14 sunny; y 15-60: x<=7 rain, x 8-19 sunny, else rain; y>=61 sunny clouds) + events.inc:58-61 and map.json: RAIN at (7,15),(7,16),(22,61),(12,64); SUNNY (28,15-17); SUNNY_CLOUDS (35-38,63),(10-13,75),(32,88-91). BFS from the Fortree end (sunny start): the only way south passes (7,15)/(7,16) -> rain for Colin (5,22), Riley (19,28), Callie (19,32), Angelica (18,34), Leonel (14,34), Jennifer (31,37), Jenna (36,45), Lorenzo (27,51); from the Route 121 end Chip (9,60) is rain. Not included: Clarissa (16,6), Robert (32,14) sunny; Jeffrey (19,80) clouds; Keigo (10,72) rain or clouds depending on direction.
- Route113/JaggedPass/MtChimney use WEATHER_VOLCANIC_ASH (no battle weather); Mt Pyre Summit, Cave of Origin, Terra/Marine Cave end, Lavaridge Gym, Rusturf Tunnel use fog (no effect); underwater maps bubbles (no effect). Route123 cycle 25% rain -> excluded. Abnormal weather (Routes 105/114/115/116/118/125/127/129, data/scripts/abnormal_weather.inc:165-171 DOWNPOUR/DROUGHT) is a postgame event active at one location at a time (not >50%); Sootopolis downpour/drought only affects wild legendary fights (no outdoor trainers); WEATHER_ABNORMAL (Sky Pillar) has no battle weather.

### FireRed, LeafGreen

No area has battle weather.


Evidence:

- pokefirered/src/battle_util.c:1707-1745 - same switch(GetCurrentWeather()) (RAIN/RAIN_THUNDERSTORM/DOWNPOUR -> rain, SANDSTORM -> sandstorm, DROUGHT -> sun).
- pokefirered data/maps/*/map.json and scripts - no map has rain/sandstorm/drought weather: only WEATHER_FOG_HORIZONTAL (Pokemon Tower 3F-7F, Five Island Lost Cave rooms; no battle effect); the only setweather uses are WEATHER_SHADE (data/event_scripts.s:1308) and 0 (NavelRock_Summit). WEATHER_RAIN/SANDSTORM/DROUGHT are marked unused in include/constants/weather.h:7,12,16. No FRLG trainer fight starts with battle weather.

### Diamond, Pearl

- Location **Route 215**: Rain
- Location **Route 216**: Hail
- Location **Route 217**: Hail
- Location **Route 228**: Sandstorm
- Fog: Ace Trainer Alyssa, Ace Trainer Arthur, Ace Trainer Brandi, Ace Trainer Clarice, Ace Trainer Ernest, Ace Trainer Micah, Bird Keeper Autumn, Bird Keeper Brianna, Bird Keeper Brianna Rematch 2, Bird Keeper Brianna Rematch 3, Black Belt Adam, Black Belt Eddie, Black Belt Willie, Double Team Zac & Jen, Dragon Tamer Joe, Dragon Tamer Patrick, Dragon Tamer Patrick Rematch 2, Dragon Tamer Patrick Rematch 3, Ninja Boy Davido, Ninja Boy Joel, Ninja Boy Nathan, Psychic Deandre, Psychic Desiree, Psychic Kendra, Psychic Landon, Veteran Brenden, Veteran Brian, Veteran Brian Rematch 2, Veteran Brian Rematch 3, Veteran Terrell
- Rain: Collector Dominique, Fisherman Cameron, Fisherman Juan, Fisherman Travis, Parasol Lady Alexa, Parasol Lady Alexa Rematch 2, Parasol Lady Alexa Rematch 3, Parasol Lady Alexa Rematch 4, Parasol Lady Sabrina, Pokemon Ranger Allison, Pokemon Ranger Allison Rematch 2, Pokemon Ranger Allison Rematch 3, Pokemon Ranger Allison Rematch 4, Pokemon Ranger Jeffrey, Pokemon Ranger Taylor, Pokemon Ranger Taylor Rematch 2, Pokemon Ranger Taylor Rematch 3, Pokemon Ranger Taylor Rematch 4, Policeman Danny, Policeman Danny Rematch 2, Policeman Danny Rematch 3, Policeman Danny Rematch 4, Scientist Shaun, Scientist Shaun Rematch 2, Scientist Shaun Rematch 3, Scientist Shaun Rematch 4, Scientist Stefano

Evidence:

- The path A:/Dropbox/stp-projects/programs/poke_map/repos/pokediamond does NOT exist on this machine (the docs say it is only on the Mac). Used the identical upstream https://github.com/pret/pokediamond (master) via read-only HTTPS.
- pokediamond has no decompiled battle code, so the overworld->battle weather switch could not be read there. It is inferred from the same Gen 4 battle code: Platinum src/battle/battle_lib.c:3660-3710 and HeartGold A:/decomps/pokeheartgold/src/battle/overlay_12_0224E4FC.c:3140-3175 (rain/heavy rain/thunderstorm -> overworld rain; snow/light snow/blizzard -> overworld hail; sandstorm; fog; diamond dust, ash, cloudy -> nothing; weather is permanent).
- arm9/src/map_header.c (struct include/map_header.h: weather_type after mapsec): line 447 MAP_ROUTE_210_NORTH=14 (fog), 455 MAP_ROUTE_212_SOUTH=24 (yearly col 0), 457 MAP_ROUTE_213=25, 466 MAP_ROUTE_215=2 (rain), 467 MAP_ROUTE_216=26, 469 MAP_ROUTE_217=7 (blizzard), 490 MAP_ROUTE_228=10 (sandstorm), 331 MAP_VICTORY_ROAD_1F_BACK_2=14 (fog), 424 MAP_ACUITY_LAKEFRONT=27, 249 MAP_SNOWPOINT=28; Lost Tower and the other Platinum-only fog/cave maps have weather 0 in DP.
- arm9/src/field_system_rtc_weather.c:13 sWeatherTable[366][5] (weather>=24 indexes it by day of year): same values as Platinum. Counted days: col0 (Route 212 S) rain-type 365/366, col1 (Route 213) rain 10/366, col2 (Route 216) snow-type 366/366, col3 (Acuity) 366/366, col4 (Snowpoint) snow-type 357/366 (9 days diamond dust).
- Trainer->map: arm9/ zone event bins files/fielddata/eventdata/zone_event_release/narc_NNNN.bin (NNNN from the map header events_bank); object event script id 3000+(rom_id-1) = trainer (5000+(rom_id-1) = double-battle partner); verified on Platinum-identical positions (e.g. script 3065 at 534,522 on Route 210 North = Ace Trainer Ernest, rom_id 66). Rematch trainers inherit the base trainer's map.
- Result matches Platinum for the routes: Route 210 North (fog), Route 212 South (rain), Route 215 (rain), 216/217 (hail), 228 (sandstorm), Victory Road 1F back room 2 (fog; 14 trainers, Pokemon Trainer Marley is not placed in DP). No Lost Tower fog in DP. Snowpoint City trainers are all inside the gym; Leader Candice is a gym leader.

### Platinum

- Location **Route 215**: Rain
- Location **Route 216**: Hail
- Location **Route 217**: Hail
- Location **Route 228**: Sandstorm
- Fog: Ace Trainer Alyssa, Ace Trainer Arthur, Ace Trainer Brandi, Ace Trainer Clarice, Ace Trainer Ernest, Ace Trainer Micah, Belle & Pa Beth & Bob, Bird Keeper Autumn, Bird Keeper Brianna, Bird Keeper Brianna Rematch 2, Bird Keeper Brianna Rematch 3, Black Belt Adam, Black Belt Eddie, Black Belt Willie, Double Team Zac & Jen, Dragon Tamer Joe, Dragon Tamer Patrick, Dragon Tamer Patrick Rematch 2, Dragon Tamer Patrick Rematch 3, Ninja Boy Davido, Ninja Boy Joel, Ninja Boy Nathan, Pokefan Leonard, Pokefan Rebekah, Pokemon Trainer Marley, Psychic Deandre, Psychic Desiree, Psychic Kendra, Psychic Landon, Roughneck Kirby, Veteran Brenden, Veteran Brian, Veteran Brian Rematch 2, Veteran Brian Rematch 3, Veteran Terrell, Young Couple Mike & Nat
- Rain: Collector Dean, Fisherman Josh, Fisherman Juan, Fisherman Travis, Parasol Lady Alexa, Parasol Lady Alexa Rematch 2, Parasol Lady Alexa Rematch 3, Parasol Lady Alexa Rematch 4, Parasol Lady Sabrina, Pokemon Ranger Allison, Pokemon Ranger Allison Rematch 2, Pokemon Ranger Allison Rematch 3, Pokemon Ranger Allison Rematch 4, Pokemon Ranger Jeffrey, Pokemon Ranger Taylor, Pokemon Ranger Taylor Rematch 2, Pokemon Ranger Taylor Rematch 3, Pokemon Ranger Taylor Rematch 4, Policeman Danny, Policeman Danny Rematch 2, Policeman Danny Rematch 3, Policeman Danny Rematch 4, Scientist Shaun, Scientist Shaun Rematch 2, Scientist Shaun Rematch 3, Scientist Shaun Rematch 4, Scientist Stefano

Evidence:

- src/battle/battle_lib.c:3660-3710 - BattleSystem_TriggerEffectOnSwitch (switch-in check state FIELD_WEATHER) maps BattleSystem_GetFieldWeather(): OVERWORLD_WEATHER_RAINING/HEAVY_RAIN/THUNDERSTORM -> subscript_overworld_rain (Rain); SNOWING/HEAVY_SNOW/BLIZZARD -> subscript_overworld_hail (Hail); SANDSTORM -> sand; FOG/DEEP_FOG -> subscript_overworld_fog; HARSH_SUN (1001, Battle Frontier only) -> sun; TRICK_ROOM (1002). Everything else (CLEAR, CLOUDY, HAILING=11 'diamond dust', SLOW_ASHFALL, 8, 13, 23, 26-30...) gives no battle weather.
- src/field_battle_data_transfer.c:267 - dto->fieldWeather = FieldOverworldState_GetWeather(fieldOverworldState); src/battle/battle_main.c:1059 battleSys->fieldWeather = dto->fieldWeather
- src/field_map_change.c:273-280 - weather = FieldSystem_GetWeather(map header weather); FOG is replaced by CLEAR when the Defog flag is active (and DARK_FLASH by Flash); stored with FieldOverworldState_SetWeather on every map change
- res/battle/scripts/subscripts/subscript_overworld_rain.s:10 / _hail.s:11 / _sand.s:11 / _fog.s:11 / _sun.s:11 - set FIELD_CONDITION_RAINING_PERM / HAILING_PERM / SANDSTORM_PERM / DEEP_FOG / SUNNY_PERM, i.e. permanent (no turn counter); fog is Deep Fog (accuracy)
- src/field_overworld_weather.c:17-424 - sYearlyWeather[366][5] for Route 212 South / Route 213 / Route 216 / Acuity Lakefront / Snowpoint City (RTC date driven). Counted days: R212S Rain-type 365/366 (1 cloudy), R213 Rain-type 10/366 (rest clear/cloudy -> no weather), R216 hail-type 366/366, Acuity 366/366, Snowpoint hail-type 356/366 (10 days HAILING=11 which gives no battle weather). The save-penalty case uses Jan 2 (Route 212S rain, 216/Acuity/Snowpoint snow).
- include/data/map_headers.h:8043 ROUTE_210_NORTH=FOG; 8219 ROUTE_212_SOUTH=yearly(212S); 8263 ROUTE_213=yearly(213, mostly clear); 8461 ROUTE_215=RAINING; 8483 ROUTE_216=yearly(216); 8527 ROUTE_217=BLIZZARD; 8989 ROUTE_228=SANDSTORM; 5491 VICTORY_ROAD_1F_ROOM_2=FOG; 7955/7977/7999 ROUTE_209_LOST_TOWER_3F/4F/5F=FOG; 7537 ACUITY_LAKEFRONT, 3687 SNOWPOINT_CITY (no outdoor trainers), 4677 MT_CORONET_OUTSIDE_NORTH heavy snow (no trainers).
- Trainer->map: res/field/events/events_<map>.json object_events 'script': TRAINER_* (index in generated/trainers.txt == router rom_id) plus TRAINER_* constants in res/field/scripts/scripts_<map>.s, joined via the map header eventsArchiveID/scriptsArchiveID. Rematch trainers inherit the map of their base trainer. E.g. events_route_210_north.json:27 TRAINER_ACE_TRAINER_ERNEST (Route 210 North, fog); Route 210 South, Route 212 North, Route 213, Route 209 outdoors etc. are clear/no battle weather.
- Excluded (no battle weather): Route 213 (rain only 10/366 days), Stark Mountain/Route 227 (SLOW_ASHFALL), Route 206/208/Iron Island/Pokemon League (weather 8), Eterna Forest/Wayward Cave/Solaceon/Mt. Coronet interior/Lost Tower 2F (23/27/29/DARK_FLASH).

### HeartGold, SoulSilver

- Location **Lake of Rage**: Rain
- Location **Mt. Silver Cave**: Hail
- Location **Route 33**: Rain

Evidence:

- src/battle/battle_setup.c:247 - sub_02051D18 (called by BattleSetup_InitFromFieldSystem, line 258, used by SetupAndStartTrainerBattle in src/encounter.c:715) copies setup->weatherType = LocalFieldData_GetWeatherType(local), i.e. the current overworld weather, into the battle setup
- src/battle/overlay_12_0224E4FC.c:3143-3180 - TryAbilityOnEntry sendOutState 0 ('field weather') switches on BattleSystem_GetWeather: 1,2,3 (rain/heavy rain/thunderstorm) -> BATTLE_SUBSCRIPT_OVERWORLD_RAIN; 4,5,6 (storm?/snow/blizzard) -> OVERWORLD_HAIL; 7 (sandstorm) -> OVERWORLD_SAND; 9,10 (mist) -> OVERWORLD_FOG; 1001 -> OVERWORLD_SUN; 1002 -> trick room; everything else (0 sunny, 8 diamond dust, 11/12 flash, 13 low light) -> no battle weather. The weather is announced on the first send-out only. (battleSystem->weather is filled from setup->weatherType in non-decompiled overlay code; the C side only reads it, so that link is inferred from the identical value space)
- include/constants/weather.h:6-19 - overworld weather IDs (0 sunny, 1 rain, 2 heavy rain, 3 thunderstorm, 5 snow, 6 blizzard, 7 sandstorm, 8 diamond dust, 9/10 mist, 11/12 flash, 13 low light)
- files/battledata/script/subscript/subscript_0271_OverworldRain.s, _0272_OverworldHail.s, _0273_OverworldSand.s, _0294_OverworldSun.s - set FIELD_CONDITION_RAIN_PERMANENT / HAIL_PERMANENT / SANDSTORM_PERMANENT / SUN_PERMANENT (no turn counter, lasts the whole battle); _0274_OverworldFog.s sets FIELD_CONDITION_FOG
- src/data/map_headers.h:1143 - MAP_ROUTE_33 .weather = 1 (rain)
- src/data/map_headers.h:2673 - MAP_LAKE_OF_RAGE .weather = 1 (rain)
- src/data/map_headers.h:13983 - MAP_MOUNT_SILVER_CAVE_SUMMIT .weather = 5 (snow -> battle hail). Only other nonzero headers: MAP_SINJOH_RUINS_EXTERIOR 5 (no trainers on it), flash/low-light maps (11/13, no battle weather)
- src/field_warp_tasks.c:189-199 - the map's weather is computed on map load (FieldSystem_GetWeather_HandleDiamondDust) and stored with LocalFieldData_SetWeatherType; no other source of overworld weather exists (no weather script commands; only Flash sets 12 in src/scrcmd_c.c:2288)
- src/field_warp_tasks.c:190-192 and src/map_matrix.c:160-181 - Lake of Rage weather is forced to 0 (clear) only when ShouldUseAlternateLakeOfRage is true: after the Red Gyarados meeting (FLAG_RED_GYARADOS_MEET) AND the real-time day is Wednesday (the drained-lake day). So Lake of Rage is rainy 6 days of 7, and on Wednesdays after the Red Gyarados event it is clear (no battle weather)
- src/field_system_rtc_weather.c:22-40 - on MAP_MOUNT_SILVER_CAVE_SUMMIT the weather becomes 8 (diamond dust, no battle weather) on 8 specific real-time dates (Jan 1, Jan 31, Feb 1, Feb 29, Mar 15, Oct 10, Dec 3, Dec 31) unless FieldSystem_HasPenalty; on all other days Red starts with permanent hail
- files/fielddata/script/scr_seq/scr_seq_0107_D41R0108.s:17 - trainer_battle TRAINER_PKMN_TRAINER_RED_RED (rom_id 260) in map D41R0108 = MAP_MOUNT_SILVER_CAVE_SUMMIT (map_headers.h:13966 scriptsBank scr_seq_0107_D41R0108)
- files/fielddata/eventdata/zone_event/034_R33.json:20 - the only trainer on Route 33 is TRAINER_HIKER_ANTHONY (61); rematches 100/155/523 reuse the same NPC
- files/fielddata/eventdata/zone_event/085_T29.json:120,137,154,171 - Lake of Rage trainers Andre(126), Raymond(127), Alton(109), Lois(116) are on MAP_LAKE_OF_RAGE
- No other map with weather has a trainer: Rock Tunnel/Whirl Islands/Dark Cave/Cerulean Cave (weather 11 flash), Battle Arcade (13) and Sinjoh Ruins Exterior (5) have no std_trainer events in their zone_event files

### Black, White

- Location **Desert Resort**: Sandstorm
- Location **Route 4**: Sandstorm

Evidence:

- https://bulbapedia.bulbagarden.net/wiki/Sandstorm_(weather_condition) (seen only via search-engine summary; direct fetch blocked by Cloudflare) — overworld sandstorm table: Route 4 'Always' (B/W/W2), Desert Resort 'Always'; in-battle weather during overworld Sandstorm is Sandstorm
- https://bulbapedia.bulbagarden.net/wiki/Desert_Resort (search summary) — a sandstorm is always occurring in both parts of the area
- https://pokemondb.net/pokebase/299707/what-is-the-weather-schedule-for-every-area-in-every-game — Gen V (cites Bulbapedia Weather): Sandstorm = Route 4 and Desert Resort, all the time; hail/rain are only on specific winter/spring/summer/autumn days (<50%, excluded)
- https://pokemondb.net/pokebase/104995/how-is-the-random-weather-determined-in-gen-v and https://www.serebii.net/games/weather.shtml — Route 4 and Desert Resort constantly sandstorm; hail only in winter on Routes 6/7/8, Moor of Icirrus, Icirrus City, Twist Mountain
- https://www.thonky.com/pokemon-black-white/route-4 — sandstorm damages every turn on Route 4, no part of the route stated to be free of it; Cheren's Route 4 battle is in it
- Relic Castle is an interior (no weather), left out. Rain from Tornadus/Thundurus roaming storms is excluded (a single route at a time, far below 50%). Giant Chasm fog is not a battle weather.

### Black 2, White 2

- Location **Desert Resort**: Sandstorm

Evidence:

- https://bulbapedia.bulbagarden.net/wiki/Sandstorm_(weather_condition) (search summary) — Desert Resort 'Always'
- https://www.psypokes.com/bw2/walkthrough.php?part=4 — B2W2: 'the Sandstorm finally starts up' at Desert Resort; Black 2 Route 4 sandstorm gone (construction finished), White 2 Route 4 still has it
- Relic Castle / Relic Passage are interiors (no weather); Route 4 handled in the White 2 group; hail/rain in Routes 6/7/8, Twist Mountain, Icirrus, Driftveil, Mistralton, Virbank, Floccesy only on some winter/rain days (<50%) so excluded

### White 2

- Sandstorm: Backpacker Clifford, Backpacker Rachel, Backpacker Roland, Fisherman Andrew, Fisherman Hubert, Lady Lina, Pokemon Breeder Irene, Pokemon Trainer Colress (358), Pokéfan Norbert, Pokéfan Sachiko, Rich Boy Keenan, Twins Tara & Val

Evidence:

- https://www.psypokes.com/bw2/walkthrough.php?part=4 — White 2 Route 4 is 'a desert path that leads you through a raging sandstorm' ('a Sandstorm will blow through the entire battle'); Black 2 Route 4 sandstorm is gone
- https://bulbapedia.bulbagarden.net/wiki/Sandstorm_(weather_condition) (search summary) — Route 4 'Always', tagged for B, W and W2 only (not B2)
- Trainer version split on Route 4 from https://www.arrpeegeez.com/2022/07/pokemon-black-2-and-white-2-walkthrough_23.html and https://www.gamerguides.com/pokemon-black-white-2/guide/main-walkthrough/to-gym-4 (Black 2 only: Braven, Dell, Neagle, Joey, Silvester, Sinclair, Stanley, Rob & Sal; these are NOT listed). Clifford is White 2 per arrpeegeez/gamerguides but Black 2 per Serebii (uncertain).

## Judgement calls

- **HG/SS Red:** the summit has diamond dust (no weather) on 8 calendar days a
  year, and hail on every other day.
- **HG/SS Lake of Rage:** clear on Wednesdays once the red Gyarados has been
  met, so rain 6 days in 7.
- **Gen 3 Route 119:** the weather cycle is rain on 3 of its 4 stages. The
  Weather Institute grunts and admin fight indoors and are excluded.
- **Gen 3 Route 120:** the weather depends on the way the player walked in.
  Only the trainers you reach after crossing the rain triggers from the
  Fortree side are listed. Keigo, Clarissa, Robert and Jeffrey/Brandon are
  left out.
- **Gen 3 Route 111:** only the desert trainers are listed. Travis and Irene,
  in the west corridor, are left out.
- **Gen 4 fog** (Route 210 North, the Victory Road back room, Platinum's Lost
  Tower 3F/4F) assumes Defog wasn't used on that map. Rematch trainers take
  their base trainer's map.
- **White 2 Route 4:** keeps its sandstorm, while Black 2's Route 4 is built
  up. The B2/W2 data shares one Route 4, so White 2 lists its trainers by
  name. Sources disagree on whether Backpacker Clifford appears in Black 2 or
  White 2.
- **Gen 5 sources:** Bulbapedia blocked automated reads. The gen 5 entries rest
  on psypokes, Serebii, Thonky, pokemondb and Bulbapedia as quoted elsewhere.
