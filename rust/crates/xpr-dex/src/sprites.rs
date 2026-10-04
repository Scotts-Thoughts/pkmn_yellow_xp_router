//! HOME sprites (Solodex `utils/sprites.ts`, `data/formSprites.ts`): base
//! forms by national dex number, alternate forms by PokeAPI id.

use std::borrow::Cow;

use crate::store::store;

/// The PokeAPI id of a species' sprite.
pub fn sprite_id(species: &str, national_dex_number: i32) -> u32 {
    store().form_sprites().get(species).copied().unwrap_or(national_dex_number.max(0) as u32)
}

/// The 128 px WebP of a species.
pub fn sprite_bytes(species: &str, national_dex_number: i32) -> Option<Cow<'static, [u8]>> {
    crate::source::sprite(sprite_id(species, national_dex_number))
}
