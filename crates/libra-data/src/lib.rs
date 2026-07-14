//! libra-data — the high-rate data plane (ARCHITECTURE.md §14).
//!
//! Samples flow: source (tape / generator / [later] sockets) → ingest
//! thread → per-series ring buffer → read at paint time through a
//! `SeriesHandle` → M4 downsample → polyline geometry. State holds
//! handles; `update` never sees a sample.
//!
//! Determinism at the boundary (§14.4): in tests, sources are recorded
//! tapes fed *synchronously* through the same ring buffers — a chart
//! snapshot test feeds a tape and asserts bytes.
//!
//! Phase 2 locking shortcut: rings are `Mutex`-guarded rather than the
//! specced lock-free SPSC seqlock. At 10 kHz writes + 60 Hz reads the
//! uncontended mutex is nanoseconds, and the API (push / snapshot)
//! is shaped so the seqlock swap is invisible when it lands.

use libra_core::{SeriesId, SourceSpec};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Sample time, ms (source-relative; monotonic per series).
    pub t_ms: f64,
    pub v: f64,
}

// ---------------------------------------------------------------------------
// Ring buffer
// ---------------------------------------------------------------------------

/// Fixed-capacity ring with a monotonic sequence number. `generation`
/// (total pushes) keys downsample caches: same generation + window +
/// width ⇒ same geometry.
pub struct Ring {
    inner: Mutex<RingInner>,
}

struct RingInner {
    cap: usize,
    buf: Vec<Sample>,
    /// Total samples ever pushed; buf holds the last `min(gen, cap)`.
    generation: u64,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "ring capacity must be nonzero");
        Ring { inner: Mutex::new(RingInner { cap, buf: Vec::with_capacity(cap), generation: 0 }) }
    }

    pub fn push(&self, s: Sample) {
        let mut r = self.inner.lock().expect("ring lock");
        let cap = r.cap;
        let gen = r.generation as usize;
        if r.buf.len() < cap {
            r.buf.push(s);
        } else {
            r.buf[gen % cap] = s;
        }
        r.generation += 1;
    }

    pub fn generation(&self) -> u64 {
        self.inner.lock().expect("ring lock").generation
    }

    /// Samples with `t_ms >= t_from`, oldest→newest, plus the ring
    /// generation at snapshot time. Always internally consistent
    /// (§14.4b): one lock, one copy.
    pub fn snapshot_from(&self, t_from: f64) -> (Vec<Sample>, u64) {
        let r = self.inner.lock().expect("ring lock");
        let n = r.buf.len();
        let mut out = Vec::new();
        // Oldest slot is generation % cap once wrapped, else 0.
        let start = if (r.generation as usize) > n { r.generation as usize % r.cap } else { 0 };
        for i in 0..n {
            let s = r.buf[(start + i) % n.max(1)];
            if s.t_ms >= t_from {
                out.push(s);
            }
        }
        (out, r.generation)
    }

    /// M4 column strips (§14.3), computed in one pass under the lock —
    /// no sample copy. For each pixel column: the min/max envelope,
    /// bridged with the previous column's last value so a fast-moving
    /// line has no vertical gaps. Returns (strips, count) where strips
    /// is `width_px` slots of `Option<(min, max)>` (None = empty column).
    pub fn m4_strips(
        &self,
        t_from: f64,
        t_to: f64,
        width_px: u32,
    ) -> (Vec<Option<(f64, f64)>>, usize) {
        let mut out = vec![None; width_px as usize];
        if width_px == 0 || t_to <= t_from {
            return (out, 0);
        }
        let r = self.inner.lock().expect("ring lock");
        let n = r.buf.len();
        if n == 0 {
            return (out, 0);
        }
        let start = if (r.generation as usize) > n { r.generation as usize % r.cap } else { 0 };
        let at = |i: usize| r.buf[(start + i) % n];
        // Samples are time-ordered per series: binary-search the window
        // start instead of scanning the whole ring (the window is often
        // a fraction of capacity).
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if at(mid).t_ms < t_from {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let col_scale = width_px as f64 / (t_to - t_from);
        let mut count = 0usize;
        let mut prev_v: Option<f64> = None;
        // Logical order is buf[start..] then buf[..start]; iterate the
        // in-window suffix as (at most) two contiguous slices — no
        // per-sample modulo on the hot path.
        let (newer, older) = (&r.buf[start..], &r.buf[..start]);
        let (first, second) = if lo < newer.len() {
            (&newer[lo..], older)
        } else {
            (&older[lo - newer.len()..], &[][..])
        };
        'outer: for slice in [first, second] {
            for s in slice {
                if s.t_ms > t_to {
                    break 'outer;
                }
                if s.t_ms < t_from {
                    prev_v = None;
                    continue;
                }
                count += 1;
                let c = (((s.t_ms - t_from) * col_scale) as usize).min(width_px as usize - 1);
                let slot = &mut out[c];
                match slot {
                    None => {
                        // Bridge from the previous column's last value.
                        let (mut cl, mut ch) = (s.v, s.v);
                        if let Some(pv) = prev_v {
                            cl = cl.min(pv);
                            ch = ch.max(pv);
                        }
                        *slot = Some((cl, ch));
                    }
                    Some((cl, ch)) => {
                        *cl = cl.min(s.v);
                        *ch = ch.max(s.v);
                    }
                }
                prev_v = Some(s.v);
            }
        }
        (out, count)
    }

    /// Bucket samples with `t ∈ [t_from, t_from + ncols·dt)` into
    /// per-column (envelope, count), bridging each column with the last
    /// value before it (`bridge` in, updated bridge out). One ordered
    /// pass over just that time span (binary-searched start).
    #[allow(clippy::type_complexity)]
    pub fn scan_cols(
        &self,
        t_from: f64,
        dt: f64,
        ncols: usize,
        mut bridge: Option<f64>,
    ) -> (Vec<(Option<(f64, f64)>, u32)>, Option<f64>) {
        let mut out = vec![(None, 0u32); ncols];
        let t_to = t_from + ncols as f64 * dt;
        let r = self.inner.lock().expect("ring lock");
        let n = r.buf.len();
        if n == 0 || ncols == 0 {
            return (out, bridge);
        }
        let start = if (r.generation as usize) > n { r.generation as usize % r.cap } else { 0 };
        let at = |i: usize| r.buf[(start + i) % n];
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if at(mid).t_ms < t_from {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let (newer, older) = (&r.buf[start..], &r.buf[..start]);
        let (first, second) = if lo < newer.len() {
            (&newer[lo..], older)
        } else {
            (&older[lo - newer.len()..], &[][..])
        };
        let inv_dt = 1.0 / dt;
        'outer: for slice in [first, second] {
            for s in slice {
                if s.t_ms >= t_to {
                    break 'outer;
                }
                if s.t_ms < t_from {
                    continue;
                }
                let c = (((s.t_ms - t_from) * inv_dt) as usize).min(ncols - 1);
                let (env, count) = &mut out[c];
                match env {
                    None => {
                        let (mut cl, mut ch) = (s.v, s.v);
                        if let Some(b) = bridge {
                            cl = cl.min(b);
                            ch = ch.max(b);
                        }
                        *env = Some((cl, ch));
                    }
                    Some((cl, ch)) => {
                        *cl = cl.min(s.v);
                        *ch = ch.max(s.v);
                    }
                }
                *count += 1;
                bridge = Some(s.v);
            }
        }
        (out, bridge)
    }

    /// Newest sample time, if any.
    pub fn latest_t(&self) -> Option<f64> {
        let r = self.inner.lock().expect("ring lock");
        if r.buf.is_empty() {
            return None;
        }
        let newest = (r.generation as usize + r.buf.len() - 1) % r.buf.len().max(1);
        // When not yet wrapped, newest is simply last pushed:
        let idx = if (r.generation as usize) <= r.buf.len() {
            r.generation as usize - 1
        } else {
            newest
        };
        Some(r.buf[idx % r.buf.len()].t_ms)
    }
}

// ---------------------------------------------------------------------------
// Store: SeriesId → ring (+ ingest thread registry)
// ---------------------------------------------------------------------------

pub const DEFAULT_RING_CAP: usize = 1 << 16; // 65_536 samples ≈ 6.5s @ 10 kHz

pub struct SeriesStore {
    series: RwLock<HashMap<SeriesId, Arc<Ring>>>,
    stops: Mutex<HashMap<SeriesId, Arc<AtomicBool>>>,
    /// §14.3 downsample cache: (series, window, width) → computed
    /// columns. Live-follow reuses all but the few new columns.
    strip_caches: Mutex<HashMap<(SeriesId, u64, u32), StripCache>>,
}

/// Caches *closed* columns only — the column containing the newest
/// sample is still filling and recomputes fresh every paint. Closed
/// columns are immutable because per-series sample time is monotonic:
/// no future sample can land in an earlier column.
struct StripCache {
    dt: f64,
    /// Exclusive end of the closed range: cols cover
    /// `[closed_end - cols.len(), closed_end)` in units of dt.
    closed_end: i64,
    cols: std::collections::VecDeque<(Option<(f64, f64)>, u32)>,
    total_samples: usize,
    /// Last closed sample value — bridges into the open column.
    bridge: Option<f64>,
}

/// What a chart paints: one envelope per pixel column, plus how many
/// samples the window holds (drives the dense/sparse rendering choice).
pub struct ChartColumns {
    pub strips: Vec<Option<(f64, f64)>>,
    pub samples: usize,
    /// Window covered, quantized to whole columns.
    pub t_from: f64,
    pub t_to: f64,
}

impl Default for SeriesStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SeriesStore {
    pub fn new() -> Self {
        SeriesStore {
            series: RwLock::new(HashMap::new()),
            stops: Mutex::new(HashMap::new()),
            strip_caches: Mutex::new(HashMap::new()),
        }
    }

    /// Chart read path (§14.3): M4 column envelopes for the trailing
    /// `window_ms`, quantized to whole columns so a sliding live window
    /// reuses every already-computed column and folds in only the new
    /// ones. Deterministic: a pure function of (ring contents,
    /// window_ms, width_px) — the cache is invisible.
    pub fn chart_columns(&self, id: SeriesId, window_ms: f64, width_px: u32) -> ChartColumns {
        let empty = ChartColumns { strips: vec![], samples: 0, t_from: 0.0, t_to: 0.0 };
        if width_px == 0 || window_ms <= 0.0 {
            return empty;
        }
        let ring = self.ring(id);
        let Some(latest) = ring.latest_t() else { return empty };
        let dt = window_ms / width_px as f64;
        let width = width_px as usize;
        // The column containing `latest` is open (still filling); the
        // closed range shown is the width-1 columns before it.
        let closed_end = (latest / dt).floor() as i64;
        let closed_len = width - 1;

        let key = (id, window_ms.to_bits(), width_px);
        let mut caches = self.strip_caches.lock().expect("strip cache lock");
        // Resize drags touch many widths — keep only the current one per
        // series/window so the cache doesn't grow without bound.
        let wbits = window_ms.to_bits();
        if caches.keys().any(|(sid, wms, wp)| *sid == id && *wms == wbits && *wp != width_px) {
            caches.retain(|(sid, wms, wp), _| {
                *sid != id || *wms != wbits || *wp == width_px
            });
        }
        let cache = caches.entry(key).or_insert(StripCache {
            dt,
            closed_end: i64::MIN,
            cols: std::collections::VecDeque::new(),
            total_samples: 0,
            bridge: None,
        });

        let stale = cache.dt != dt
            || closed_end < cache.closed_end // time went backwards: source restarted
            || cache.cols.len() != closed_len
            || closed_end - cache.closed_end >= closed_len as i64; // jumped a window
        if stale {
            let t_from = (closed_end - closed_len as i64) as f64 * dt;
            let (cols, bridge) = ring.scan_cols(t_from, dt, closed_len, None);
            cache.dt = dt;
            cache.closed_end = closed_end;
            cache.total_samples = cols.iter().map(|(_, c)| *c as usize).sum();
            cache.cols = cols.into();
            cache.bridge = bridge;
        } else if closed_end > cache.closed_end {
            // Slide: drop k old columns, fold in the k newly closed ones.
            let k = (closed_end - cache.closed_end) as usize;
            for _ in 0..k {
                if let Some((_, c)) = cache.cols.pop_front() {
                    cache.total_samples -= c as usize;
                }
            }
            let scan_from = cache.closed_end as f64 * dt;
            let (new_cols, bridge) = ring.scan_cols(scan_from, dt, k, cache.bridge);
            for (env, c) in new_cols {
                cache.total_samples += c as usize;
                cache.cols.push_back((env, c));
            }
            cache.bridge = bridge;
            cache.closed_end = closed_end;
        }
        // else: same open column as last paint — closed cache is current.

        // The open column recomputes fresh every paint (≤ one column's
        // worth of samples), bridged from the last closed sample.
        let (open, _) = ring.scan_cols(closed_end as f64 * dt, dt, 1, cache.bridge);
        let (open_env, open_count) = open[0];

        let mut strips: Vec<Option<(f64, f64)>> =
            cache.cols.iter().map(|(env, _)| *env).collect();
        strips.push(open_env);
        ChartColumns {
            strips,
            samples: cache.total_samples + open_count as usize,
            t_from: (closed_end - closed_len as i64) as f64 * dt,
            t_to: (closed_end + 1) as f64 * dt,
        }
    }

    /// Get or create the ring for a series.
    pub fn ring(&self, id: SeriesId) -> Arc<Ring> {
        if let Some(r) = self.series.read().expect("store read").get(&id) {
            return Arc::clone(r);
        }
        let mut w = self.series.write().expect("store write");
        Arc::clone(w.entry(id).or_insert_with(|| Arc::new(Ring::new(DEFAULT_RING_CAP))))
    }

    /// Start an ingest thread for `source` → `id` (§14.2). Any previous
    /// ingest for the same series is stopped first.
    pub fn ingest_open(self: &Arc<Self>, id: SeriesId, source: SourceSpec) {
        self.ingest_close(id);
        let stop = Arc::new(AtomicBool::new(false));
        self.stops.lock().expect("stops lock").insert(id, Arc::clone(&stop));
        let ring = self.ring(id);
        std::thread::spawn(move || run_source(&source, &ring, &stop));
    }

    pub fn ingest_close(&self, id: SeriesId) {
        if let Some(stop) = self.stops.lock().expect("stops lock").remove(&id) {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// Feed a tape synchronously — the deterministic test path (§14.4a).
    /// No threads, no clocks: every sample lands before this returns.
    pub fn feed_tape_sync(&self, id: SeriesId, path: &Path) -> Result<usize, String> {
        let ring = self.ring(id);
        let samples = read_tape(path)?;
        let n = samples.len();
        for s in samples {
            ring.push(s);
        }
        Ok(n)
    }
}

// ---------------------------------------------------------------------------
// Sources
// ---------------------------------------------------------------------------

fn run_source(source: &SourceSpec, ring: &Ring, stop: &AtomicBool) {
    match source {
        SourceSpec::Tape { path, speed } => {
            let Ok(samples) = read_tape(path) else { return };
            let start = std::time::Instant::now();
            for s in samples {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                if speed.is_finite() && *speed > 0.0 {
                    let due_ms = s.t_ms / speed;
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    if due_ms > elapsed {
                        std::thread::sleep(std::time::Duration::from_micros(
                            ((due_ms - elapsed) * 1000.0) as u64,
                        ));
                    }
                }
                ring.push(s);
            }
        }
        SourceSpec::Sine { rate_hz, freq_hz, amp, seed } => {
            // Deterministic generator: sample k is a pure function of k.
            let period_us = (1_000_000.0 / rate_hz) as u64;
            let mut k = 0u64;
            let start = std::time::Instant::now();
            while !stop.load(Ordering::Relaxed) {
                let t_ms = k as f64 * 1000.0 / rate_hz;
                ring.push(Sample { t_ms, v: sine_sample(t_ms, *freq_hz, *amp, *seed) });
                k += 1;
                let due = std::time::Duration::from_micros(k * period_us);
                let elapsed = start.elapsed();
                if due > elapsed {
                    std::thread::sleep(due - elapsed);
                }
            }
        }
    }
}

/// Pure sample function for the Sine source — shared with benches so a
/// "10 kHz for 3 s" dataset can also be produced synchronously.
pub fn sine_sample(t_ms: f64, freq_hz: f64, amp: f64, seed: u64) -> f64 {
    let phase = 2.0 * std::f64::consts::PI * freq_hz * (t_ms / 1000.0);
    // Cheap deterministic noise: hash of (seed, t index) → [-0.05, 0.05]·amp
    let k = (t_ms * 10.0) as u64 ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let h = k.wrapping_mul(0xD6E8_FEB8_6659_FD93).rotate_right(29);
    let noise = ((h % 1000) as f64 / 1000.0 - 0.5) * 0.1;
    amp * (phase.sin() + noise)
}

// ---------------------------------------------------------------------------
// Tapes (.ltape) — Open Question 5 resolved for now: line-oriented text,
// `t_ms<TAB>value`, `#` comments. Diffable, greppable, deterministic.
// ---------------------------------------------------------------------------

pub fn read_tape(path: &Path) -> Result<Vec<Sample>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split_whitespace();
        let (Some(t), Some(v)) = (cols.next(), cols.next()) else {
            return Err(format!("{}:{}: expected `t_ms value`", path.display(), i + 1));
        };
        out.push(Sample {
            t_ms: t.parse().map_err(|e| format!("{}:{}: t: {e}", path.display(), i + 1))?,
            v: v.parse().map_err(|e| format!("{}:{}: v: {e}", path.display(), i + 1))?,
        });
    }
    Ok(out)
}

pub fn write_tape(path: &Path, samples: &[Sample]) -> Result<(), String> {
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    writeln!(f, "# libra tape v0: t_ms\tvalue").map_err(|e| e.to_string())?;
    for s in samples {
        writeln!(f, "{}\t{}", s.t_ms, s.v).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// M4 downsampling (§14.3)
// ---------------------------------------------------------------------------

/// M4: per pixel column keep (first, last, min, max) — pixel-identical
/// to plotting every point for line charts, at ≤4·width points.
/// Input must be time-ordered. Output is time-ordered.
pub fn m4_downsample(samples: &[Sample], t_from: f64, t_to: f64, width_px: u32) -> Vec<Sample> {
    if samples.is_empty() || width_px == 0 || t_to <= t_from {
        return vec![];
    }
    if samples.len() <= 4 * width_px as usize {
        return samples.to_vec(); // already under budget: exact
    }
    let col_w = (t_to - t_from) / width_px as f64;
    let mut out: Vec<Sample> = Vec::with_capacity(4 * width_px as usize);
    let mut col_idx = u32::MAX;
    let (mut first, mut last, mut min, mut max) =
        (Sample { t_ms: 0.0, v: 0.0 }, Sample { t_ms: 0.0, v: 0.0 }, Sample { t_ms: 0.0, v: 0.0 }, Sample { t_ms: 0.0, v: 0.0 });

    let flush = |first: Sample, last: Sample, min: Sample, max: Sample, out: &mut Vec<Sample>| {
        // Emit in time order, dropping duplicates.
        let mut four = [first, min, max, last];
        four.sort_by(|a, b| a.t_ms.total_cmp(&b.t_ms));
        for s in four {
            if out.last() != Some(&s) {
                out.push(s);
            }
        }
    };

    for &s in samples {
        if s.t_ms < t_from || s.t_ms > t_to {
            continue;
        }
        let c = (((s.t_ms - t_from) / col_w) as u32).min(width_px - 1);
        if c != col_idx {
            if col_idx != u32::MAX {
                flush(first, last, min, max, &mut out);
            }
            col_idx = c;
            first = s;
            last = s;
            min = s;
            max = s;
        } else {
            last = s;
            if s.v < min.v {
                min = s;
            }
            if s.v > max.v {
                max = s;
            }
        }
    }
    if col_idx != u32::MAX {
        flush(first, last, min, max, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t_ms: f64, v: f64) -> Sample {
        Sample { t_ms, v }
    }

    #[test]
    fn ring_keeps_the_newest_cap_samples_in_order() {
        let r = Ring::new(4);
        for i in 0..6 {
            r.push(s(i as f64, i as f64 * 10.0));
        }
        let (snap, generation) = r.snapshot_from(0.0);
        assert_eq!(generation, 6);
        assert_eq!(snap, vec![s(2.0, 20.0), s(3.0, 30.0), s(4.0, 40.0), s(5.0, 50.0)]);
        assert_eq!(r.latest_t(), Some(5.0));
    }

    #[test]
    fn snapshot_window_filters_by_time() {
        let r = Ring::new(16);
        for i in 0..10 {
            r.push(s(i as f64 * 100.0, i as f64));
        }
        let (snap, _) = r.snapshot_from(700.0);
        assert_eq!(snap.len(), 3); // t = 700, 800, 900
        assert_eq!(snap[0].t_ms, 700.0);
    }

    #[test]
    fn tape_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.ltape");
        let samples = vec![s(0.0, 1.5), s(0.1, -2.25), s(0.2, 0.0)];
        write_tape(&path, &samples).unwrap();
        assert_eq!(read_tape(&path).unwrap(), samples);
    }

    #[test]
    fn feed_tape_sync_is_deterministic_and_threadless() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.ltape");
        write_tape(&path, &[s(0.0, 1.0), s(1.0, 2.0)]).unwrap();
        let store = SeriesStore::new();
        let n = store.feed_tape_sync(SeriesId(1), &path).unwrap();
        assert_eq!(n, 2);
        let (snap, generation) = store.ring(SeriesId(1)).snapshot_from(0.0);
        assert_eq!((snap.len(), generation), (2, 2));
    }

    #[test]
    fn m4_is_exact_when_under_budget() {
        let samples: Vec<Sample> = (0..100).map(|i| s(i as f64, (i % 7) as f64)).collect();
        // 100 samples ≤ 4×400 → passthrough
        assert_eq!(m4_downsample(&samples, 0.0, 100.0, 400), samples);
    }

    #[test]
    fn m4_preserves_extremes_and_bounds_output() {
        // 100k samples with a known global min and max spike.
        let mut samples: Vec<Sample> = (0..100_000)
            .map(|i| s(i as f64, (i as f64 * 0.001).sin()))
            .collect();
        samples[33_333].v = -50.0; // global min
        samples[66_666].v = 50.0; // global max
        let out = m4_downsample(&samples, 0.0, 100_000.0, 800);
        assert!(out.len() <= 4 * 800);
        let vmin = out.iter().map(|p| p.v).fold(f64::INFINITY, f64::min);
        let vmax = out.iter().map(|p| p.v).fold(f64::NEG_INFINITY, f64::max);
        assert_eq!((vmin, vmax), (-50.0, 50.0), "M4 must keep per-column extremes");
        // Output is time-ordered:
        assert!(out.windows(2).all(|w| w[0].t_ms <= w[1].t_ms));
    }

    /// The §14.3 cache contract: sliding incrementally through a stream
    /// yields the same columns as computing cold from the same ring.
    #[test]
    fn chart_columns_incremental_equals_cold_rebuild() {
        let live = SeriesStore::new();
        let id = SeriesId(4);
        let wave = |k: usize| ((k as f64) * 0.037).sin() * 3.0 + ((k % 13) as f64) * 0.1;

        // Stream in 20 chunks, querying between chunks (exercises the
        // slide path with varying k).
        let mut pushed = 0usize;
        for chunk in 0..20 {
            for _ in 0..(150 + chunk * 37) {
                live.ring(id).push(s(pushed as f64 * 0.4, wave(pushed)));
                pushed += 1;
            }
            let _ = live.chart_columns(id, 500.0, 120); // warm/slide the cache
        }
        let warm = live.chart_columns(id, 500.0, 120);

        // Cold store: identical ring contents, single query.
        let cold_store = SeriesStore::new();
        for k in 0..pushed {
            cold_store.ring(id).push(s(k as f64 * 0.4, wave(k)));
        }
        let cold = cold_store.chart_columns(id, 500.0, 120);

        assert_eq!(warm.samples, cold.samples);
        assert_eq!((warm.t_from, warm.t_to), (cold.t_from, cold.t_to));
        assert_eq!(warm.strips, cold.strips, "incremental cache diverged from cold rebuild");
    }

    #[test]
    fn generator_sample_is_a_pure_function() {
        assert_eq!(sine_sample(123.4, 3.0, 1.0, 42), sine_sample(123.4, 3.0, 1.0, 42));
        assert_ne!(sine_sample(123.4, 3.0, 1.0, 42), sine_sample(123.4, 3.0, 1.0, 43));
    }

    #[test]
    fn ingest_open_and_close_manage_a_live_source() {
        let store = Arc::new(SeriesStore::new());
        store.ingest_open(
            SeriesId(9),
            SourceSpec::Sine { rate_hz: 10_000.0, freq_hz: 5.0, amp: 1.0, seed: 1 },
        );
        // Wait until samples demonstrably flow.
        let ring = store.ring(SeriesId(9));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while ring.generation() < 100 {
            assert!(std::time::Instant::now() < deadline, "ingest never produced samples");
            std::thread::yield_now();
        }
        store.ingest_close(SeriesId(9));
        let g1 = ring.generation();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let g2 = ring.generation();
        // Writer may land a final in-flight sample after the stop flag,
        // but the flow must cease.
        assert!(g2 - g1 <= 2, "ingest kept writing after close ({g1} → {g2})");
    }
}
