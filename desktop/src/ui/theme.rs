//! Colours and type, the same five schemes as the Android app (Palettes.kt), each in light and dark.

use gpui::{hsla, rgb, rgba, Hsla};

/// Raw hex values. Light: bg, ink, muted. Dark: bg, ink, sheet, dock.
/// Tiles: five (tile, ink) pairs for the category slots: gift, plan, note, extra A, extra B.
pub use memi_core::palette::SCHEMES;

#[derive(Clone)]
pub struct Colors {
    pub is_dark: bool,
    pub bg: Hsla,
    pub ink: Hsla,
    pub muted: Hsla,
    pub card: Hsla,
    pub line: Hsla,
    pub sheet: Hsla,
    pub track: Hsla,
    pub tiles: [Hsla; 5],
    pub tile_inks: [Hsla; 5],
    pub strips: [Hsla; 5],
    pub tones: [Hsla; 8],
    /// Look A4: (card, ink) for each tone; see memi_core::palette::person_card.
    pub person_cards: [(Hsla, Hsla); 8],
    pub hit: Hsla,
    pub hit_ink: Hsla,
    pub miss: Hsla,
    pub miss_ink: Hsla,
    pub danger: Hsla,
    pub scrim: Hsla,
}

fn c(hex: u32) -> Hsla { rgb(hex).into() }
fn with_alpha(h: Hsla, a: f32) -> Hsla { Hsla { a, ..h } }

impl Colors {
    pub fn gift(&self) -> Hsla { self.tiles[0] }
    pub fn gift_ink(&self) -> Hsla { self.tile_inks[0] }
    pub fn plan(&self) -> Hsla { self.tiles[1] }
    pub fn plan_ink(&self) -> Hsla { self.tile_inks[1] }
    pub fn tone(&self, i: i32) -> Hsla { self.tones[i.rem_euclid(8) as usize] }
    pub fn person_card(&self, tone: i32) -> Hsla { self.person_cards[tone.rem_euclid(8) as usize].0 }
    pub fn person_ink(&self, tone: i32) -> Hsla { self.person_cards[tone.rem_euclid(8) as usize].1 }
    pub fn tile(&self, slot: i32) -> Hsla { self.tiles[slot.rem_euclid(5) as usize] }
    pub fn tile_ink(&self, slot: i32) -> Hsla { self.tile_inks[slot.rem_euclid(5) as usize] }
}

fn person_cards(r: &memi_core::palette::Raw, dark: bool) -> [(Hsla, Hsla); 8] {
    r.tones.map(|t| { let (card, ink) = memi_core::palette::person_card(t, r.base[0], dark); (c(card), c(ink)) })
}

fn pairs(t: &[u32; 10]) -> ([Hsla; 5], [Hsla; 5]) {
    (std::array::from_fn(|i| c(t[i * 2])), std::array::from_fn(|i| c(t[i * 2 + 1])))
}

pub fn colors(scheme: &str, dark: bool) -> Colors {
    let s = memi_core::palette::scheme(scheme);
    let white: Hsla = rgba(0xFFFFFFFF).into();
    if !dark {
        let r = &s.light;
        let (tiles, tile_inks) = pairs(&r.tiles);
        let ink = c(r.base[1]);
        Colors {
            is_dark: false, bg: c(r.base[0]), ink, muted: c(r.base[2]),
            card: white, line: with_alpha(ink, 0.08), sheet: white, track: c(r.base[0]),
            tiles, tile_inks, strips: r.strips.map(c), tones: r.tones.map(c), person_cards: person_cards(r, false),
            hit: tiles[1], hit_ink: tile_inks[1], miss: with_alpha(ink, 0.08), miss_ink: c(r.base[2]),
            danger: c(0xB42318), scrim: hsla(0., 0., 0., 0.32),
        }
    } else {
        let r = &s.dark;
        let (tiles, tile_inks) = pairs(&r.tiles);
        let ink = c(r.base[1]);
        Colors {
            is_dark: true, bg: c(r.base[0]), ink, muted: with_alpha(ink, 0.66),
            card: with_alpha(white, 0.07), line: with_alpha(white, 0.08), sheet: c(r.base[2]), track: with_alpha(white, 0.06),
            tiles, tile_inks, strips: r.strips.map(c), tones: r.tones.map(c), person_cards: person_cards(r, true),
            hit: tiles[1], hit_ink: tile_inks[1], miss: with_alpha(white, 0.08), miss_ink: with_alpha(ink, 0.66),
            danger: c(0xFF8A80), scrim: hsla(0., 0., 0., 0.5),
        }
    }
}

pub const FONT: &str = "Plus Jakarta Sans";
