#!/usr/bin/env node
/**
 * Regenerate `dex_data/` (the Pokédex reference data behind the Dex page)
 * from Solodex's `data_objects-main/` and its bundled HOME sprites.
 *
 *   node tools/dex_data/sync.mjs [<solodex repo>]
 *
 * The default source is A:/Dropbox/stp-projects/programs/solodex. The data
 * modules are plain `export const NAME = {...}` files; this script evaluates
 * them the way Solodex's build plugin does (JSON fast path, sandboxed vm for
 * the unquoted-key ones) and applies, once, every transform Solodex applied
 * when a game loaded:
 *
 *   - the gen 1-4 games are sliced out of the shared pokedex.js;
 *   - species names are normalised (SPECIES_ALIASES, incl. evolution families);
 *   - Red/Blue and Yellow take `transfer_learnset` from their per-game files;
 *   - learnsets are respelled to the game's own move names (Faint Attack,
 *     SolarBeam, Hi Jump Kick ... through gen 5; Vice Grip in gens 6-7).
 *
 * It also writes the cross-game species index (Solodex's build-time
 * `buildSpeciesIndex`) and the form -> sprite id table from formSprites.ts.
 * Output JSON is compact and keeps the source key order (the rankings break
 * ties by it).
 */
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { fileURLToPath } from 'node:url'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const SOLODEX = path.resolve(process.argv[2] ?? 'A:/Dropbox/stp-projects/programs/solodex')
const DATA = path.join(SOLODEX, 'data_objects-main')
const OUT = path.join(ROOT, 'dex_data')

// ---- tables copied from Solodex (src/renderer/src/data/games.ts, speciesIndex.ts, index.ts) ----

const GAMES = [
  'Red and Blue', 'Yellow', 'Gold and Silver', 'Crystal', 'Ruby and Sapphire', 'Emerald',
  'FireRed and LeafGreen', 'Diamond and Pearl', 'Platinum', 'HeartGold and SoulSilver', 'Black',
  'Black 2 and White 2', 'X and Y', 'Omega Ruby and Alpha Sapphire', 'Sun and Moon',
  'Ultra Sun and Ultra Moon', 'Sword and Shield', 'Brilliant Diamond and Shining Pearl',
  'Legends Arceus', 'Scarlet and Violet', 'Legends Z-A',
]

const GAME_TO_GEN = {
  'Red and Blue': 1, 'Yellow': 1, 'Gold and Silver': 2, 'Crystal': 2, 'Ruby and Sapphire': 3,
  'Emerald': 3, 'FireRed and LeafGreen': 3, 'Diamond and Pearl': 4, 'Platinum': 4,
  'HeartGold and SoulSilver': 4, 'Black': 5, 'Black 2 and White 2': 5, 'X and Y': 6,
  'Omega Ruby and Alpha Sapphire': 6, 'Sun and Moon': 7, 'Ultra Sun and Ultra Moon': 7,
  'Sword and Shield': 8, 'Brilliant Diamond and Shining Pearl': 8, 'Legends Arceus': 8,
  'Scarlet and Violet': 9, 'Legends Z-A': 9,
}

const POKEDEX_SOURCES = {
  'Red and Blue': { file: 'pokedex.js', key: 'Red and Blue', transferFile: 'pokedex/red_blue.js' },
  'Yellow': { file: 'pokedex.js', key: 'Yellow', transferFile: 'pokedex/yellow.js' },
  'Gold and Silver': { file: 'pokedex.js', key: 'Gold and Silver' },
  'Crystal': { file: 'pokedex.js', key: 'Crystal' },
  'Ruby and Sapphire': { file: 'pokedex.js', key: 'Ruby and Sapphire' },
  'Emerald': { file: 'pokedex.js', key: 'Emerald' },
  'FireRed and LeafGreen': { file: 'pokedex.js', key: 'FireRed and LeafGreen' },
  'Diamond and Pearl': { file: 'pokedex.js', key: 'Diamond and Pearl' },
  'Platinum': { file: 'pokedex.js', key: 'Platinum' },
  'HeartGold and SoulSilver': { file: 'pokedex.js', key: 'HeartGold and SoulSilver' },
  'Black': { file: 'pokedex/black_white.js' },
  'Black 2 and White 2': { file: 'pokedex/black2_white2.js' },
  'X and Y': { file: 'pokedex/x_y.js' },
  'Omega Ruby and Alpha Sapphire': { file: 'pokedex/omega_ruby_alpha_sapphire.js' },
  'Sun and Moon': { file: 'pokedex/sun_moon.js' },
  'Ultra Sun and Ultra Moon': { file: 'pokedex/ultra_sun_ultra_moon.js' },
  'Sword and Shield': { file: 'pokedex/sword_shield.js' },
  'Brilliant Diamond and Shining Pearl': { file: 'pokedex/brilliant_diamond_shining_pearl.js' },
  'Legends Arceus': { file: 'pokedex/legends_arceus.js' },
  'Scarlet and Violet': { file: 'pokedex/scarlet_violet.js' },
  'Legends Z-A': { file: 'pokedex/legends_za.js' },
}

const ENCOUNTER_SOURCES = {
  'Red and Blue': 'red_blue', 'Yellow': 'yellow', 'Gold and Silver': 'gold_silver', 'Crystal': 'crystal',
  'Ruby and Sapphire': 'ruby_sapphire', 'Emerald': 'emerald', 'FireRed and LeafGreen': 'firered_leafgreen',
  'Diamond and Pearl': 'diamond_pearl', 'Platinum': 'platinum', 'HeartGold and SoulSilver': 'heartgold_soulsilver',
  'Black': 'black_white', 'Black 2 and White 2': 'black2_white2', 'X and Y': 'x_y',
  'Omega Ruby and Alpha Sapphire': 'omega_ruby_alpha_sapphire', 'Sun and Moon': 'sun_moon',
  'Ultra Sun and Ultra Moon': 'ultra_sun_ultra_moon',
}

const SPECIES_ALIASES = {
  'Nidoran♀': 'Nidoran_F',
  'Nidoran♂': 'Nidoran_M',
  'Farfetch’d': "Farfetch'd",
  'Galarian Farfetch’d': "Galarian Farfetch'd",
  'Sirfetch’d': "Sirfetch'd",
  'Wormadam (Plant Cloak)': 'Wormadam',
  'Wormadam (Sandy Cloak)': 'Wormadam (Sandy)',
  'Wormadam (Trash Cloak)': 'Wormadam (Trash)',
  'Giratina': 'Giratina (Altered)',
  'Shaymin': 'Shaymin (Land)',
  'Deoxys': 'Deoxys (Normal)',
  'Meloetta': 'Meloetta (Aria)',
}

const MOVE_RENAMES = [
  ['Ancient Power', 'AncientPower', 5], ['Bubble Beam', 'BubbleBeam', 5], ['Conversion 2', 'Conversion2', 2],
  ['Double Slap', 'DoubleSlap', 5], ['Dragon Breath', 'DragonBreath', 5], ['Dynamic Punch', 'DynamicPunch', 5],
  ['Extreme Speed', 'ExtremeSpeed', 5], ['Feather Dance', 'FeatherDance', 5], ['Feint Attack', 'Faint Attack', 5],
  ['Grass Whistle', 'GrassWhistle', 5], ['High Jump Kick', 'Hi Jump Kick', 5], ['Poison Powder', 'PoisonPowder', 5],
  ['Sand Attack', 'Sand-Attack', 5], ['Self-Destruct', 'Selfdestruct', 5], ['Smelling Salts', 'SmellingSalt', 5],
  ['Smokescreen', 'SmokeScreen', 5], ['Soft-Boiled', 'Softboiled', 5], ['Solar Beam', 'SolarBeam', 5],
  ['Sonic Boom', 'SonicBoom', 5], ['Thunder Punch', 'ThunderPunch', 5], ['Thunder Shock', 'ThunderShock', 5],
  ['Vise Grip', 'ViceGrip', 5], ['Vise Grip', 'Vice Grip', 7],
]

const LEARNSET_LIST_FIELDS = [
  'tm_hm_learnset', 'tutor_learnset', 'egg_moves', 'transfer_learnset', 'prior_evolution_learnset',
  'form_change_learnset', 'zygarde_cube_learnset', 'light_ball_egg_learnset',
]

// ---- helpers ------------------------------------------------------------------------------------

function readDataModule(file) {
  const source = fs.readFileSync(file, 'utf8')
  const header = /^\s*export const (\w+)\s*=\s*/.exec(source)
  if (!header) throw new Error(`${file}: expected a single "export const NAME = {...}" module`)
  const body = source.slice(header[0].length).replace(/;?\s*$/, '')
  try {
    return JSON.parse(body)
  } catch {
    return vm.runInNewContext(`(${body})`, Object.create(null), { filename: file })
  }
}

const slug = game => game.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '')

function writeJson(rel, value) {
  const file = path.join(OUT, rel)
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, JSON.stringify(value))
  return fs.statSync(file).size
}

function normalizePokedex(raw) {
  const out = {}
  for (const [name, data] of Object.entries(raw)) {
    const canonical = SPECIES_ALIASES[name] ?? name
    const family = data.evolution_family?.map(evo => {
      const c = SPECIES_ALIASES[evo.species] ?? evo.species
      return c !== evo.species ? { ...evo, species: c } : evo
    })
    out[canonical] = { ...data, species: canonical, evolution_family: family ?? data.evolution_family }
  }
  return out
}

function mergeTransferLearnsets(dex, transfer) {
  const out = { ...dex }
  for (const [name, entry] of Object.entries(transfer)) {
    const canonical = SPECIES_ALIASES[name] ?? name
    if (out[canonical] && entry.transfer_learnset) out[canonical] = { ...out[canonical], transfer_learnset: entry.transfer_learnset }
  }
  return out
}

const MOVE_NAME_ALIASES = {}
for (const [modern, legacy] of MOVE_RENAMES) {
  MOVE_NAME_ALIASES[modern] ??= legacy
  MOVE_NAME_ALIASES[legacy] = modern
}
const SPELLINGS_BY_GEN = {}
for (const [modern, legacy, lastGen] of MOVE_RENAMES) (SPELLINGS_BY_GEN[modern] ??= []).push([lastGen, legacy])
for (const list of Object.values(SPELLINGS_BY_GEN)) list.sort((a, b) => a[0] - b[0])

function moveNameForGen(name, gen) {
  const modern = SPELLINGS_BY_GEN[name] ? name : MOVE_NAME_ALIASES[name]
  const spellings = modern ? SPELLINGS_BY_GEN[modern] : undefined
  if (!spellings) return name
  return spellings.find(([lastGen]) => gen <= lastGen)?.[1] ?? modern
}

function respellLearnsets(dex, gen) {
  const respell = m => moveNameForGen(m, gen)
  const out = {}
  for (const [name, data] of Object.entries(dex)) {
    const entry = { ...data }
    if (data.level_up_learnset) entry.level_up_learnset = data.level_up_learnset.map(([lv, m]) => [lv, respell(m)])
    for (const field of LEARNSET_LIST_FIELDS) if (Array.isArray(data[field])) entry[field] = data[field].map(respell)
    out[name] = entry
  }
  return out
}

// forms.ts classifyForm, reduced to the mega check the index needs
function isMegaForm(name) {
  let rest = name
  const suffix = rest.match(/^(.+?) \((.+)\)$/)
  if (suffix) {
    if (suffix[2] === 'Mega Z') return true
    rest = suffix[1]
  }
  const regional = rest.match(/^(Alolan|Galarian|Hisuian|Paldean) (.+)$/)
  if (regional) rest = regional[2]
  return /^(Mega|Primal) (.+?)( X| Y| Z)?$/.test(rest)
}

function evolutionStage(name, family, evolvedFromSet) {
  if (isMegaForm(name)) return 'mega'
  if (!family || family.length <= 1) return 'single'
  const evolvesInto = family.some(e => e.species !== name && e.method !== null)
  const evolvedFrom = evolvedFromSet.has(name)
  if (evolvesInto && !evolvedFrom) return 'first'
  if (evolvesInto && evolvedFrom) return 'middle'
  if (!evolvesInto && evolvedFrom) return 'final'
  return 'single'
}

/** speciesIndex.ts buildSpeciesIndex over the normalised tables. */
function buildSpeciesIndex(normalized) {
  const evolvedFromSet = new Set()
  for (const [, dex] of normalized) {
    for (const data of Object.values(dex)) {
      for (const evo of data.evolution_family ?? []) if (evo.method !== null && evo.species !== data.species) evolvedFromSet.add(evo.species)
    }
  }
  const seen = new Map()
  for (const [game, dex] of normalized) {
    for (const [name, data] of Object.entries(dex)) {
      const entry = seen.get(name)
      if (entry) { entry.games.push(game); continue }
      seen.set(name, {
        name,
        national_dex_number: data.national_dex_number,
        type_1: data.type_1,
        type_2: data.type_2,
        growth_rate: data.growth_rate,
        evolution_stage: evolutionStage(name, data.evolution_family, evolvedFromSet),
        games: [game],
      })
    }
  }
  // Array.prototype.sort is stable, like Rust's sort_by
  return Array.from(seen.values()).sort((a, b) => a.national_dex_number - b.national_dex_number)
}

// ---- run -----------------------------------------------------------------------------------------

if (!fs.existsSync(DATA)) {
  console.error(`No data_objects-main under ${SOLODEX}`)
  process.exit(1)
}
fs.rmSync(OUT, { recursive: true, force: true })
fs.mkdirSync(OUT, { recursive: true })

const sizes = {}
const manifest = { source: SOLODEX.replace(/\\/g, '/'), generated: new Date().toISOString(), games: [] }
const shared = new Map()
const normalizedForIndex = []
for (const game of GAMES) {
  const src = POKEDEX_SOURCES[game]
  const file = path.join(DATA, src.file)
  if (!shared.has(file)) shared.set(file, readDataModule(file))
  const module = shared.get(file)
  const table = src.key ? module[src.key] : module
  if (!table) throw new Error(`${src.file} has no entry for "${game}"`)
  let dex = normalizePokedex(table)
  normalizedForIndex.push([game, dex])
  if (src.transferFile) dex = mergeTransferLearnsets(dex, readDataModule(path.join(DATA, src.transferFile)))
  dex = respellLearnsets(dex, GAME_TO_GEN[game])
  const rel = `pokedex/${slug(game)}.json`
  sizes[rel] = writeJson(rel, dex)
  const enc = ENCOUNTER_SOURCES[game]
  let encRel = null
  if (enc) {
    encRel = `encounters/${slug(game)}.json`
    sizes[encRel] = writeJson(encRel, readDataModule(path.join(DATA, 'encounters', `${enc}_by_pokemon.js`)))
  }
  manifest.games.push({ name: game, gen: GAME_TO_GEN[game], pokedex: rel, encounters: encRel, species: Object.keys(dex).length })
}

sizes['species_index.json'] = writeJson('species_index.json', buildSpeciesIndex(normalizedForIndex))
for (const name of ['moves', 'effectiveness', 'tmhm', 'natures', 'unobtainable_moves']) {
  sizes[`${name}.json`] = writeJson(`${name}.json`, readDataModule(path.join(DATA, `${name}.js`)))
}

// formSprites.ts: `'Name': id,` / `"Name": id,` entries of FORM_SPRITE_IDS
const formSrc = fs.readFileSync(path.join(SOLODEX, 'src/renderer/src/data/formSprites.ts'), 'utf8')
const body = formSrc.slice(formSrc.indexOf('FORM_SPRITE_IDS'))
const formSprites = {}
for (const m of body.matchAll(/^\s*(?:'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)")\s*:\s*(\d+)\s*,/gm)) {
  formSprites[(m[1] ?? m[2]).replace(/\\(.)/g, '$1')] = Number(m[3])
}
sizes['form_sprites.json'] = writeJson('form_sprites.json', formSprites)

// HOME sprites (128 px WebP)
const spriteSrc = path.join(SOLODEX, 'src/renderer/public/sprites/home')
const spriteDst = path.join(OUT, 'sprites/home')
fs.mkdirSync(spriteDst, { recursive: true })
let spriteBytes = 0
let spriteCount = 0
for (const f of fs.readdirSync(spriteSrc)) {
  if (!f.endsWith('.webp')) continue
  fs.copyFileSync(path.join(spriteSrc, f), path.join(spriteDst, f))
  spriteBytes += fs.statSync(path.join(spriteDst, f)).size
  spriteCount++
}
manifest.sprites = spriteCount
writeJson('manifest.json', manifest)

const total = Object.values(sizes).reduce((a, b) => a + b, 0)
console.log(`dex_data: ${Object.keys(sizes).length} JSON files, ${(total / 1e6).toFixed(1)} MB; ${spriteCount} sprites, ${(spriteBytes / 1e6).toFixed(1)} MB; ${Object.keys(formSprites).length} form sprite ids`)
