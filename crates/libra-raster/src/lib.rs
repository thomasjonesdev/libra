//! libra-raster — the canonical executor: pure-Rust CPU rasterization
//! (tiny-skia geometry, fontdue glyphs, deterministic PNG). Same DrawCmd list
//! in → same bytes out on every machine. `render_damage` is a cost
//! optimization that stays byte-equivalent to a full render (property-tested).
//!
//! Paint-path crates (this one included) compile at `opt-level = 2` under
//! the workspace `[profile.dev.package.*]` so plain debug runs stay within
//! the §22 damage-tracked budget — pixel loops are not usable at opt-level 0.
//!
//! Architecture: ARCHITECTURE.md §9.2 (canonical executor), §9.3 (damage
//! layers).

use libra_draw::{Color, DrawCmd, FontStyle, Rect};
use libra_components::TextMeasurer;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub struct Renderer {
    /// Work Sans Medium (500) — bundled as [`FontStyle::Regular`] (PLAN-GEIST G1).
    regular: fontdue::Font,
    /// Work Sans Bold (700) — [`FontStyle::Bold`].
    bold: fontdue::Font,
    /// Work Sans Italic — [`FontStyle::Italic`], markdown emphasis only.
    italic: fontdue::Font,
    /// Space Mono Regular — reserved for future `type.mono.*` (PLAN-GEIST G4); parsed
    /// at init so a bad bundle fails fast, not on first mono draw.
    #[allow(dead_code)]
    mono: fontdue::Font,
}

const FONT_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/WorkSans-Medium.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../../assets/fonts/WorkSans-Bold.ttf");
const FONT_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/WorkSans-Italic.ttf");
const FONT_MONO: &[u8] = include_bytes!("../../../assets/fonts/SpaceMono-Regular.ttf");

impl Renderer {
    pub fn new() -> Self {
        let settings = fontdue::FontSettings::default();
        Renderer {
            regular: fontdue::Font::from_bytes(FONT_REGULAR, settings.clone())
                .expect("bundled Work Sans Medium parses"),
            bold: fontdue::Font::from_bytes(FONT_BOLD, settings.clone())
                .expect("bundled Work Sans Bold parses"),
            italic: fontdue::Font::from_bytes(FONT_ITALIC, settings.clone())
                .expect("bundled Work Sans Italic parses"),
            mono: fontdue::Font::from_bytes(FONT_MONO, settings)
                .expect("bundled Space Mono Regular parses"),
        }
    }

    fn font(&self, style: FontStyle) -> &fontdue::Font {
        match style {
            FontStyle::Regular => &self.regular,
            FontStyle::Bold => &self.bold,
            FontStyle::Italic => &self.italic,
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
        self.replay(&mut pixmap, cmds, width, height, None);
        pixmap
    }

    /// Execute `cmds` into `pixmap`, tracking the clip stack (PLAN-SCROLL).
    /// `damage`, when set, restricts drawing to cmds touching that rect and
    /// clears only it (the §9.3 partial-repaint path). Clip cmds are **always**
    /// processed, so clip state stays correct even when draws are skipped —
    /// which is what keeps the damage replay byte-identical to a full render.
    fn replay(&self, pixmap: &mut Pixmap, cmds: &[DrawCmd], width: u32, height: u32, damage: Option<Rect>) {
        let mut clip_stack: Vec<Rect> = Vec::new();
        let mut clip: Option<Clip> = None;
        for cmd in cmds {
            match cmd {
                DrawCmd::PushClip { rect } => {
                    clip_stack.push(*rect);
                    clip = clip_from_stack(&clip_stack).map(|r| Clip::new(r, width, height));
                }
                DrawCmd::PopClip => {
                    clip_stack.pop();
                    clip = clip_from_stack(&clip_stack).map(|r| Clip::new(r, width, height));
                }
                DrawCmd::Clear(c) => match damage {
                    Some(d) => fill_rect_exact(
                        pixmap,
                        d.x as i32,
                        d.y as i32,
                        (d.x + d.w) as i32,
                        (d.y + d.h) as i32,
                        *c,
                    ),
                    None => pixmap.fill(tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)),
                },
                _ => {
                    let draw_it = match damage {
                        Some(d) => cmd.bounds().is_some_and(|b| b.intersects(&d)),
                        None => true,
                    };
                    if draw_it {
                        self.draw(pixmap, cmd, clip.as_ref());
                    }
                }
            }
        }
    }

    fn draw(&self, pixmap: &mut Pixmap, cmd: &DrawCmd, clip: Option<&Clip>) {
        match cmd {
            // Handled by `replay`; never routed here.
            DrawCmd::Clear(_) | DrawCmd::PushClip { .. } | DrawCmd::PopClip => {}
            DrawCmd::Rect { rect, color } => self.fill_rect(pixmap, *rect, *color, clip),
            DrawCmd::RRect { rect, radius, color } => {
                self.fill_rrect(pixmap, *rect, *radius, *color, clip)
            }
            DrawCmd::Text { x, y, content, px, style, color } => {
                self.draw_text(pixmap, *x, *y, content, *px, *style, *color, clip)
            }
            DrawCmd::Polyline { points, width, color } => {
                self.stroke_polyline(pixmap, points, *width, *color, clip)
            }
            DrawCmd::Strips { x, col_w, strips, color } => {
                fill_strips(pixmap, *x, *col_w, strips, *color, clip)
            }
        }
    }

    fn stroke_polyline(&self, pixmap: &mut Pixmap, points: &[(f32, f32)], width: f32, color: Color, clip: Option<&Clip>) {
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
                clip.map(|c| &c.mask),
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

    fn fill_rect(&self, pixmap: &mut Pixmap, r: Rect, color: Color, clip: Option<&Clip>) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(r.x, r.y, r.w, r.h) {
            pixmap.fill_rect(rect, &Self::paint(color), Transform::identity(), clip.map(|c| &c.mask));
        }
    }

    fn fill_rrect(&self, pixmap: &mut Pixmap, r: Rect, radius: f32, color: Color, clip: Option<&Clip>) {
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
            pixmap.fill_path(&path, &Self::paint(color), FillRule::Winding, Transform::identity(), clip.map(|c| &c.mask));
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
        clip: Option<&Clip>,
    ) {
        let font = self.font(style);
        let baseline = y + self.ascent(px, style);
        let mut pen = x;
        for ch in text.chars() {
            let (metrics, coverage) = font.rasterize(ch, px);
            // Glyph bitmap top-left in pixmap space:
            let gx = (pen + metrics.xmin as f32).round() as i32;
            let gy = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;
            blend_glyph(pixmap, gx, gy, metrics.width, metrics.height, &coverage, color, clip);
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
        cmds: Vec<DrawCmd>,
        width: u32,
        height: u32,
    ) -> Damage {
        let Some(prev) = cache.as_mut() else {
            return self.render_full(cache, cmds, width, height);
        };
        if prev.pixmap.width() != width || prev.pixmap.height() != height {
            return self.render_full(cache, cmds, width, height);
        }
        if prev.cmds == cmds {
            return Damage::None;
        }
        let Some(dirty) = diff_dirty_rect(&prev.cmds, &cmds) else {
            return self.render_full(cache, cmds, width, height);
        };
        // Round out to the pixel grid and clamp to the frame.
        let x0 = (dirty.x.floor() as i32).max(0);
        let y0 = (dirty.y.floor() as i32).max(0);
        let x1 = ((dirty.x + dirty.w).ceil() as i32).min(width as i32);
        let y1 = ((dirty.y + dirty.h).ceil() as i32).min(height as i32);
        if x0 >= x1 || y0 >= y1 {
            // Changed cmds have no visible footprint.
            prev.cmds = cmds;
            return Damage::None;
        }
        let (dw, dh) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let clip_rect = Rect::new(x0 as f32, y0 as f32, dw as f32, dh as f32);

        // Replay every cmd touching the region into the scratch at
        // original coordinates — identical FP math to the full render, now
        // sharing the clip-stack loop (PLAN-SCROLL) so clips replay
        // identically. `Clear` is the one exception (§9.3 Stage 3): its
        // output is coordinate-independent, so filling just the dirty rect is
        // byte-identical to filling the whole scratch — and only the dirty
        // rows are copied out below. Clip cmds are always processed even when
        // draws are skipped, which is what keeps this byte-identical.
        self.replay(&mut prev.scratch, &cmds, width, height, Some(clip_rect));

        // Copy only the dirty rows into the cached frame.
        let stride = width as usize;
        let dst = prev.pixmap.pixels_mut();
        let src = prev.scratch.pixels();
        for row in 0..dh as usize {
            let o = (y0 as usize + row) * stride + x0 as usize;
            dst[o..o + dw as usize].copy_from_slice(&src[o..o + dw as usize]);
        }
        prev.cmds = cmds;
        Damage::Partial(clip_rect)
    }

    /// Stage 4: takes `cmds` by value so the cache can store it by move
    /// instead of cloning — a plain method rather than a closure over
    /// `render_damage`'s local `cmds` so the non-full paths there can
    /// still borrow it afterward (a `move` closure would capture it
    /// whole, up front, and poison every other branch's access).
    fn render_full(&self, cache: &mut Option<FrameCache>, cmds: Vec<DrawCmd>, width: u32, height: u32) -> Damage {
        let pixmap = self.render(&cmds, width, height);
        let scratch = Pixmap::new(width, height).expect("nonzero scratch");
        *cache = Some(FrameCache { cmds, pixmap, scratch });
        Damage::Full
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

/// Overwrites `[x0,x1) × [y0,y1)` with `color`'s premultiplied bytes —
/// the region-clipped equivalent of `Pixmap::fill` (§9.3 Stage 3). Uses
/// the exact same `premultiply().to_color_u8()` conversion tiny-skia's
/// own `fill` uses internally, so a clipped `Clear` replay is
/// byte-identical to a full one restricted to the same pixels.
/// An active rectangular clip (PLAN-SCROLL). `bounds` are integer pixel edges
/// for the manual coverage blits (text, strips); `mask` is the tiny-skia clip
/// for the path fills — both derived from the *same* integer rect, so every
/// draw path clips to identical pixels (byte-equivalence, Appendix A).
struct Clip {
    bounds: (i32, i32, i32, i32), // x0, y0, x1, y1
    mask: tiny_skia::Mask,
}

impl Clip {
    fn new(rect: Rect, width: u32, height: u32) -> Clip {
        let x0 = (rect.x.floor() as i32).clamp(0, width as i32);
        let y0 = (rect.y.floor() as i32).clamp(0, height as i32);
        let x1 = ((rect.x + rect.w).ceil() as i32).clamp(0, width as i32);
        let y1 = ((rect.y + rect.h).ceil() as i32).clamp(0, height as i32);
        let mut mask = tiny_skia::Mask::new(width, height).expect("nonzero mask");
        if x1 > x0 && y1 > y0 {
            let mut pb = PathBuilder::new();
            pb.move_to(x0 as f32, y0 as f32);
            pb.line_to(x1 as f32, y0 as f32);
            pb.line_to(x1 as f32, y1 as f32);
            pb.line_to(x0 as f32, y1 as f32);
            pb.close();
            if let Some(path) = pb.finish() {
                // Non-AA rect fill: coverage is exactly 255 inside the integer
                // rect, 0 outside — matching the `bounds` check the blits use.
                mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
            }
        }
        Clip { bounds: (x0, y0, x1, y1), mask }
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        let (x0, y0, x1, y1) = self.bounds;
        x >= x0 && x < x1 && y >= y0 && y < y1
    }
}

/// Intersect the clip stack into one rect (`None` = empty stack = no clip).
fn clip_from_stack(stack: &[Rect]) -> Option<Rect> {
    let mut acc = *stack.first()?;
    for r in &stack[1..] {
        let x0 = acc.x.max(r.x);
        let y0 = acc.y.max(r.y);
        let x1 = (acc.x + acc.w).min(r.x + r.w);
        let y1 = (acc.y + acc.h).min(r.y + r.h);
        acc = Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0));
    }
    Some(acc)
}

fn fill_rect_exact(pixmap: &mut Pixmap, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
    let c = tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
        .premultiply()
        .to_color_u8();
    let stride = pixmap.width() as usize;
    let pixels = pixmap.pixels_mut();
    for y in y0..y1 {
        let row = y as usize * stride;
        pixels[row + x0 as usize..row + x1 as usize].fill(c);
    }
}

/// src-over blend of `color` at `cov` (0..=255 coverage) onto one
/// premultiplied pixel — the math `blend_glyph` (glyph coverage bitmaps)
/// and `fill_strips`' Stage 7 edge coverage (dense chart columns) both
/// use (PLAN-DASHBOARD-PERF.md).
fn blend_px(pixels: &mut [tiny_skia::PremultipliedColorU8], idx: usize, color: Color, cov: u8) {
    if cov == 0 {
        return;
    }
    let cov = cov as u32;
    // Source in premultiplied form, alpha = color.a * coverage.
    let sa = (color.a as u32 * cov + 127) / 255; // 0..=255
    if sa == 0 {
        return;
    }
    let sr = (color.r as u32 * sa + 127) / 255;
    let sg = (color.g as u32 * sa + 127) / 255;
    let sb = (color.b as u32 * sa + 127) / 255;
    let d = pixels[idx];
    let inv = 255 - sa;
    let r = (sr + (d.red() as u32 * inv + 127) / 255).min(255) as u8;
    let g = (sg + (d.green() as u32 * inv + 127) / 255).min(255) as u8;
    let b = (sb + (d.blue() as u32 * inv + 127) / 255).min(255) as u8;
    let a = (sa + (d.alpha() as u32 * inv + 127) / 255).min(255) as u8;
    pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
        .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
}

/// Dense-chart strips: opaque vertical pixel runs, integer-rounded —
/// no AA machinery on the core, deterministic by construction. The fast
/// path that makes a 16-chart wall affordable (§9.3 / §14.3). Stage 7
/// (PLAN-DASHBOARD-PERF.md) softens the staircase on slow curves with a
/// single blended pixel row above and below the opaque core, instead of
/// switching dense charts to AA polylines — that's the §22 hot-path
/// lesson (18.6ms → 0.64ms, CLAUDE.md) this deliberately avoids
/// re-triggering.
fn fill_strips(pixmap: &mut Pixmap, x: f32, col_w: f32, strips: &[Option<(f32, f32)>], color: Color, clip: Option<&Clip>) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    // Clip bounds default to the whole pixmap (no clip → byte-identical to before).
    let (clip_x0, clip_y0, clip_x1, clip_y1) = clip.map_or((0, 0, pw, ph), |c| c.bounds);
    let opaque = tiny_skia::PremultipliedColorU8::from_rgba(
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
        let (cx0, cx1) = (cx0.max(0).max(clip_x0), cx1.min(pw).min(clip_x1));
        if cx0 >= cx1 {
            continue;
        }

        // The opaque core: rounded to the pixel grid exactly as before
        // (byte-for-byte), with the existing min-1px visibility
        // guarantee for near-flat spans. Row `fill` keeps the same
        // writes as the prior per-pixel loop — cheaper in debug and
        // release (§22 hot path).
        let (fy0, fy1) = (y0.min(*y1), y0.max(*y1));
        let ry0 = fy0.round() as i32;
        let ry1 = (fy1.round() as i32).max(ry0 + 1);
        for cy in ry0.max(0).max(clip_y0)..ry1.min(ph).min(clip_y1) {
            let row = (cy * pw) as usize;
            pixels[row + cx0 as usize..row + cx1 as usize].fill(opaque);
        }

        // Soft edges: the fraction of the true (pre-rounding) span the
        // core above doesn't already cover, blended into the one pixel
        // row just outside it. Purely additive — never touches a pixel
        // the opaque core already painted, so the core stays byte-exact.
        let cov_above = (ry0 as f32 - fy0).clamp(0.0, 1.0);
        let ay = ry0 - 1;
        if cov_above > 0.0 && ay >= 0 && ay < ph && ay >= clip_y0 && ay < clip_y1 {
            let cov = (cov_above * 255.0).round() as u8;
            let row = ay * pw;
            for cx in cx0..cx1 {
                blend_px(pixels, (row + cx) as usize, color, cov);
            }
        }
        let cov_below = (fy1 - ry1 as f32).clamp(0.0, 1.0);
        let by = ry1;
        if cov_below > 0.0 && by >= 0 && by < ph && by >= clip_y0 && by < clip_y1 {
            let cov = (cov_below * 255.0).round() as u8;
            let row = by * pw;
            for cx in cx0..cx1 {
                blend_px(pixels, (row + cx) as usize, color, cov);
            }
        }
    }
}

/// src-over blend of a coverage bitmap tinted `color` onto a premultiplied pixmap.
#[allow(clippy::too_many_arguments)]
fn blend_glyph(
    pixmap: &mut Pixmap,
    gx: i32,
    gy: i32,
    gw: usize,
    gh: usize,
    coverage: &[u8],
    color: Color,
    clip: Option<&Clip>,
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
            if clip.is_some_and(|c| !c.contains(pxx, py)) {
                continue;
            }
            let cov = coverage[(row as usize) * gw + col as usize];
            blend_px(pixels, (py * pw + pxx) as usize, color, cov);
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
            r.render_damage(&mut cache, cmds.clone(), w, h);
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
                    DrawCmd::Clear(_) | DrawCmd::PushClip { .. } | DrawCmd::PopClip => {}
                }
                let outcome = r.render_damage(&mut cache, cmds.clone(), w, h);
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

    /// Stage 3 guard: the `Clear` replay is clipped to the dirty rect.
    /// The randomized property test above mutates near frame edges as
    /// often as not; this pins a dirty rect strictly in the interior
    /// (touching none of the frame's four edges) so every clip boundary
    /// of the clipped fill gets exercised deterministically.
    #[test]
    fn damage_tracked_clear_clip_is_exact_for_an_interior_rect() {
        let r = Renderer::new();
        let (w, h) = (400, 300);
        let base = vec![
            DrawCmd::Clear(tokens::BG),
            DrawCmd::Rect { rect: Rect::new(150.0, 100.0, 40.0, 30.0), color: tokens::ACCENT },
        ];
        let mut cache = None;
        r.render_damage(&mut cache, base.clone(), w, h);

        // Move the rect elsewhere, still well inside the frame — the
        // dirty rect (union of old + new, both interior) never touches
        // x=0, y=0, x=w, or y=h.
        let mut moved = base.clone();
        if let DrawCmd::Rect { rect, .. } = &mut moved[1] {
            rect.x = 200.0;
            rect.y = 140.0;
        }
        let outcome = r.render_damage(&mut cache, moved.clone(), w, h);
        assert!(
            matches!(outcome, Damage::Partial(_)),
            "expected an interior partial repaint, got {outcome:?}"
        );
        let incremental = cache.as_ref().unwrap().pixmap().encode_png().unwrap();
        let full = r.render(&moved, w, h).encode_png().unwrap();
        assert_eq!(incremental, full, "interior Clear-clip diverged from full render");
    }

    /// PLAN-SCROLL: `PushClip`/`PopClip` actually clip — a rect and text
    /// drawn under a clip touch only pixels inside the clip rect.
    #[test]
    fn clip_confines_drawing_to_its_rect() {
        let r = Renderer::new();
        let (w, h) = (200, 120);
        let clip = Rect::new(40.0, 30.0, 80.0, 40.0); // interior window
        let cmds = vec![
            DrawCmd::Clear(tokens::BG),
            DrawCmd::PushClip { rect: clip },
            // A big rect + text that would cover the whole frame if unclipped.
            DrawCmd::Rect { rect: Rect::new(0.0, 0.0, w as f32, h as f32), color: tokens::ACCENT },
            DrawCmd::Text { x: 0.0, y: 0.0, content: "clipped".into(), px: 40.0, style: FontStyle::Regular, color: tokens::TEXT },
            DrawCmd::PopClip,
        ];
        let pm = r.render(&cmds, w, h);
        let px = |x: i32, y: i32| pm.pixels()[(y * w as i32 + x) as usize];
        let bg = tiny_skia::Color::from_rgba8(tokens::BG.r, tokens::BG.g, tokens::BG.b, tokens::BG.a)
            .premultiply()
            .to_color_u8();
        // Outside the clip: untouched background. Inside: painted (≠ bg).
        assert_eq!(px(5, 5), bg, "top-left is outside the clip → background");
        assert_eq!(px(150, 100), bg, "bottom-right is outside the clip → background");
        assert_ne!(px(60, 45), bg, "center is inside the clip → painted");
    }

    /// PLAN-SCROLL determinism: the damage-tracked replay stays byte-identical
    /// to a full render when the frame contains clips (Appendix A) — the clip
    /// stack is reconstructed identically in both paths.
    #[test]
    fn damage_tracked_render_is_byte_equivalent_with_clips() {
        let r = Renderer::new();
        let (w, h) = (240, 160);
        let frame = |ty: f32| {
            vec![
                DrawCmd::Clear(tokens::BG),
                DrawCmd::PushClip { rect: Rect::new(20.0, 20.0, 160.0, 90.0) },
                DrawCmd::Rect { rect: Rect::new(10.0, 10.0, 200.0, 30.0), color: tokens::SURFACE },
                DrawCmd::Text { x: 30.0, y: ty, content: "scrolling text".into(), px: 16.0, style: FontStyle::Regular, color: tokens::TEXT },
                DrawCmd::PopClip,
            ]
        };
        let mut cache = None;
        r.render_damage(&mut cache, frame(50.0), w, h);
        // Move the clipped text (a "scroll") — partial repaint under the clip.
        let outcome = r.render_damage(&mut cache, frame(64.0), w, h);
        let incremental = cache.as_ref().unwrap().pixmap().encode_png().unwrap();
        let full = r.render(&frame(64.0), w, h).encode_png().unwrap();
        assert_eq!(incremental, full, "clipped partial repaint diverged from full ({outcome:?})");
    }

    #[test]
    fn unchanged_frame_is_zero_damage() {
        let r = Renderer::new();
        let cmds = synth_frame(3, 10);
        let mut cache = None;
        assert_eq!(r.render_damage(&mut cache, cmds.clone(), 360, 200), Damage::Full);
        assert_eq!(r.render_damage(&mut cache, cmds, 360, 200), Damage::None);
    }

    #[test]
    fn clear_change_forces_full_repaint() {
        let r = Renderer::new();
        let mut cmds = synth_frame(4, 6);
        let mut cache = None;
        r.render_damage(&mut cache, cmds.clone(), 360, 200);
        cmds[0] = DrawCmd::Clear(Color::rgb(0, 0, 0));
        assert_eq!(r.render_damage(&mut cache, cmds.clone(), 360, 200), Damage::Full);
        let incremental = cache.as_ref().unwrap().pixmap().encode_png().unwrap();
        assert_eq!(incremental, r.render(&cmds, 360, 200).encode_png().unwrap());
    }
}
