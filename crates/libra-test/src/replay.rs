//! `.lreplay` sessions (§15.6). A recording is the action names that
//! ran, how many intercepted effects to discard after each one, then
//! the final state hash and the final frame hash. `Harness::replay`
//! drives a fresh harness through that list and checks both hashes.
//!
//! Effect *results* (HTTP bodies, dialog answers) and tapes are still
//! open. A step that leaves effects queued fails until `discard` names
//! how many to drop.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One committed session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    #[serde(default, rename = "preludeDiscard")]
    pub prelude_discard: usize,
    pub steps: Vec<ReplayStep>,
    #[serde(rename = "stateHash")]
    pub state_hash: String,
    /// blake3 of the canonical PNG (density 1, the snapshot frame).
    pub frame: String,
}

/// One action, then an optional number of intercepted effects to drop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayStep {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub discard: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Replay {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("replay json")
    }
}
