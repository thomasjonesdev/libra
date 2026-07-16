//! Windowed: `cargo run -p dashboard --release`
//! Benchmark: `cargo run -p dashboard --release -- bench`
//!
//! The bench is the §22 budget check, headless and synchronous: rings
//! are fed deterministically (no ingest threads, no clocks in the
//! pipeline), then 600 frames run through the real
//! view→layout→paint→damage-render path at 1280×800.

use dashboard::{Action, Dashboard, RATE_HZ, SERIES, VIEW, WINDOW_MS};
use libra_core::{FrameInfo, ReflectAction, SeriesId};
use libra_data::{sine_sample, Sample, SeriesStore};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("bench") {
        bench();
        return;
    }
    if let Err(e) = dashboard::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn bench() {
    let (w, h) = (1280u32, 800u32);
    let frames = 600usize;
    let samples_per_frame = (RATE_HZ / 60.0) as usize; // 10 kHz at 60 fps

    let store = Arc::new(SeriesStore::new());
    let (state, _fx) = Dashboard::init();
    let ast = libra_markup::parse(VIEW).expect("view parses");
    let renderer = libra_raster::Renderer::new();
    let theme = libra_draw::Theme::dark();

    // Prefill: one full window of history per series, synchronously.
    let prefill = (RATE_HZ * WINDOW_MS / 1000.0) as usize;
    for i in 0..SERIES {
        let (ring, spec) = (store.ring(SeriesId(i)), dashboard::source(i));
        let libra_core::SourceSpec::Sine { freq_hz, amp, seed, .. } = spec else { unreachable!() };
        for k in 0..prefill {
            let t_ms = k as f64 * 1000.0 / RATE_HZ;
            ring.push(Sample { t_ms, v: sine_sample(t_ms, freq_hz, amp, seed) });
        }
    }

    let field = |p: &str| libra_core::ReflectState::field(&state, p);
    let has_action = |m: &str| Action::action_by_name(m).is_some();
    let has_str_action = |m: &str| Action::action_with_str(m, String::new()).is_some();
    let collection = |p: &str| libra_core::ReflectState::collection(&state, p);
    let has_frame_info_action = |m: &str| Action::action_with_frame_info(m, FrameInfo::at(0.0)).is_some();
    let scope = libra_interp::Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection: &collection,
        has_frame_info_action: &has_frame_info_action,
    };

    let mut cache = None;
    let mut layout_ms = Vec::with_capacity(frames);
    let mut paint_ms = Vec::with_capacity(frames);
    let mut raster_ms = Vec::with_capacity(frames);
    let mut total_ms = Vec::with_capacity(frames);
    let mut full_frames = 0usize;

    for f in 0..frames {
        // 1/60s of new data per series, deterministic, synchronous.
        for i in 0..SERIES {
            let ring = store.ring(SeriesId(i));
            let libra_core::SourceSpec::Sine { freq_hz, amp, seed, .. } = dashboard::source(i)
            else {
                unreachable!()
            };
            let base = prefill + f * samples_per_frame;
            for k in 0..samples_per_frame {
                let t_ms = (base + k) as f64 * 1000.0 / RATE_HZ;
                ring.push(Sample { t_ms, v: sine_sample(t_ms, freq_hz, amp, seed) });
            }
        }

        let t0 = Instant::now();
        let view = libra_interp::resolve(&ast, &scope).expect("resolves");
        let laid = libra_components::layout(&view, (w as f32, h as f32), &renderer, &theme).expect("lays out");
        let t1 = Instant::now();
        let cmds = laid.paint(&renderer, &store, None, &theme);
        let t2 = Instant::now();
        let damage = renderer.render_damage(&mut cache, cmds, w, h);
        let t3 = Instant::now();

        if matches!(damage, libra_raster::Damage::Full) {
            full_frames += 1;
        }
        layout_ms.push(ms(t1 - t0));
        paint_ms.push(ms(t2 - t1));
        raster_ms.push(ms(t3 - t2));
        total_ms.push(ms(t3 - t0));
    }

    println!("libra dashboard bench — {frames} frames, {SERIES} series × {RATE_HZ} Hz, {w}×{h}");
    println!("  full repaints: {full_frames} (first frame expected)");
    report("view+layout", &mut layout_ms);
    report("paint (M4 + geometry)", &mut paint_ms);
    report("raster (damage-tracked)", &mut raster_ms);
    report("frame total", &mut total_ms);
    println!("  §22 budgets: damage-tracked dashboard frame < 2 ms; full repaint 1440p < 8 ms");
}

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn report(label: &str, xs: &mut [f64]) {
    xs.sort_by(f64::total_cmp);
    let p = |q: f64| xs[((xs.len() - 1) as f64 * q) as usize];
    println!(
        "  {label:<24} p50 {:>7.3} ms   p95 {:>7.3} ms   max {:>7.3} ms",
        p(0.5),
        p(0.95),
        xs[xs.len() - 1]
    );
}
