//! libra-draw — the display list: `DrawCmd`, the universal currency between
//! the component layer and executors, plus the theme tokens.
//!
//! Architecture: ARCHITECTURE.md §9.1 (the display list), §11 (theming).

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
    /// Push a rectangular clip (§9.1; PLAN-SCROLL). Subsequent draws are
    /// masked to the intersection of the clip stack until the matching
    /// [`DrawCmd::PopClip`]. A pure rect clip — deterministic, so the
    /// damage-tracked replay stays byte-identical (Appendix A).
    PushClip { rect: Rect },
    /// Pop the innermost clip pushed by [`DrawCmd::PushClip`].
    PopClip,
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
            // Clip cmds carry no ink; they're handled explicitly by the
            // executor (always replayed to keep clip state), and a *changed*
            // clip forces a full repaint (None) — the safe, rare case.
            DrawCmd::PushClip { .. } | DrawCmd::PopClip => None,
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
/// values, kept verbatim as the `libra-dark` theme's table — snapshot
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

    // --- PLAN-TOKENS T2 scales (metrics are theme-independent; shared by
    //     libra-dark and libra-light). Read only by post-T1 components, so
    //     existing snapshots are undisturbed. ---

    pub const RADIUS_SM: f32 = 4.0;
    pub const RADIUS_LG: f32 = 10.0;
    /// Pill radius — clamped by the rasterizer to half the rect's short side.
    pub const RADIUS_FULL: f32 = 9999.0;

    pub const TYPE_XS: f32 = 11.0;
    pub const TYPE_LG: f32 = 18.0;

    /// Spacing scale, steps 1..=8 (4px-ish base). Resolve via `Theme::space`
    /// so global density applies.
    pub const SPACE: [f32; 8] = [2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0];

    pub const CONTROL_H_SM: f32 = 24.0;
    pub const CONTROL_H_MD: f32 = 32.0;
    pub const CONTROL_H_LG: f32 = 40.0;
}

/// Global spacing density (§11, PLAN-TOKENS T4). Multiplies the spacing scale
/// and control metrics through `Theme::space` / `Theme::dense`; `Normal` is
/// ×1.0 so the built-in themes render exactly as before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    Compact,
    #[default]
    Normal,
    Comfortable,
}

impl Density {
    pub fn factor(self) -> f32 {
        match self {
            Density::Compact => 0.875,
            Density::Normal => 1.0,
            Density::Comfortable => 1.125,
        }
    }
}

/// A resolved theme: the named-token table every component styles through
/// (§11 — no hex codes in components or markup). Two ship built in:
/// `libra-dark` (mission-control default) and `libra-light`. Themes are
/// part of the determinism contract: a snapshot's identity includes the
/// theme it rendered under. Runtime switching is `Effect::SetTheme`;
/// TOML theme tables + hot reload are the remaining §11 items.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub surface: Color,
    /// One step above `surface`: hover washes, raised panels, list-row
    /// selection backgrounds (PLAN-TOKENS T1).
    pub surface_raised: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub secondary: Color,
    pub selection: Color,
    /// 1px rules: `Separator`, pane seams, table row dividers
    /// (PLAN-TOKENS T1). Sits between `bg` and `secondary` so it reads
    /// as structure, not content.
    pub hairline: Color,
    /// Full-viewport modal backdrop (`<Dialog>`, PLAN-CATALOG) — an
    /// alpha wash over the app while a dialog is up.
    pub scrim: Color,
    /// Semantic status colors (PLAN-TOKENS T1) — `Badge`/`Status`/
    /// `Toast` variants resolve these by meaning, never by hex.
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,
    pub radius_md: f32,
    pub type_sm: f32,
    pub type_md: f32,
    pub type_xl: f32,
    pub btn_pad_x: f32,
    pub btn_pad_y: f32,

    // --- PLAN-TOKENS T2/T4: the expanded named table. Every field below is
    //     read only by post-T1 components, so libra-dark / libra-light stay
    //     byte-identical for existing snapshots (tests/theme assert this). ---
    /// Text on an accent or status fill (e.g. a Badge label).
    pub text_inverse: Color,
    /// Structural outline, one step stronger than `hairline` — input/card edges.
    pub border: Color,
    /// Soft status pairs: a tinted `bg` fill + a legible `fg` text, for
    /// Badge / Status / Toast (resolved by meaning, never by hex).
    pub danger_bg: Color,
    pub danger_fg: Color,
    pub warning_bg: Color,
    pub warning_fg: Color,
    pub success_bg: Color,
    pub success_fg: Color,
    pub info_bg: Color,
    pub info_fg: Color,
    /// Radius ramp (`radius_md` above is the mid step).
    pub radius_sm: f32,
    pub radius_lg: f32,
    /// Pill radius (clamped to half the short side at raster time).
    pub radius_full: f32,
    /// Type ramp (`type_sm`/`type_md`/`type_xl` above).
    pub type_xs: f32,
    pub type_lg: f32,
    /// Spacing scale, steps 1..=8. Resolve via [`Theme::space`] so density applies.
    pub space: [f32; 8],
    /// Control heights (sm/md/lg). Resolve via [`Theme::dense`] for density.
    pub control_h_sm: f32,
    pub control_h_md: f32,
    pub control_h_lg: f32,
    /// Global spacing density (§11 / T4).
    pub density: Density,
}

impl Theme {
    /// The mission-control default. Byte-identical to the Phase 0
    /// token values — existing snapshots are recorded under it. New
    /// T1 keys (`surface_raised`/`hairline`/status colors) are only
    /// read by post-T1 components, so adding them cannot disturb any
    /// committed snapshot.
    pub fn dark() -> Theme {
        Theme {
            name: "libra-dark".to_string(),
            bg: tokens::BG,
            surface: tokens::SURFACE,
            surface_raised: Color::rgb(0x1B, 0x22, 0x2E),
            text: tokens::TEXT,
            text_muted: tokens::TEXT_MUTED,
            accent: tokens::ACCENT,
            accent_text: tokens::ACCENT_TEXT,
            secondary: tokens::SECONDARY,
            selection: Color::rgb(0x2A, 0x4A, 0x7A),
            hairline: Color::rgb(0x1E, 0x25, 0x31),
            scrim: Color { r: 0x00, g: 0x00, b: 0x00, a: 0x8C },
            danger: Color::rgb(0xE5, 0x53, 0x4B),
            warning: Color::rgb(0xD9, 0xA4, 0x3F),
            success: Color::rgb(0x3F, 0xB9, 0x50),
            info: tokens::ACCENT,
            radius_md: tokens::RADIUS_MD,
            type_sm: tokens::TYPE_SM,
            type_md: tokens::TYPE_MD,
            type_xl: tokens::TYPE_XL,
            btn_pad_x: tokens::BTN_PAD_X,
            btn_pad_y: tokens::BTN_PAD_Y,
            text_inverse: Color::rgb(0xFF, 0xFF, 0xFF),
            border: Color::rgb(0x2A, 0x33, 0x41),
            danger_bg: Color::rgb(0x3A, 0x1D, 0x1B),
            danger_fg: Color::rgb(0xF0, 0xA8, 0xA3),
            warning_bg: Color::rgb(0x3A, 0x2E, 0x14),
            warning_fg: Color::rgb(0xE9, 0xC7, 0x7E),
            success_bg: Color::rgb(0x16, 0x31, 0x1F),
            success_fg: Color::rgb(0x8F, 0xD6, 0xA3),
            info_bg: Color::rgb(0x16, 0x26, 0x3A),
            info_fg: Color::rgb(0x9D, 0xC0, 0xF0),
            radius_sm: tokens::RADIUS_SM,
            radius_lg: tokens::RADIUS_LG,
            radius_full: tokens::RADIUS_FULL,
            type_xs: tokens::TYPE_XS,
            type_lg: tokens::TYPE_LG,
            space: tokens::SPACE,
            control_h_sm: tokens::CONTROL_H_SM,
            control_h_md: tokens::CONTROL_H_MD,
            control_h_lg: tokens::CONTROL_H_LG,
            density: Density::Normal,
        }
    }

    pub fn light() -> Theme {
        Theme {
            name: "libra-light".to_string(),
            bg: Color::rgb(0xF5, 0xF6, 0xF8),
            surface: Color::rgb(0xFF, 0xFF, 0xFF),
            surface_raised: Color::rgb(0xEC, 0xEF, 0xF3),
            text: Color::rgb(0x1A, 0x20, 0x2B),
            text_muted: Color::rgb(0x6B, 0x74, 0x84),
            accent: Color::rgb(0x2A, 0x66, 0xD9),
            accent_text: Color::rgb(0xFF, 0xFF, 0xFF),
            secondary: Color::rgb(0xE4, 0xE7, 0xEC),
            selection: Color::rgb(0xB8, 0xD0, 0xF5),
            hairline: Color::rgb(0xDD, 0xE1, 0xE7),
            scrim: Color { r: 0x14, g: 0x19, b: 0x23, a: 0x6E },
            danger: Color::rgb(0xC7, 0x35, 0x2E),
            warning: Color::rgb(0xA9, 0x71, 0x08),
            success: Color::rgb(0x1F, 0x87, 0x3D),
            info: Color::rgb(0x2A, 0x66, 0xD9),
            radius_md: tokens::RADIUS_MD,
            type_sm: tokens::TYPE_SM,
            type_md: tokens::TYPE_MD,
            type_xl: tokens::TYPE_XL,
            btn_pad_x: tokens::BTN_PAD_X,
            btn_pad_y: tokens::BTN_PAD_Y,
            text_inverse: Color::rgb(0xFF, 0xFF, 0xFF),
            border: Color::rgb(0xD0, 0xD5, 0xDC),
            danger_bg: Color::rgb(0xFB, 0xE9, 0xE7),
            danger_fg: Color::rgb(0xA2, 0x28, 0x20),
            warning_bg: Color::rgb(0xFB, 0xF0, 0xDA),
            warning_fg: Color::rgb(0x8A, 0x5B, 0x06),
            success_bg: Color::rgb(0xE3, 0xF4, 0xE8),
            success_fg: Color::rgb(0x1B, 0x6E, 0x33),
            info_bg: Color::rgb(0xE4, 0xED, 0xFB),
            info_fg: Color::rgb(0x1E, 0x4F, 0xA8),
            radius_sm: tokens::RADIUS_SM,
            radius_lg: tokens::RADIUS_LG,
            radius_full: tokens::RADIUS_FULL,
            type_xs: tokens::TYPE_XS,
            type_lg: tokens::TYPE_LG,
            space: tokens::SPACE,
            control_h_sm: tokens::CONTROL_H_SM,
            control_h_md: tokens::CONTROL_H_MD,
            control_h_lg: tokens::CONTROL_H_LG,
            density: Density::Normal,
        }
    }

    /// Spacing scale step `level` (1..=8), scaled by the theme's [`Density`].
    /// Components resolve spacing through this, never a raw pixel literal.
    pub fn space(&self, level: usize) -> f32 {
        self.space[level.clamp(1, 8) - 1] * self.density.factor()
    }

    /// Apply the density multiplier to a base metric (control heights/pads).
    pub fn dense(&self, px: f32) -> f32 {
        px * self.density.factor()
    }

    /// Resolve a theme by name (`Effect::SetTheme`, harness `.theme(…)`).
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
                "surface_raised" => t.surface_raised = color(value)?,
                "text" => t.text = color(value)?,
                "text_muted" => t.text_muted = color(value)?,
                "accent" => t.accent = color(value)?,
                "accent_text" => t.accent_text = color(value)?,
                "secondary" => t.secondary = color(value)?,
                "selection" => t.selection = color(value)?,
                "hairline" => t.hairline = color(value)?,
                "scrim" => t.scrim = color(value)?,
                "danger" => t.danger = color(value)?,
                "warning" => t.warning = color(value)?,
                "success" => t.success = color(value)?,
                "info" => t.info = color(value)?,
                "radius_md" => t.radius_md = num(value)?,
                "type_sm" => t.type_sm = num(value)?,
                "type_md" => t.type_md = num(value)?,
                "type_xl" => t.type_xl = num(value)?,
                "btn_pad_x" => t.btn_pad_x = num(value)?,
                "btn_pad_y" => t.btn_pad_y = num(value)?,
                "text_inverse" => t.text_inverse = color(value)?,
                "border" => t.border = color(value)?,
                "danger_bg" => t.danger_bg = color(value)?,
                "danger_fg" => t.danger_fg = color(value)?,
                "warning_bg" => t.warning_bg = color(value)?,
                "warning_fg" => t.warning_fg = color(value)?,
                "success_bg" => t.success_bg = color(value)?,
                "success_fg" => t.success_fg = color(value)?,
                "info_bg" => t.info_bg = color(value)?,
                "info_fg" => t.info_fg = color(value)?,
                "radius_sm" => t.radius_sm = num(value)?,
                "radius_lg" => t.radius_lg = num(value)?,
                "radius_full" => t.radius_full = num(value)?,
                "type_xs" => t.type_xs = num(value)?,
                "type_lg" => t.type_lg = num(value)?,
                "control_h_sm" => t.control_h_sm = num(value)?,
                "control_h_md" => t.control_h_md = num(value)?,
                "control_h_lg" => t.control_h_lg = num(value)?,
                "density" => {
                    t.density = match unquote(value) {
                        "compact" => Density::Compact,
                        "normal" => Density::Normal,
                        "comfortable" => Density::Comfortable,
                        other => {
                            return Err(format!(
                                "line {ln}: `density` expects compact|normal|comfortable, got {other}"
                            ))
                        }
                    }
                }
                // space_1 … space_8
                k if k
                    .strip_prefix("space_")
                    .and_then(|n| n.parse::<usize>().ok())
                    .is_some_and(|n| (1..=8).contains(&n)) =>
                {
                    let idx = k["space_".len()..].parse::<usize>().unwrap() - 1;
                    t.space[idx] = num(value)?;
                }
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

    #[test]
    fn expanded_tokens_and_scales_present() {
        for th in [Theme::dark(), Theme::light()] {
            // Ramps are ordered and sane.
            assert!(th.radius_sm < th.radius_md && th.radius_md < th.radius_lg);
            assert!(th.type_xs < th.type_sm && th.type_lg < th.type_xl);
            assert_eq!(th.space.len(), 8);
            assert!(th.space.windows(2).all(|w| w[0] < w[1]), "spacing scale ascends");
            assert!(th.control_h_sm < th.control_h_md && th.control_h_md < th.control_h_lg);
            assert_eq!(th.density, Density::Normal);
        }
    }

    #[test]
    fn density_scales_spacing_and_metrics() {
        let mut t = Theme::dark();
        let base = t.space(4);
        assert_eq!(t.density.factor(), 1.0);
        assert_eq!(t.space(4), t.space[3]); // normal = ×1
        t.density = Density::Comfortable;
        assert!(t.space(4) > base);
        assert_eq!(t.dense(t.control_h_md), t.control_h_md * 1.125);
        // Out-of-range levels clamp.
        assert_eq!(Theme::dark().space(0), Theme::dark().space(1));
        assert_eq!(Theme::dark().space(99), Theme::dark().space(8));
    }

    #[test]
    fn shipped_sample_theme_parses() {
        // The designer-facing sample must always load (§11 / PLAN-TOKENS).
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../themes/solarized.toml"));
        let t = Theme::from_toml_str(src).expect("themes/solarized.toml parses");
        assert_eq!(t.name, "solarized-dark");
    }

    #[test]
    fn toml_parses_new_keys_and_density() {
        let t = Theme::from_toml_str(
            "danger_bg = \"#101010\"\nspace_3 = 10\ndensity = \"compact\"\n",
        )
        .unwrap();
        assert_eq!(t.danger_bg, Color::rgb(0x10, 0x10, 0x10));
        assert_eq!(t.space[2], 10.0);
        assert_eq!(t.density, Density::Compact);
        let e = Theme::from_toml_str("density = \"cozy\"").unwrap_err();
        assert!(e.contains("compact|normal|comfortable"), "{e}");
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}
