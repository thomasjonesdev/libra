//! The Phase 2 benchmark app (§22): 16 series × 10 kHz through the data
//! plane, a 4×4 chart wall, one `Frame` msg per tick. The Model holds
//! handles and a status string — samples never touch it (§14.2).

use libra_core::{App, Cmd, FrameInfo, LibraReflect, SeriesHandle, SeriesId, SourceSpec, Sub};

pub const VIEW: &str = include_str!("../views/dashboard.lx");
pub const SERIES: u32 = 16;
pub const RATE_HZ: f64 = 10_000.0;
pub const WINDOW_MS: f64 = 3_000.0;

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct Dashboard {
    pub status: String,
    #[libra(skip)]
    pub running: bool,
    pub s0: SeriesHandle,
    pub s1: SeriesHandle,
    pub s2: SeriesHandle,
    pub s3: SeriesHandle,
    pub s4: SeriesHandle,
    pub s5: SeriesHandle,
    pub s6: SeriesHandle,
    pub s7: SeriesHandle,
    pub s8: SeriesHandle,
    pub s9: SeriesHandle,
    pub s10: SeriesHandle,
    pub s11: SeriesHandle,
    pub s12: SeriesHandle,
    pub s13: SeriesHandle,
    pub s14: SeriesHandle,
    pub s15: SeriesHandle,
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub enum Msg {
    Toggle,
    Frame(FrameInfo),
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

impl App for Dashboard {
    type Msg = Msg;

    fn init() -> (Self, Vec<Cmd<Msg>>) {
        let h = |i: u32| SeriesHandle::new(SeriesId(i), WINDOW_MS);
        let model = Dashboard {
            status: "starting…".into(),
            running: true,
            s0: h(0),
            s1: h(1),
            s2: h(2),
            s3: h(3),
            s4: h(4),
            s5: h(5),
            s6: h(6),
            s7: h(7),
            s8: h(8),
            s9: h(9),
            s10: h(10),
            s11: h(11),
            s12: h(12),
            s13: h(13),
            s14: h(14),
            s15: h(15),
        };
        let cmds = (0..SERIES)
            .map(|i| Cmd::IngestOpen { source: source(i), series: SeriesId(i) })
            .collect();
        (model, cmds)
    }

    fn update(&mut self, msg: Msg) -> Vec<Cmd<Msg>> {
        match msg {
            Msg::Toggle => {
                self.running = !self.running;
                if self.running {
                    self.status = "resuming…".into();
                    // Ingest threads were closed on pause; reopen.
                    (0..SERIES)
                        .map(|i| Cmd::IngestOpen { source: source(i), series: SeriesId(i) })
                        .collect()
                } else {
                    self.status = "paused".into();
                    (0..SERIES).map(|i| Cmd::IngestClose { series: SeriesId(i) }).collect()
                }
            }
            Msg::Frame(info) => {
                // All the time the app knows arrives in the msg (§14.2).
                self.status = format!("live · t={:.1}s · frame {}", info.t_ms / 1000.0, info.index);
                vec![]
            }
        }
    }

    /// Pure: "tick while running" is a testable function (§12.2).
    fn subscriptions(&self) -> Vec<Sub<Msg>> {
        if self.running {
            vec![Sub::frames(Msg::Frame)]
        } else {
            vec![]
        }
    }
}
