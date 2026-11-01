// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The platform seam of the window frame: the one place that decides how the native frame takes
//! the P4KX colours.
//!
//! Windows tints the frame through the Desktop Window Manager ([`crate::dwm`]). Every other
//! platform keeps the frame its window manager draws; the P4KX theme reaches the client area
//! through [`crate::theme::apply`] alone.

use eframe::CreationContext;

/// Tints the native frame of the window created with `creation`.
///
/// A window manager that draws its own frame is left alone, so the call is a no-op there.
#[cfg(windows)]
pub fn tint(creation: &CreationContext<'_>) {
    crate::dwm::tint(creation);
}

/// Tints the native frame of the window created with `creation`.
///
/// A window manager that draws its own frame is left alone, so the call is a no-op there.
#[cfg(not(windows))]
pub const fn tint(_creation: &CreationContext<'_>) {}
