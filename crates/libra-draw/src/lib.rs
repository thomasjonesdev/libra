//! libra-draw — the display list: the universal currency between the
//! widget layer and executors (ARCHITECTURE.md §9.1).
//!
//! Phase 0 subset: Clear, Rect, RRect, Text. Interned resources
//! (PathId/GlyphRunId/ImageId), layers, clips, and cache actions arrive
//! with damage tracking in Phase 2.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Regular,
    Bold,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawCmd {
    Clear(Color),
    Rect { rect: Rect, color: Color },
    RRect { rect: Rect, radius: f32, color: Color },
    /// Text drawn with its layout box's top-left at (x, y).
    Text { x: f32, y: f32, content: String, px: f32, style: FontStyle, color: Color },
    /// Stroked polyline (charts, sparse data). Points are absolute coordinates.
    Polyline { points: Vec<(f32, f32)>, width: f32, color: Color },
    /// Dense-chart column strips (M4 rendering): for column `i`,
    /// `strips[i] = Some((y0, y1))` fills a `col_w`-wide vertical run at
    /// `x + i·col_w` from y0 to y1. Rasterized as opaque pixel runs —
    /// no AA path machinery — which is what makes 16 live charts cheap.
    Strips { x: f32, col_w: f32, strips: Vec<Option<(f32, f32)>>, color: Color },
}

impl DrawCmd {
    /// Conservative bounding box (damage tracking). `Clear` is
    /// whole-frame and returns None (callers treat it as full damage).
    /// All bounds over-approximate by ≥1px: anti-aliased edges spill
    /// outside geometric rects, and glyph rasters overhang the advance
    /// box.
    pub fn bounds(&self) -> Option<Rect> {
        const AA_PAD: f32 = 1.0;
        let pad = |r: &Rect| Rect::new(r.x - AA_PAD, r.y - AA_PAD, r.w + 2.0 * AA_PAD, r.h + 2.0 * AA_PAD);
        match self {
            DrawCmd::Clear(_) => None,
            DrawCmd::Rect { rect, .. } => Some(pad(rect)),
            DrawCmd::RRect { rect, .. } => Some(pad(rect)),
            DrawCmd::Text { x, y, content, px, .. } => {
                // Width bound: glyphs can't exceed ~1·px advance each.
                let w = content.chars().count() as f32 * px * 1.25 + px;
                Some(Rect::new(x - px, y - px, w + 2.0 * px, px * 2.5))
            }
            DrawCmd::Polyline { points, width, .. } => {
                if points.is_empty() {
                    return Some(Rect::new(0.0, 0.0, 0.0, 0.0));
                }
                let (mut x0, mut y0, mut x1, mut y1) =
                    (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
                for (x, y) in points {
                    x0 = x0.min(*x);
                    y0 = y0.min(*y);
                    x1 = x1.max(*x);
                    y1 = y1.max(*y);
                }
                // Miter joins can extend well past width/2 on sharp
                // angles; pad generously (bounds are cost, not
                // correctness-of-content — but must never under-cover).
                let pad = 4.0 * width + AA_PAD;
                Some(Rect::new(x0 - pad, y0 - pad, (x1 - x0) + 2.0 * pad, (y1 - y0) + 2.0 * pad))
            }
            DrawCmd::Strips { x, col_w, strips, .. } => {
                let (mut y0, mut y1) = (f32::INFINITY, f32::NEG_INFINITY);
                for s in strips.iter().flatten() {
                    y0 = y0.min(s.0);
                    y1 = y1.max(s.1);
                }
                if y0 > y1 {
                    return Some(Rect::new(*x, 0.0, 0.0, 0.0));
                }
                let w = strips.len() as f32 * col_w;
                Some(Rect::new(x - AA_PAD, y0 - AA_PAD, w + 2.0 * AA_PAD, (y1 - y0) + 2.0 * AA_PAD))
            }
        }
    }
}

impl Rect {
    /// Smallest rect covering both.
    pub fn union(&self, o: &Rect) -> Rect {
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = (self.x + self.w).max(o.x + o.w);
        let y1 = (self.y + self.h).max(o.y + o.h);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

/// Design tokens (ARCHITECTURE.md §11). Phase 0's hard-coded dark
/// values, kept verbatim as the `libra-dark` theme's table — Tier-4
/// snapshot identity depends on them.
pub mod tokens {
    use super::Color;

    pub const BG: Color = Color::rgb(0x0B, 0x0E, 0x14);
    pub const SURFACE: Color = Color::rgb(0x15, 0x1A, 0x23);
    pub const TEXT: Color = Color::rgb(0xE6, 0xEA, 0xF2);
    pub const TEXT_MUTED: Color = Color::rgb(0x8A, 0x93, 0xA6);
    pub const ACCENT: Color = Color::rgb(0x4C, 0x8D, 0xFF);
    pub const ACCENT_TEXT: Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const SECONDARY: Color = Color::rgb(0x23, 0x2B, 0x3A);

    pub const RADIUS_MD: f32 = 6.0;

    pub const TYPE_SM: f32 = 12.0;
    pub const TYPE_MD: f32 = 14.0;
    pub const TYPE_XL: f32 = 24.0;

    pub const BTN_PAD_X: f32 = 12.0;
    pub const BTN_PAD_Y: f32 = 6.0;
}

/// A resolved theme: the named-token table every widget styles through
/// (§11 — no hex codes in widgets or markup). Two ship built in:
/// `libra-dark` (mission-control default) and `libra-light`. Themes are
/// part of the determinism contract: a snapshot's identity includes the
/// theme it rendered under. Runtime switching is `Cmd::SetTheme`;
/// TOML theme tables + hot reload are the remaining §11 items.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub surface: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub secondary: Color,
    pub selection: Color,
    pub radius_md: f32,
    pub type_sm: f32,
    pub type_md: f32,
    pub type_xl: f32,
    pub btn_pad_x: f32,
    pub btn_pad_y: f32,
}

impl Theme {
    /// The mission-control default. Byte-identical to the Phase 0
    /// token values — existing snapshots are recorded under it.
    pub fn dark() -> Theme {
        Theme {
            name: "libra-dark".to_string(),
            bg: tokens::BG,
            surface: tokens::SURFACE,
            text: tokens::TEXT,
            text_muted: tokens::TEXT_MUTED,
            accent: tokens::ACCENT,
            accent_text: tokens::ACCENT_TEXT,
            secondary: tokens::SECONDARY,
            selection: Color::rgb(0x2A, 0x4A, 0x7A),
            radius_md: tokens::RADIUS_MD,
            type_sm: tokens::TYPE_SM,
            type_md: tokens::TYPE_MD,
            type_xl: tokens::TYPE_XL,
            btn_pad_x: tokens::BTN_PAD_X,
            btn_pad_y: tokens::BTN_PAD_Y,
        }
    }

    pub fn light() -> Theme {
        Theme {
            name: "libra-light".to_string(),
            bg: Color::rgb(0xF5, 0xF6, 0xF8),
            surface: Color::rgb(0xFF, 0xFF, 0xFF),
            text: Color::rgb(0x1A, 0x20, 0x2B),
            text_muted: Color::rgb(0x6B, 0x74, 0x84),
            accent: Color::rgb(0x2A, 0x66, 0xD9),
            accent_text: Color::rgb(0xFF, 0xFF, 0xFF),
            secondary: Color::rgb(0xE4, 0xE7, 0xEC),
            selection: Color::rgb(0xB8, 0xD0, 0xF5),
            radius_md: tokens::RADIUS_MD,
            type_sm: tokens::TYPE_SM,
            type_md: tokens::TYPE_MD,
            type_xl: tokens::TYPE_XL,
            btn_pad_x: tokens::BTN_PAD_X,
            btn_pad_y: tokens::BTN_PAD_Y,
        }
    }

    /// Resolve a theme by name (`Cmd::SetTheme`, harness `.theme(…)`).
    pub fn by_name(name: &str) -> Option<Theme> {
        match name {
            "libra-dark" => Some(Theme::dark()),
            "libra-light" => Some(Theme::light()),
            _ => None,
        }
    }

    /// Parse a theme table (§11) — a flat TOML subset: `key = value`
    /// lines, `#` comments, colors as `"#RRGGBB"`/`"#RRGGBBAA"`,
    /// metrics as numbers. Unspecified tokens inherit from
    /// `libra-dark`, so a theme file overrides only what it cares
    /// about. Errors carry line numbers.
    pub fn from_toml_str(src: &str) -> Result<Theme, String> {
        let mut t = Theme::dark();
        t.name = "custom".to_string();
        for (idx, raw) in src.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let ln = idx + 1;
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {ln}: expected `key = value`"))?;
            let (key, value) = (key.trim(), value.trim());
            let color = |v: &str| -> Result<Color, String> {
                parse_hex_color(v).ok_or_else(|| {
                    format!("line {ln}: `{key}` expects \"#RRGGBB\" or \"#RRGGBBAA\", got {v}")
                })
            };
            let num = |v: &str| -> Result<f32, String> {
                v.parse().map_err(|_| format!("line {ln}: `{key}` expects a number, got {v}"))
            };
            match key {
                "name" => t.name = unquote(value).to_string(),
                "bg" => t.bg = color(value)?,
                "surface" => t.surface = color(value)?,
                "text" => t.text = color(value)?,
                "text_muted" => t.text_muted = color(value)?,
                "accent" => t.accent = color(value)?,
                "accent_text" => t.accent_text = color(value)?,
                "secondary" => t.secondary = color(value)?,
                "selection" => t.selection = color(value)?,
                "radius_md" => t.radius_md = num(value)?,
                "type_sm" => t.type_sm = num(value)?,
                "type_md" => t.type_md = num(value)?,
                "type_xl" => t.type_xl = num(value)?,
                "btn_pad_x" => t.btn_pad_x = num(value)?,
                "btn_pad_y" => t.btn_pad_y = num(value)?,
                other => return Err(format!("line {ln}: unknown token `{other}`")),
            }
        }
        Ok(t)
    }
}

fn unquote(v: &str) -> &str {
    v.trim_matches('"')
}

fn parse_hex_color(v: &str) -> Option<Color> {
    let hex = unquote(v).strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(Color { r: byte(0)?, g: byte(2)?, b: byte(4)?, a: 255 }),
        8 => Some(Color { r: byte(0)?, g: byte(2)?, b: byte(4)?, a: byte(6)? }),
        _ => None,
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn theme_file_overrides_inherit_from_dark() {
        let t = Theme::from_toml_str(
            "# my ops theme\nname = \"solarized\"\nbg = \"#002B36\"\naccent = \"#268BD2\"\n",
        )
        .unwrap();
        assert_eq!(t.name, "solarized");
        assert_eq!(t.bg, Color::rgb(0x00, 0x2B, 0x36));
        assert_eq!(t.accent, Color::rgb(0x26, 0x8B, 0xD2));
        // Unspecified tokens inherit dark:
        assert_eq!(t.text, Theme::dark().text);
        assert_eq!(t.radius_md, Theme::dark().radius_md);
    }

    #[test]
    fn theme_file_errors_carry_line_numbers() {
        let e = Theme::from_toml_str("bg = \"#00\"").unwrap_err();
        assert!(e.contains("line 1"), "{e}");
        let e = Theme::from_toml_str("\n\nnot a kv line").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
        let e = Theme::from_toml_str("bgg = \"#002B36\"").unwrap_err();
        assert!(e.contains("unknown token `bgg`"), "{e}");
    }

    #[test]
    fn alpha_colors_parse() {
        let t = Theme::from_toml_str("selection = \"#26648080\"").unwrap();
        assert_eq!(t.selection, Color { r: 0x26, g: 0x64, b: 0x80, a: 0x80 });
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}
