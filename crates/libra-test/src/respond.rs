//! Auto-resolving intercepted effects (§15.4 + §12.3).
//!
//! Hand-feeding results one at a time is right when a test is *about* a
//! specific flow. It does not scale to a run that takes hundreds of steps and
//! cannot know in advance which effects each one emits. A [`Responder`] closes
//! that gap: the test says once how each *kind* of effect answers, and the
//! harness drains the queue on demand.
//!
//! The results are still fixtures — nothing executes (§12.6). What the drain
//! adds is **order**: pending effects resolve in a seeded shuffle rather than
//! emission order, because §12.3 makes cross-effect ordering deliberately
//! unguaranteed and state is required to tolerate it. Emission order is the
//! one order a real runtime will not promise you, so it is the worst one to
//! test exclusively.

use libra_core::{
    ClipboardResult, Effect, FolderPick, HttpResult, KvResult, NotifyResult, ProcessResult,
};

/// A test's answer for one intercepted effect.
///
/// Every variant except [`Reply::Defer`] and [`Reply::Drop`] carries the same
/// fixture type the matching `resolve_*` verb takes, so a responder is just
/// those verbs expressed as data.
#[derive(Debug, Clone)]
pub enum Reply {
    /// Deliver nothing and discard the effect — a fire-and-forget write, or a
    /// kind this run does not model.
    Drop,
    /// Leave it queued and offer it again on the next round. Models an effect
    /// still in flight, and is what an unhandled kind should return so it
    /// shows up in the storm report rather than vanishing.
    Defer,
    Http(HttpResult),
    KvGet(Option<Vec<u8>>),
    KvSet(KvResult),
    KvScan(Vec<(String, Vec<u8>)>),
    Random(Vec<u8>),
    Process(ProcessResult),
    Clipboard(ClipboardResult),
    FolderPick(FolderPick),
    Notify(NotifyResult),
    /// Fire a timer's action now — tests own the clock (§15.4).
    Timer,
}

/// How a test answers effects during an automatic drain.
pub type Responder<M> = Box<dyn FnMut(&Effect<M>) -> Reply>;

/// A drain that needs more rounds than this is not converging. Real chains are
/// short (a load reads KV, lists, opens, then settles); a four-figure count
/// means an effect cycle, which is a bug worth failing on rather than hanging.
pub(crate) const MAX_ROUNDS: usize = 100;

/// Deterministic PRNG (SplitMix64).
///
/// Seeded and self-contained so a failing autopilot run reproduces exactly
/// from its seed — an autopilot you cannot replay is a flake generator, not a
/// test.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-enough index below `n`. Panics on `n == 0`.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub(crate) fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = self.below(i + 1);
            xs.swap(i, j);
        }
    }
}

/// Short label for storm/failure reports — `Debug` on an `Effect` prints its
/// payload, which is unreadable a hundred at a time.
pub(crate) fn label<M>(effect: &Effect<M>) -> String {
    match effect {
        Effect::Http { req, .. } => format!("Http({} {})", req.method, req.url),
        Effect::KvGet { key, .. } => format!("KvGet({key})"),
        Effect::KvSet { key, .. } => format!("KvSet({key})"),
        Effect::KvDelete { key } => format!("KvDelete({key})"),
        Effect::KvScan { prefix, .. } => format!("KvScan({prefix})"),
        Effect::Timer { after_ms, .. } => format!("Timer({after_ms}ms)"),
        Effect::CancelTimer { .. } => "CancelTimer".into(),
        Effect::Random { bytes, .. } => format!("Random({bytes})"),
        Effect::IngestOpen { series, .. } => format!("IngestOpen({series:?})"),
        Effect::IngestClose { series } => format!("IngestClose({series:?})"),
        Effect::SetTheme { name } => format!("SetTheme({name})"),
        Effect::ClipboardSet { .. } => "ClipboardSet".into(),
        Effect::PickFolder { .. } => "PickFolder".into(),
        Effect::Notify { note, .. } => format!("Notify({})", note.title),
        Effect::WindowOp { op } => format!("WindowOp({op:?})"),
        Effect::LoginItem { enabled, .. } => format!("LoginItem({enabled})"),
        Effect::LoginItemQuery { .. } => "LoginItemQuery".into(),
        Effect::Process { command, .. } => {
            format!("Process({} {})", command.program, command.args.join(" "))
        }
        Effect::CancelProcess { .. } => "CancelProcess".into(),
        Effect::Spawn {
            stream, command, ..
        } => {
            format!(
                "Spawn({stream} {} {})",
                command.program,
                command.args.join(" ")
            )
        }
        Effect::CloseStream { stream } => format!("CloseStream({stream})"),
        // `Effect` is `non_exhaustive` (PLAN-SDK S4).
        _ => "Effect".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn a_seed_reproduces_its_sequence() {
        let a: Vec<u64> = (0..16).map(|_| Rng::new(42).next_u64()).collect();
        let mut rng = Rng::new(42);
        let b: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
        assert_eq!(a[0], b[0]);
        let mut again = Rng::new(42);
        let c: Vec<u64> = (0..16).map(|_| again.next_u64()).collect();
        assert_eq!(b, c, "same seed must replay the same sequence");
        assert!(b.windows(2).all(|w| w[0] != w[1]), "not a constant stream");
    }

    #[test]
    fn shuffle_permutes_without_losing_elements() {
        let mut rng = Rng::new(9);
        let mut xs: Vec<u32> = (0..64).collect();
        rng.shuffle(&mut xs);
        assert_ne!(xs, (0..64).collect::<Vec<_>>(), "nothing moved");
        xs.sort_unstable();
        assert_eq!(xs, (0..64).collect::<Vec<_>>(), "elements were lost");
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::new(5);
        for n in 1..40usize {
            for _ in 0..50 {
                assert!(rng.below(n) < n);
            }
        }
    }
}
