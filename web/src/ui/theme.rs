//! The colour scheme as CSS variables on <html>, from the same palettes as the apps
//! (memi_engine::core::palette), so style.css only ever says var(--tile0) and so on.

use memi_engine::core::palette::scheme;

fn hex(c: u32) -> String { format!("#{c:06x}") }
fn alpha(c: u32, a: f32) -> String { format!("rgba({}, {}, {}, {a})", c >> 16 & 0xff, c >> 8 & 0xff, c & 0xff) }

pub fn css_vars(name: &str, dark: bool) -> String {
    let s = scheme(name);
    let mut v: Vec<(String, String)> = vec![];
    let r = if dark { &s.dark } else { &s.light };
    let ink = r.base[1];
    if !dark {
        v.extend([
            ("bg", hex(r.base[0])), ("ink", hex(ink)), ("muted", hex(r.base[2])), ("card", "#ffffff".into()),
            ("line", alpha(ink, 0.08)), ("sheet", "#ffffff".into()), ("track", hex(r.base[0])),
            ("danger", "#b42318".into()), ("scrim", "rgba(0,0,0,0.32)".into()), ("hover", alpha(ink, 0.05)),
        ].map(|(a, b)| (a.to_string(), b)));
    } else {
        v.extend([
            ("bg", hex(r.base[0])), ("ink", hex(ink)), ("muted", alpha(ink, 0.66)), ("card", "rgba(255,255,255,0.07)".into()),
            ("line", "rgba(255,255,255,0.08)".into()), ("sheet", hex(r.base[2])), ("track", "rgba(255,255,255,0.06)".into()),
            ("danger", "#ff8a80".into()), ("scrim", "rgba(0,0,0,0.5)".into()), ("hover", "rgba(255,255,255,0.06)".into()),
        ].map(|(a, b)| (a.to_string(), b)));
    }
    for i in 0..5 {
        v.push((format!("tile{i}"), hex(r.tiles[i * 2])));
        v.push((format!("tileink{i}"), hex(r.tiles[i * 2 + 1])));
        v.push((format!("strip{i}"), hex(r.strips[i])));
    }
    for (i, t) in r.tones.iter().enumerate() { v.push((format!("tone{i}"), hex(*t))); }
    v.push(("scheme".into(), if dark { "dark".into() } else { "light".into() }));
    v.iter().map(|(k, val)| if k == "scheme" { format!("color-scheme:{val};") } else { format!("--{k}:{val};") }).collect()
}

/// A swatch preview of a scheme, for the theme panel: (background, big tile, two small tiles).
pub fn preview(name: &str, dark: bool) -> (String, String, String, String) {
    let s = scheme(name);
    let r = if dark { &s.dark } else { &s.light };
    (hex(r.base[0]), hex(r.tiles[0]), hex(r.tiles[2]), hex(r.tiles[4]))
}
