// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The smoke run: open the window, draw a few frames and exit, to check on a machine without a
//! user that the window starts and the graphics stack draws.

use std::ffi::OsStr;

/// The environment variable that turns the run into a smoke run; its value is the number of
/// frames to draw.
pub const VARIABLE: &str = "P4KX_SMOKE_FRAMES";

/// The frames of a smoke run still to draw.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frames {
    left: u32,
}

impl Frames {
    /// Returns the frames named by `value`, or `None` when it is unset, empty, zero or not a
    /// number.
    #[must_use]
    pub fn from_value(value: Option<&OsStr>) -> Option<Self> {
        let count: u32 = value?.to_str()?.trim().parse().ok()?;
        (count > 0).then_some(Self { left: count })
    }

    /// Returns the frames named by [`VARIABLE`].
    #[must_use]
    pub fn from_environment() -> Option<Self> {
        Self::from_value(std::env::var_os(VARIABLE).as_deref())
    }

    /// Counts one drawn frame and returns whether the run is over.
    pub const fn drawn(&mut self) -> bool {
        self.left = self.left.saturating_sub(1);
        self.left == 0
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn report_native_scale(ctx: &egui::Context) {
    use std::io::Write as _;

    let Some(scale) = ctx.input(|input| input.viewport().native_pixels_per_point) else {
        return;
    };
    if let Err(error) = writeln!(std::io::stdout().lock(), "P4KX_GUI_NATIVE_SCALE={scale}") {
        log::error!("cannot report GUI smoke scale: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::Frames;

    #[test]
    fn a_count_names_the_frames_to_draw() {
        let mut frames = Frames::from_value(Some(OsStr::new("3"))).expect("a count");

        assert!(!frames.drawn(), "first frame");
        assert!(!frames.drawn(), "second frame");
        assert!(frames.drawn(), "third frame ends the run");
    }

    #[test]
    fn a_missing_zero_or_malformed_value_is_no_smoke_run() {
        for value in [None, Some(""), Some("0"), Some("-1"), Some("many")] {
            assert_eq!(
                Frames::from_value(value.map(OsStr::new)),
                None,
                "{value:?} is no smoke run"
            );
        }
    }

    #[test]
    fn a_finished_run_stays_finished() {
        let mut frames = Frames::from_value(Some(OsStr::new("1"))).expect("a count");

        assert!(frames.drawn(), "ends");
        assert!(frames.drawn(), "still ended");
    }
}
