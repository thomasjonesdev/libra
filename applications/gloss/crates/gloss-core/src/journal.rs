use serde::{Deserialize, Serialize};

use crate::patch::{Op, PatchId, RunId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run: RunId,
    pub patch: PatchId,
    pub inverse: Vec<Op>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Journal {
    pub runs: Vec<RunRecord>,
}

impl Journal {
    pub fn record(&mut self, run: RunId, patch: PatchId, inverse: Vec<Op>) {
        self.runs.push(RunRecord {
            run,
            patch,
            inverse,
        });
    }

    pub fn inverse_for(&self, run: RunId) -> Option<&[Op]> {
        self.runs
            .iter()
            .rev()
            .find(|r| r.run == run)
            .map(|r| r.inverse.as_slice())
    }

    pub fn last_run(&self) -> Option<RunId> {
        self.runs.last().map(|r| r.run.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_run_wins_lookup() {
        let mut j = Journal::default();
        j.record(RunId("a".into()), PatchId("p1".into()), vec![]);
        j.record(RunId("b".into()), PatchId("p2".into()), vec![]);
        assert_eq!(j.last_run(), Some(RunId("b".into())));
        assert!(j.inverse_for(RunId("a".into())).is_some());
    }
}
