// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The tint of the native Windows frame through the Desktop Window Manager.

#![expect(unsafe_code, reason = "DwmSetWindowAttribute is a foreign function")]

use std::ffi::c_void;

use egui::Color32;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWINDOWATTRIBUTE, DwmSetWindowAttribute,
};

use crate::theme::palette;

/// Tints the frame of `window` in the P4KX colours and switches it to dark mode.
///
/// The frame keeps snap layouts, shadow and rounded corners. A window without a Win32 handle,
/// or a Windows version without one of the attributes, keeps the default for it.
pub fn tint(window: &impl HasWindowHandle) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = win32.hwnd.get() as HWND;
    set(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, 1);
    set(hwnd, DWMWA_CAPTION_COLOR, colorref(palette::SURFACE_BAR));
    set(hwnd, DWMWA_TEXT_COLOR, colorref(palette::GRAY_1));
    set(hwnd, DWMWA_BORDER_COLOR, colorref(palette::GRAY_3));
}

/// Sets the 32-bit DWM `attribute` of `hwnd` to `value`, ignoring a refusal.
///
/// # Safety
///
/// The call reads exactly the four bytes of the local `value`, whose address and size it passes.
/// `hwnd` comes from a borrowed window handle that is valid while [`tint`] runs, and DWM rejects
/// a handle that is no window with an error instead of touching it.
#[expect(
    clippy::undocumented_unsafe_blocks,
    reason = "the Safety section of this function documents its one unsafe block"
)]
fn set(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: u32) {
    let Ok(size) = u32::try_from(size_of_val(&value)) else {
        return;
    };
    let _ignored = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute.cast_unsigned(),
            (&raw const value).cast::<c_void>(),
            size,
        )
    };
}

/// Returns `color` as a Win32 `COLORREF`, `0x00BBGGRR`.
const fn colorref(color: Color32) -> u32 {
    u32::from_le_bytes([color.r(), color.g(), color.b(), 0])
}

#[cfg(test)]
mod tests {
    use egui::Color32;

    use super::colorref;

    #[test]
    fn a_colorref_holds_blue_green_red_from_the_high_byte_down() {
        let orange = Color32::from_rgb(0xE7, 0x7E, 0x23);

        assert_eq!(colorref(orange), 0x0023_7EE7, "0x00BBGGRR");
    }
}
