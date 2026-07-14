//! The Phase 2 benchmark app (§22): 16 series × 10 kHz through the data
//! plane, a 4×4 chart wall, one `frame` action per tick. State holds
//! handles and a status string — samples never touch it (§14.2).
//!
//! §6.2's collection-binding proof: the sixteen `s0`..`s15` fields
//! collapse to four `Vec<SeriesHandle>` rows, each bound through
//! `<for each={rowN} as="s" key={s}>` — one row field per grid row,
//! since `<for>` iterates a flat field and the 4×4 layout (not a
//! flat 1×16 row) is part of this app's snapshot-tested output. The
//! ingest/toggle logic doesn't touch these fields at all — it only
//! ever needs "all sixteen series ids", which is just `0..SERIES`.
//!
//! `subscriptions` stays hand-written (`libra::app!`'s `subscriptions:`
//! field) rather than a §12.5 `<Frames>` source element: pause/resume
//! needs *conditional* mounting ("only tick while running"), and the
//! grammar has no `<if>` yet (§26) — an unconditionally-mounted
//! `<Frames>` would tick at full rate even while paused and just
//! discard the result, which defeats the point for a throughput
//! benchmark specifically.

use libra::{actions, app, state};
use libra_core::{Effect, FrameInfo, SeriesHandle, SeriesId, SourceSpec, Sub};

pub const SERIES: u32 = 16;
pub const ROW_LEN: u32 = 4;
pub const RATE_HZ: f64 = 10_000.0;
pub const WINDOW_MS: f64 = 3_000.0;

#[state]
pub struct Dashboard {
    pub status: String,
    #[libra(skip)]
    pub running: bool,
    pub row0: Vec<SeriesHandle>,
    pub row1: Vec<SeriesHandle>,
    pub row2: Vec<SeriesHandle>,
    pub row3: Vec<SeriesHandle>,
}

/// The generator spec for series `i` — shared with the bench and tests
/// so every consumer sees identical data.
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
            status: "starting…".into(),
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
            self.status = "resuming…".into();
            // Ingest threads were closed on pause; reopen.
            (0..SERIES).map(|i| Effect::IngestOpen { source: source(i), series: SeriesId(i) }).collect()
        } else {
            self.status = "paused".into();
            (0..SERIES).map(|i| Effect::IngestClose { series: SeriesId(i) }).collect()
        }
    }

    /// All the time the app knows arrives in the action (§14.2).
    pub fn frame(&mut self, info: FrameInfo) {
        self.status = format!("live · t={:.1}s · frame {}", info.t_ms / 1000.0, info.index);
    }
}

/// Pure: "tick while running" is a testable function (§12.2).
pub fn subscriptions(state: &Dashboard) -> Vec<Sub<Action>> {
    if state.running {
        vec![Sub::frames(Action::Frame)]
    } else {
        vec![]
    }
}

app! {
    state: Dashboard,
    view: "../views/dashboard.lx",
    init: Dashboard::init,
    subscriptions: subscriptions,
    window: (1280.0, 640.0),
    title: "Libra Dashboard",
}
