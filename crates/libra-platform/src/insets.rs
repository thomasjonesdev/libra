//! Desktop safe-area insets (PLAN-NATIVE-INTEGRATIONS N5).
//!
//! winit does not report a desktop safe area. A frameless macOS window
//! draws under the title bar, so the shell publishes a fixed logical
//! top inset. Every other host stays at zero. The number is a constant,
//! not a read of the running OS, so the same inputs lay out the same way.

use libra_components::Insets;

/// Standard macOS title-bar band, in logical px.
pub const MACOS_TITLEBAR: f32 = 28.0;

pub fn desktop_insets(frameless: bool, fullscreen: bool) -> Insets {
    if cfg!(target_os = "macos") && frameless && !fullscreen {
        Insets { top: MACOS_TITLEBAR, ..Insets::ZERO }
    } else {
        Insets::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_windowed_frameless_mac_window_insets_the_title_bar() {
        let framed = desktop_insets(false, false);
        assert_eq!(framed, Insets::ZERO);
        let full = desktop_insets(true, true);
        assert_eq!(full, Insets::ZERO);
        let frameless = desktop_insets(true, false);
        if cfg!(target_os = "macos") {
            assert_eq!(frameless.top, MACOS_TITLEBAR);
            assert_eq!(frameless.bottom, 0.0);
        } else {
            assert_eq!(frameless, Insets::ZERO);
        }
    }
}
