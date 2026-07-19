//! The Phase 2 benchmark app: 16 series × 10 kHz through the data plane, a 4×4
//! chart wall. State holds handles only — samples never touch it. The sixteen
//! series collapse to four `Vec<SeriesHandle>` rows bound through `<for>`.
//!
//! The charts **auto-run**: while ingest is open the data plane drives the
//! shell's display loop (§12.5, `SeriesStore::has_live_ingest`), so there is no
//! `<Frames>` pump. Pause is just `IngestClose` — the loop idles and the charts
//! freeze on their last frame; resume reopens ingest.
//!
//! Architecture: ARCHITECTURE.md §22 (performance budgets), §14.2 (data-plane
//! architecture), §6.2 (the closed grammar), §12.5 (a live data plane drives
//! the display loop).

use libra::{actions, app, state};
use libra_core::{Effect, SeriesHandle, SeriesId, SourceSpec};

const LIVE_STATUS: &str = "live · 16 series @ 10 kHz";

pub const SERIES: u32 = 16;
pub const ROW_LEN: u32 = 4;
pub const RATE_HZ: f64 = 10_000.0;
pub const WINDOW_MS: f64 = 3_000.0;

#[state]
pub struct Dashboard {
    pub status: String,
    pub running: bool,
    pub row0: Vec<SeriesHandle>,
    pub row1: Vec<SeriesHandle>,
    pub row2: Vec<SeriesHandle>,
    pub row3: Vec<SeriesHandle>,
}

pub fn source(i: u32) -> SourceSpec {
    SourceSpec::Sine {
        rate_hz: RATE_HZ,
        freq_hz: 0.5 + i as f64 * 0.35,
        amp: 1.0 + (i % 4) as f64,
        seed: 1000 + i as u64,
    }
}

fn row(start: u32) -> Vec<SeriesHandle> {
    (start..start + ROW_LEN).map(|i| SeriesHandle::new(SeriesId(i), WINDOW_MS)).collect()
}

impl Dashboard {
    pub fn init() -> (Dashboard, Vec<Effect<Action>>) {
        let state = Dashboard {
            status: LIVE_STATUS.into(),
            running: true,
            row0: row(0),
            row1: row(ROW_LEN),
            row2: row(2 * ROW_LEN),
            row3: row(3 * ROW_LEN),
        };
        let fx = (0..SERIES)
            .map(|i| Effect::IngestOpen { source: source(i), series: SeriesId(i) })
            .collect();
        (state, fx)
    }
}

#[actions]
impl Dashboard {
    pub fn toggle(&mut self) -> Vec<Effect<Action>> {
        self.running = !self.running;
        if self.running {
            self.status = LIVE_STATUS.into();
            // Ingest threads were closed on pause; reopen — the reopened
            // sources re-arm the display loop (§12.5), so the charts resume.
            (0..SERIES).map(|i| Effect::IngestOpen { source: source(i), series: SeriesId(i) }).collect()
        } else {
            self.status = "paused".into();
            // Closing every source idles the display loop; the last frame
            // stays on screen.
            (0..SERIES).map(|i| Effect::IngestClose { series: SeriesId(i) }).collect()
        }
    }
}

app! {
    state: Dashboard,
    view: "../views/dashboard.lx",
    init: Dashboard::init,
    window: (1280.0, 640.0),
    title: "Libra Dashboard",
}
