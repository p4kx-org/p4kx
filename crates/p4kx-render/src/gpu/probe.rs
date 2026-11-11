// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::{Deref, DerefMut};

use p4kx_testkit::watchdog::{Guard, arm};

use super::{Adapters, Frame, Offscreen, OffscreenError, Picture};

#[cfg(target_os = "macos")]
pub(super) const TEST_ADAPTERS: Adapters = Adapters::Metal;
#[cfg(not(target_os = "macos"))]
pub(super) const TEST_ADAPTERS: Adapters = Adapters::Software;

/// An [`Offscreen`] opened by a watched test: the watchdog names the test and the phase it
/// hangs in, and the phase is `teardown` while the device is dropped.
pub(super) struct Probe {
    offscreen: Offscreen,
    guard: Guard,
}

impl Probe {
    /// Draws `frame` and reads the picture back, as [`Offscreen::draw`] does.
    pub(super) fn draw(&mut self, frame: &Frame) -> Result<Picture, OffscreenError> {
        self.guard.phase("draw and readback");
        let picture = self.offscreen.draw(frame);
        self.guard.phase("check");
        picture
    }
}

impl Deref for Probe {
    type Target = Offscreen;

    fn deref(&self) -> &Offscreen {
        &self.offscreen
    }
}

impl DerefMut for Probe {
    fn deref_mut(&mut self) -> &mut Offscreen {
        &mut self.offscreen
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.guard.phase("teardown");
    }
}

/// Arms `test` and opens an offscreen renderer on `adapters`. Returns `None` with a printed
/// reason when there is none and the test run permits an unavailable adapter.
#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
pub(super) fn open(test: &str, adapters: Adapters) -> Option<Probe> {
    let guard = arm(test);
    guard.phase("adapter and device");
    match Offscreen::new(adapters) {
        Ok(offscreen) => {
            guard.phase("check");
            Some(Probe { offscreen, guard })
        }
        Err(error) => {
            assert!(
                !p4kx_testkit::render_adapter_required(),
                "{test}: a graphics adapter is required: {error}"
            );
            eprintln!("{test}: no graphics adapter ({error}), nothing checked");
            None
        }
    }
}
