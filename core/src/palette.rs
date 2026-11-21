//! The colour schemes every app offers: Bento (the default), Sorbet, Sea glass, Lavender and Clay,
//! each in light and dark. Hex RGB values; each app turns them into its own colour type.

pub struct Raw {
    /// Light: background, ink, muted. Dark: background, ink, sheet, raised.
    pub base: &'static [u32],
    /// Five category tiles as (tile, ink on tile) pairs: gift, plan, note, extra, extra.
    pub tiles: [u32; 10],
    pub strips: [u32; 5],
    /// Placeholder tones for people without a photo.
    pub tones: [u32; 8],
}

pub struct Scheme {
    pub name: &'static str,
    pub light: Raw,
    pub dark: Raw,
}

pub fn scheme(name: &str) -> &'static Scheme {
    SCHEMES.iter().find(|s| s.name == name).unwrap_or(&SCHEMES[0])
}

pub const SCHEMES: &[Scheme] = &[
    Scheme {
        name: "Bento",
        light: Raw { base: &[0xF4F3F8, 0x1C1B2E, 0x5E5C74], tiles: [0xFFDCCB, 0x8A3B17, 0xCFF0DE, 0x1E6B43, 0xFFF0C2, 0x7A5A00, 0xE3DEFF, 0x4B3FD1, 0xD5E9FF, 0x1D5A9E], strips: [0xFFB899, 0x8FDDB4, 0xFFDC6B, 0xBDB3FF, 0x9CC8F5], tones: [0xE8B8A2, 0xA9CDB9, 0xBDB6E6, 0xE6CF95, 0xA9C6E4, 0xE4AFC4, 0xC9D29A, 0xB9C3D6] },
        dark: Raw { base: &[0x15141E, 0xF4F3F8, 0x22212E, 0x2E2C40], tiles: [0x4A3029, 0xFFC9B3, 0x1F3B2F, 0xA8E8C6, 0x3D3520, 0xF5DC8A, 0x2E2A55, 0xC9C1FF, 0x1F3048, 0xAED3FF], strips: [0xB06A4C, 0x4E9C74, 0xB8963A, 0x7D70D6, 0x5D8FC7], tones: [0x7A5446, 0x4D6B5C, 0x5B5480, 0x7A6A44, 0x4C6680, 0x7A4D5E, 0x65703F, 0x56607A] },
    },
    Scheme {
        name: "Sorbet",
        light: Raw { base: &[0xFBF4F1, 0x2B1D24, 0x6E5A62], tiles: [0xFFD5DF, 0x8C2547, 0xDCF1CC, 0x2F6420, 0xFFE3C4, 0x7E4A12, 0xE9DDFB, 0x5B3A9E, 0xD3ECF2, 0x1E5E6E], strips: [0xF59AB2, 0x9AD47C, 0xF8B878, 0xB79BEA, 0x86C3D2], tones: [0xE9AFBD, 0xB5D49E, 0xF2C29A, 0xC9B3E6, 0xA7CFD8, 0xE3B0A4, 0xD6C78F, 0xC4B6BE] },
        dark: Raw { base: &[0x1C1418, 0xFAF1F3, 0x2A2025, 0x332630], tiles: [0x4E2532, 0xFFC2D1, 0x263A20, 0xBFE6A6, 0x4A3420, 0xFFD3A6, 0x3A2B52, 0xD9C8FF, 0x1E3940, 0xA9DCE8], strips: [0xC0627E, 0x6EA552, 0xC08A4E, 0x8C6CC4, 0x4E95A6], tones: [0x7E4A58, 0x566E48, 0x80604A, 0x66557E, 0x4E6E76, 0x7E5650, 0x6E6640, 0x645860] },
    },
    Scheme {
        name: "Sea glass",
        light: Raw { base: &[0xEEF5F4, 0x122E2C, 0x4B6664], tiles: [0xC9ECE6, 0x14594F, 0xD5E6FF, 0x1D4F8F, 0xF4E6C6, 0x6E5214, 0xE2DCF7, 0x4A3E8C, 0xF6D9D2, 0x8A3B2E], strips: [0x6CC9BA, 0x8CB8F2, 0xE4C278, 0xA99BE0, 0xE8A496], tones: [0x9FCFC6, 0xA6C3E8, 0xDCC79A, 0xB7C9B0, 0xC0B5DE, 0x93B9C9, 0xD6B3A6, 0xAFC2C0] },
        dark: Raw { base: &[0x0F1A1A, 0xECF6F5, 0x1B2626, 0x22302F], tiles: [0x1E403B, 0xA8E6DC, 0x1E2F4A, 0xB5D0FF, 0x3D3522, 0xF0D898, 0x2C2848, 0xCFC6FF, 0x452A26, 0xFFC2B6], strips: [0x3E9E8E, 0x5A86C8, 0xB09348, 0x7A6CC0, 0xC07264], tones: [0x3F6E66, 0x4A6488, 0x7A6A44, 0x56704C, 0x5E5682, 0x3E6476, 0x7A5A50, 0x4E6664] },
    },
    Scheme {
        name: "Lavender",
        light: Raw { base: &[0xF4F2FA, 0x221B3A, 0x5F5878], tiles: [0xE2D8FF, 0x4A35A8, 0xD2F0E2, 0x1E6B48, 0xFFDCE5, 0x8C2547, 0xFFEBC4, 0x7A5200, 0xD3E8FA, 0x1D5A8A], strips: [0xA48EF0, 0x7ED3AC, 0xF59AB2, 0xF2C46A, 0x8EC0EC], tones: [0xC3B5EE, 0xA9D8C2, 0xEFB6C6, 0xE6D19C, 0xAFC8E6, 0xD3B3E0, 0xC4CF9E, 0xB9B6CC] },
        dark: Raw { base: &[0x16131F, 0xF3F0FB, 0x231F30, 0x2A2540], tiles: [0x34295C, 0xD2C6FF, 0x1E3A2E, 0xA9E5C8, 0x4A2633, 0xFFC2D1, 0x453818, 0xFFDC8F, 0x1E3248, 0xAED3FF], strips: [0x7A62D0, 0x4E9C78, 0xC0627E, 0xC09A3E, 0x5D8FC7], tones: [0x5E4C93, 0x4A7060, 0x7E4E60, 0x7A6A44, 0x4C6680, 0x6E4E80, 0x65703F, 0x57536E] },
    },
    Scheme {
        name: "Clay",
        light: Raw { base: &[0xF5F1EC, 0x2A211C, 0x6A5D54], tiles: [0xF3CFBE, 0x8A3A1E, 0xDCE4C6, 0x4A5A22, 0xEFE3CC, 0x6E5420, 0xE6DCE8, 0x5E3E6E, 0xD6E2E4, 0x2E5560], strips: [0xE09A7C, 0xAFC07E, 0xD8BC84, 0xB79AC2, 0x8EB2BA], tones: [0xD9A890, 0xBFC9A0, 0xDCC7A0, 0xC7B0A0, 0xB5C2C2, 0xD0A8A0, 0xB9B08C, 0xC2B6AE] },
        dark: Raw { base: &[0x1A1613, 0xF6F0EA, 0x27211D, 0x2E2722], tiles: [0x4E2C1E, 0xF6C3AC, 0x2E3520, 0xCFDDA6, 0x3F3524, 0xEFD8A8, 0x3A2A40, 0xE6C8F0, 0x203236, 0xB4D8DE], strips: [0xB8664A, 0x7A8E48, 0xA88A50, 0x8A6A96, 0x5A8890], tones: [0x7A4E3B, 0x5E6844, 0x7A6644, 0x6E584A, 0x4E6466, 0x744E48, 0x66603E, 0x5E544C] },
    },
];

/// Look A4 (1.0): a person's card is tinted in their tone. Returns (card, ink): the card's
/// background and the colour for its secondary line, which always reaches 4.5:1 contrast
/// against the card. `background` is the screen behind the card (the scheme's base[0]).
pub fn person_card(tone: u32, background: u32, dark: bool) -> (u32, u32) {
    let card = if dark { mix(tone, background, 0.45) } else { mix(tone, 0xFFFFFF, 0.5) };
    let (h, s, l) = hsl(tone);
    let s = s.max(0.35).min(0.75);
    let mut l = if dark { l.max(0.70) } else { l.min(0.40) };
    let mut ink = from_hsl(h, s, l);
    while contrast(ink, card) < 4.5 && l > 0.0 && l < 1.0 {
        l += if dark { 0.01 } else { -0.01 };
        ink = from_hsl(h, s, l.clamp(0.0, 1.0));
    }
    (card, ink)
}

fn channels(c: u32) -> [f64; 3] { [(c >> 16 & 0xFF) as f64 / 255.0, (c >> 8 & 0xFF) as f64 / 255.0, (c & 0xFF) as f64 / 255.0] }
fn pack(v: [f64; 3]) -> u32 {
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    b(v[0]) << 16 | b(v[1]) << 8 | b(v[2])
}
/// `amount` of the way from `a` to `b`.
fn mix(a: u32, b: u32, amount: f64) -> u32 {
    let (a, b) = (channels(a), channels(b));
    pack(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * amount))
}
/// WCAG contrast ratio between two colours.
pub fn contrast(a: u32, b: u32) -> f64 {
    let lum = |c: u32| {
        let v = channels(c).map(|x| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) });
        0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2]
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}
fn hsl(c: u32) -> (f64, f64, f64) {
    let [r, g, b] = channels(c);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min { return (0.0, 0.0, l); }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h / 6.0, s, l)
}
fn from_hsl(h: f64, s: f64, l: f64) -> u32 {
    if s == 0.0 { return pack([l, l, l]); }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 { t += 1.0 } if t > 1.0 { t -= 1.0 }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t } else if t < 0.5 { q } else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 } else { p }
    };
    pack([f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tinted_card_is_readable() {
        for s in SCHEMES {
            for (raw, dark) in [(&s.light, false), (&s.dark, true)] {
                for &t in &raw.tones {
                    let (card, ink) = person_card(t, raw.base[0], dark);
                    assert!(contrast(ink, card) >= 4.5, "{} {dark} {t:06X}: secondary line", s.name);
                    assert!(contrast(raw.base[1], card) >= 7.0, "{} {dark} {t:06X}: name", s.name);
                }
            }
        }
    }

    #[test]
    fn android_person_cards_match() {
        let kt = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../app/src/main/java/app/memi/ui/theme/PersonCards.kt"))
            .unwrap().to_uppercase();
        for s in SCHEMES {
            for (raw, dark) in [(&s.light, false), (&s.dark, true)] {
                let v: Vec<String> = raw.tones.iter().map(|&t| { let (c, i) = person_card(t, raw.base[0], dark); format!("0XFF{c:06X}, 0XFF{i:06X}") }).collect();
                let line = format!("\"{} {}\" TO LISTOF({}),", s.name, if dark { "dark" } else { "light" }, v.join(", ")).to_uppercase();
                assert!(kt.contains(&line), "PersonCards.kt is out of date for {} (dark: {dark}); expected {line}", s.name);
            }
        }
    }
}
