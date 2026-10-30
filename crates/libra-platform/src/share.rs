//! `Effect::Share` (PLAN-NATIVE-INTEGRATIONS N4, §12.1).
//!
//! The payload is built in `libra-core` (`share_payload`). This module
//! only writes it to the clipboard. Tests do not call [`copy`].

use libra_core::ShareResult;

pub fn copy(payload: &str) -> ShareResult {
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(payload.to_string())) {
        Ok(()) => ShareResult::Copied,
        Err(e) => ShareResult::Failed(e.to_string()),
    }
}
