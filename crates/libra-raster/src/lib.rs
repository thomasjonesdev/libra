//! libra-raster — the canonical executor (ARCHITECTURE.md §9.2).
//!
//! Pure-Rust CPU rasterization: tiny-skia for geometry, fontdue for glyph
//! coverage, manual src-over blending, deterministic PNG encoding.
//! Same DrawCmd list in → same bytes out, on every machine.
//!
//! Damage tracking (§9.3, Phase 2): `render_damage` diffs the new
//! DrawCmd list against the previous frame's, replays only cmds that
//! touch the changed region, and copies just the dirty rows into the
//! cached pixmap. The contract is *byte-equivalence with the full
//! render* — property-tested below — so damage is purely a cost
//! optimization, invisible to snapshot identity.
//!
//! Phase 0 text stack is fontdue (simple horizontal advance, no shaping);
//! the parley/swash upgrade is Phase 3 per §10. Fonts are bundled (§10):
//! Liberation Sans Regular/Bold for the spike.

use libra_draw::{Color, DrawCmd, FontStyle, Rect};
use libra_widgets::TextMeasurer;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub struct Renderer {
    regular: fontdue::Font,
    bold: fontdue::Font,
}

const FONT_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");

impl Renderer {
    pub fn new() -> Self {
        let settings = fontdue::FontSettings::default();
        Renderer {
            regular: fontdue::Font::from_bytes(FONT_REGULAR, settings.clone())
                .expect("bundled regular font parses"),
            bold: fontdue::Font::from_bytes(FONT_BOLD, settings)
                .expect("bundled bold font parses"),
        }
    }

    fn font(&self, style: FontStyle) -> &fontdue::Font {
        match style {
            FontStyle::Regular => &self.regular,
            FontStyle::Bold => &self.bold,
        }
    }

    /// Line height for a size, from font metrics (ascent - descent + gap).
    fn line_height(&self, px: f32, style: FontStyle) -> f32 {
        let lm = self
            .font(style)
            .horizontal_line_metrics(px)
            .expect("horizontal font has line metrics");
        (lm.ascent - lm.descent + lm.line_gap).ceil()
    }

    fn ascent(&self, px: f32, style: FontStyle) -> f32 {
        self.font(style)
            .horizontal_line_metrics(px)
            .expect("horizontal font has line metrics")
            .ascent
    }

    /// Advance width of a single-line run.
    fn advance_width(&self, text: &str, px: f32, style: FontStyle) -> f32 {
        let font = self.font(style);
        let mut w = 0.0f32;
        for ch in text.chars() {
            w += font.metrics(ch, px).advance_width;
        }
        w.ceil()
    }

    pub fn render(&self, cmds: &[DrawCmd], width: u32, height: u32) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).expect("nonzero pixmap");
        for cmd in cmds {
            self.draw(&mut pixmap, cmd);
        }
        pixmap
    }

    fn draw(&self, pixmap: &mut Pixmap, cmd: &DrawCmd) {
        match cmd {
            DrawCmd::Clear(c) => {
                pixmap.fill(tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a));
            }
            DrawCmd::Rect { rect, color } => self.fill_rect(pixmap, *rect, *color),
            DrawCmd::RRect { rect, radius, color } => {
                self.fill_rrect(pixmap, *rect, *radius, *color)
            }
            DrawCmd::Text { x, y, content, px, style, color } => {
                self.draw_text(pixmap, *x, *y, content, *px, *style, *color)
            }
            DrawCmd::Polyline { points, width, color } => {
                self.stroke_polyline(pixmap, points, *width, *color)
            }
            DrawCmd::Strips { x, col_w, strips, color } => {
                fill_strips(pixmap, *x, *col_w, strips, *color)
            }
        }
    }

    fn stroke_polyline(&self, pixmap: &mut Pixmap, points: &[(f32, f32)], width: f32, color: Color) {
        if points.len() < 2 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].0, points[0].1);
        for (x, y) in &points[1..] {
            pb.line_to(*x, *y);
        }
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(
                &path,
                &Self::paint(color),
                &Stroke { width, ..Stroke::default() },
                Transform::identity(),
                None,
            );
        }
    }

    /// Deterministic PNG bytes for a rendered frame.
    pub fn render_png(&self, cmds: &[DrawCmd], width: u32, height: u32) -> Vec<u8> {
        self.render(cmds, width, height)
            .encode_png()
            .expect("png encodes")
    }

    fn paint(color: Color) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color_rgba8(color.r, color.g, color.b, color.a);
        p.anti_alias = true;
        p
    }

    fn fill_rect(&self, pixmap: &mut Pixmap, r: Rect, color: Color) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(r.x, r.y, r.w, r.h) {
            pixmap.fill_rect(rect, &Self::paint(color), Transform::identity(), None);
        }
    }

    fn fill_rrect(&self, pixmap: &mut Pixmap, r: Rect, radius: f32, color: Color) {
        let radius = radius.min(r.w / 2.0).min(r.h / 2.0);
        // Rounded corners via cubic approximation (kappa).
        const K: f32 = 0.552_284_75;
        let (x, y, w, h) = (r.x, r.y, r.w, r.h);
        let (right, bottom) = (x + w, y + h);
        let c = radius * K;
        let mut pb = PathBuilder::new();
        pb.move_to(x + radius, y);
        pb.line_to(right - radius, y);
        pb.cubic_to(right - radius + c, y, right, y + radius - c, right, y + radius);
        pb.line_to(right, bottom - radius);
        pb.cubic_to(right, bottom - radius + c, right - radius + c, bottom, right - radius, bottom);
        pb.line_to(x + radius, bottom);
        pb.cubic_to(x + radius - c, bottom, x, bottom - radius + c, x, bottom - radius);
        pb.line_to(x, y + radius);
        pb.cubic_to(x, y + radius - c, x + radius - c, y, x + radius, y);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &Self::paint(color), FillRule::Winding, Transform::identity(), None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &self,
        pixmap: &mut Pixmap,
        x: f32,
        y: f32,
        text: &str,
        px: f32,
        style: FontStyle,
        color: Color,
    ) {
        let font = self.font(style);
        let baseline = y + self.ascent(px, style);
        let mut pen = x;
        for ch in text.chars() {
            let (metrics, coverage) = font.rasterize(ch, px);
            // Glyph bitmap top-left in pixmap space:
            let gx = (pen + metrics.xmin as f32).round() as i32;
            let gy = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;
            blend_glyph(pixmap, gx, gy, metrics.width, metrics.height, &coverage, color);
            pen += metrics.advance_width;
        }
    }
}

// ---------------------------------------------------------------------------
// Damage-tracked rendering (§9.3)
// ---------------------------------------------------------------------------

/// The previous frame, kept by the caller for damage tracking.
pub struct FrameCache {
    cmds: Vec<DrawCmd>,
    pixmap: Pixmap,
    /// Reused full-size scratch for partial repaints.
    scratch: Pixmap,
}

impl FrameCache {
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Damage {
    /// Whole frame re-rasterized (first frame, size change, Clear change).
    Full,
    /// Nothing changed; cached pixmap reused untouched.
    None,
    /// Only this region re-rasterized (logical px, rounded out).
    Partial(Rect),
}

impl Renderer {
    /// Damage-tracked render (§9.3): updates `cache` to the new frame,
    /// re-rasterizing only what changed. Byte-equivalent to
    /// `render(cmds, …)` by contract (property-tested).
    ///
    /// Mechanism: cmds whose conservative bounds touch the dirty region
    /// (union of changed cmds' bounds, pixel-aligned) replay into a
    /// full-size scratch at their **original coordinates** — translated
    /// re-rasterization is not FP-bit-identical for stroked paths, so
    /// geometry must never move — then only the dirty rows copy into
    /// the cached frame. The win is skipping the raster cost of
    /// everything outside the damage, not a smaller canvas.
    pub fn render_damage(
        &self,
        cache: &mut Option<FrameCache>,
        cmds: &[DrawCmd],
        width: u32,
        height: u32,
    ) -> Damage {
        let full = |cache: &mut Option<FrameCache>, s: &Self| {
            let pixmap = s.render(cmds, width, height);
            let scratch = Pixmap::new(width, height).expect("nonzero scratch");
            *cache = Some(FrameCache { cmds: cmds.to_vec(), pixmap, scratch });
            Damage::Full
        };

        let Some(prev) = cache.as_mut() else { return full(cache, self) };
        if prev.pixmap.width() != width || prev.pixmap.height() != height {
            return full(cache, self);
        }
        if prev.cmds == cmds {
            return Damage::None;
        }
        let Some(dirty) = diff_dirty_rect(&prev.cmds, cmds) else {
            return full(cache, self);
        };
        // Round out to the pixel grid and clamp to the frame.
        let x0 = (dirty.x.floor() as i32).max(0);
        let y0 = (dirty.y.floor() as i32).max(0);
        let x1 = ((dirty.x + dirty.w).ceil() as i32).min(width as i32);
        let y1 = ((dirty.y + dirty.h).ceil() as i32).min(height as i32);
        if x0 >= x1 || y0 >= y1 {
            // Changed cmds have no visible footprint.
            prev.cmds = cmds.to_vec();
            return Damage::None;
        }
        let (dw, dh) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let clip_rect = Rect::new(x0 as f32, y0 as f32, dw as f32, dh as f32);

        // Replay every cmd touching the region into the scratch at
        // original coordinates — identical FP math to the full render.
        for cmd in cmds {
            let touches = match cmd.bounds() {
                None => true, // Clear: repaints the damaged region as bg
                Some(b) => b.intersects(&clip_rect),
            };
            if touches {
                self.draw(&mut prev.scratch, cmd);
            }
        }

        // Copy only the dirty rows into the cached frame.
        let stride = width as usize;
        let dst = prev.pixmap.pixels_mut();
        let src = prev.scratch.pixels();
        for row in 0..dh as usize {
            let o = (y0 as usize + row) * stride + x0 as usize;
            dst[o..o + dw as usize].copy_from_slice(&src[o..o + dw as usize]);
        }
        prev.cmds = cmds.to_vec();
        Damage::Partial(clip_rect)
    }
}

/// Union of bounds of cmds that differ between the two lists (common
/// prefix/suffix excluded). None = full repaint required (a Clear
/// changed, or a changed cmd has unbounded extent).
fn diff_dirty_rect(prev: &[DrawCmd], new: &[DrawCmd]) -> Option<Rect> {
    let prefix = prev.iter().zip(new.iter()).take_while(|(a, b)| a == b).count();
    let max_suffix = prev.len().min(new.len()) - prefix;
    let suffix = prev
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(max_suffix);

    let changed = prev[prefix..prev.len() - suffix]
        .iter()
        .chain(new[prefix..new.len() - suffix].iter());

    let mut dirty: Option<Rect> = None;
    for cmd in changed {
        match cmd.bounds() {
            None => return None, // Clear changed → full repaint
            Some(b) => dirty = Some(dirty.map_or(b, |d| d.union(&b))),
        }
    }
    dirty.or(Some(Rect::new(0.0, 0.0, 0.0, 0.0)))
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextMeasurer for Renderer {
    fn measure(&self, text: &str, px: f32, style: FontStyle) -> (f32, f32) {
        (self.advance_width(text, px, style), self.line_height(px, style))
    }
}

/// Dense-chart strips: opaque vertical pixel runs, integer-rounded —
/// no AA machinery, deterministic by construction. The fast path that
/// makes a 16-chart wall affordable (§9.3 / §14.3).
fn fill_strips(pixmap: &mut Pixmap, x: f32, col_w: f32, strips: &[Option<(f32, f32)>], color: Color) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let px = tiny_skia::PremultipliedColorU8::from_rgba(
        (color.r as u16 * color.a as u16 / 255) as u8,
        (color.g as u16 * color.a as u16 / 255) as u8,
        (color.b as u16 * color.a as u16 / 255) as u8,
        color.a,
    )
    .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    let pixels = pixmap.pixels_mut();
    for (i, s) in strips.iter().enumerate() {
        let Some((y0, y1)) = s else { continue };
        let cx0 = (x + i as f32 * col_w).round() as i32;
        let cx1 = ((x + (i + 1) as f32 * col_w).round() as i32).max(cx0 + 1);
        let ry0 = (y0.min(*y1)).round() as i32;
        let ry1 = ((y0.max(*y1)).round() as i32).max(ry0 + 1);
        for cy in ry0.max(0)..ry1.min(ph) {
            let row = cy * pw;
            for cx in cx0.max(0)..cx1.min(pw) {
                pixels[(row + cx) as usize] = px;
            }
        }
    }
}

/// src-over blend of a coverage bitmap tinted `color` onto a premultiplied pixmap.
fn blend_glyph(
    pixmap: &mut Pixmap,
    gx: i32,
    gy: i32,
    gw: usize,
    gh: usize,
    coverage: &[u8],
    color: Color,
) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let pixels = pixmap.pixels_mut();
    for row in 0..gh as i32 {
        let py = gy + row;
        if py < 0 || py >= ph {
            continue;
        }
        for col in 0..gw as i32 {
            let pxx = gx + col;
            if pxx < 0 || pxx >= pw {
                continue;
            }
            let cov = coverage[(row as usize) * gw + col as usize] as u32;
            if cov == 0 {
                continue;
            }
            // Source in premultiplied form, alpha = color.a * coverage.
            let sa = (color.a as u32 * cov + 127) / 255; // 0..=255
            if sa == 0 {
                continue;
            }
            let sr = (color.r as u32 * sa + 127) / 255;
            let sg = (color.g as u32 * sa + 127) / 255;
            let sb = (color.b as u32 * sa + 127) / 255;
            let idx = (py * pw + pxx) as usize;
            let d = pixels[idx];
            let inv = 255 - sa;
            let r = (sr + (d.red() as u32 * inv + 127) / 255).min(255) as u8;
            let g = (sg + (d.green() as u32 * inv + 127) / 255).min(255) as u8;
            let b = (sb + (d.blue() as u32 * inv + 127) / 255).min(255) as u8;
            let a = (sa + (d.alpha() as u32 * inv + 127) / 255).min(255) as u8;
            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
                .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_draw::tokens;

    #[test]
    fn rendering_is_bit_reproducible() {
        let r = Renderer::new();
        let cmds = vec![
            DrawCmd::Clear(tokens::BG),
            DrawCmd::RRect {
                rect: Rect::new(10.0, 10.0, 120.0, 32.0),
                radius: 6.0,
                color: tokens::ACCENT,
            },
            DrawCmd::Text {
                x: 20.0,
                y: 16.0,
                content: "Libra 0.0.1".into(),
                px: 14.0,
                style: FontStyle::Regular,
                color: tokens::ACCENT_TEXT,
            },
        ];
        let a = r.render_png(&cmds, 160, 60);
        let b = r.render_png(&cmds, 160, 60);
        assert_eq!(a, b, "two renders of the same display list must be byte-identical");
        assert!(a.len() > 100);
    }

    #[test]
    fn measure_is_monotonic_and_nonzero() {
        let r = Renderer::new();
        let (w1, h) = r.measure("+", 14.0, FontStyle::Regular);
        let (w2, _) = r.measure("++", 14.0, FontStyle::Regular);
        assert!(w1 > 0.0 && h > 0.0 && w2 > w1);
    }

    /// Deterministic pseudo-random cmd list — text, rects, rrects,
    /// polylines at hashed positions.
    fn synth_frame(seed: u64, n: usize) -> Vec<DrawCmd> {
        let mut cmds = vec![DrawCmd::Clear(tokens::BG)];
        let mut h = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut next = || {
            h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93).rotate_right(23).wrapping_add(1);
            (h % 1000) as f32 / 1000.0
        };
        for i in 0..n {
            let (a, b, c, d) = (next(), next(), next(), next());
            match i % 5 {
                0 => cmds.push(DrawCmd::Rect {
                    rect: Rect::new(a * 300.0, b * 160.0, 20.0 + c * 60.0, 10.0 + d * 40.0),
                    color: Color::rgb((a * 255.0) as u8, 100, 200),
                }),
                1 => cmds.push(DrawCmd::RRect {
                    rect: Rect::new(a * 300.0, b * 160.0, 30.0 + c * 50.0, 14.0 + d * 30.0),
                    radius: 4.0,
                    color: tokens::SECONDARY,
                }),
                2 => cmds.push(DrawCmd::Text {
                    x: a * 300.0,
                    y: b * 180.0,
                    content: format!("t{i}"),
                    px: 12.0 + c * 10.0,
                    style: FontStyle::Regular,
                    color: tokens::TEXT,
                }),
                3 => cmds.push(DrawCmd::Polyline {
                    points: (0..8)
                        .map(|k| (a * 60.0 + k as f32 * 40.0, 100.0 + (b + c * k as f32).sin() * 60.0))
                        .collect(),
                    width: 1.0 + d,
                    color: tokens::ACCENT,
                }),
                _ => cmds.push(DrawCmd::Strips {
                    x: a * 200.0,
                    col_w: 1.0,
                    strips: (0..40)
                        .map(|k| {
                            let y = 60.0 + (b * 6.0 + k as f32 * c).sin() * 40.0;
                            Some((y, y + 4.0 + d * 20.0))
                        })
                        .collect(),
                    color: tokens::ACCENT,
                }),
            }
        }
        cmds
    }

    /// The damage contract: an incrementally-updated frame is
    /// byte-identical to a from-scratch render of the same cmds —
    /// across many randomized frame-to-frame mutations.
    #[test]
    fn damage_tracked_render_is_byte_equivalent_to_full() {
        let r = Renderer::new();
        let (w, h) = (360, 200);
        for seed in 0..8u64 {
            let mut cache = None;
            let mut cmds = synth_frame(seed, 12);
            r.render_damage(&mut cache, &cmds, w, h);
            for step in 0..10 {
                // Mutate one cmd per step (move a rect / change text / shift polyline).
                let idx = 1 + ((seed as usize + step * 7) % (cmds.len() - 1));
                match &mut cmds[idx] {
                    DrawCmd::Rect { rect, .. } | DrawCmd::RRect { rect, .. } => {
                        rect.x = (rect.x + 17.0) % 300.0;
                        rect.y = (rect.y + 9.0) % 160.0;
                    }
                    DrawCmd::Text { content, x, .. } => {
                        content.push('!');
                        *x = (*x + 23.0) % 280.0;
                    }
                    DrawCmd::Polyline { points, .. } => {
                        for p in points.iter_mut() {
                            p.1 = (p.1 + 11.0) % 190.0;
                        }
                    }
                    DrawCmd::Strips { strips, .. } => {
                        for s in strips.iter_mut().flatten() {
                            s.1 = (s.1 + 13.0) % 190.0;
                        }
                    }
                    DrawCmd::Clear(_) => {}
                }
                let outcome = r.render_damage(&mut cache, &cmds, w, h);
                let incremental = cache.as_ref().unwrap().pixmap().encode_png().unwrap();
                let full = r.render(&cmds, w, h).encode_png().unwrap();
                assert_eq!(
                    incremental, full,
                    "divergence at seed {seed} step {step} ({outcome:?})"
                );
                assert!(
                    matches!(outcome, Damage::Partial(_)),
                    "single-cmd mutation should damage partially, got {outcome:?}"
                );
            }
        }
    }

    #[test]
    fn unchanged_frame_is_zero_damage() {
        let r = Renderer::new();
        let cmds = synth_frame(3, 10);
        let mut cache = None;
        assert_eq!(r.render_damage(&mut cache, &cmds, 360, 200), Damage::Full);
        assert_eq!(r.render_damage(&mut cache, &cmds, 360, 200), Damage::None);
    }

    #[test]
    fn clear_change_forces_full_repaint() {
        let r = Renderer::new();
        let mut cmds = synth_frame(4, 6);
        let mut cache = None;
        r.render_damage(&mut cache, &cmds, 360, 200);
        cmds[0] = DrawCmd::Clear(Color::rgb(0, 0, 0));
        assert_eq!(r.render_damage(&mut cache, &cmds, 360, 200), Damage::Full);
        let incremental = cache.as_ref().unwrap().pixmap().encode_png().unwrap();
        assert_eq!(incremental, r.render(&cmds, 360, 200).encode_png().unwrap());
    }
}
