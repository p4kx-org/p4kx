// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The metrics of the smallest window, taken from the `sidebar` and `workSurface` entries of
//! `docs/design/p4kx-desktop-spec.json`.

use super::desktop::{
    BUTTON_HEIGHT, BUTTON_XS_HEIGHT, PREVIEW_HEIGHT, ROW_HEIGHT, SCROLLBAR, SEGMENT_HEIGHT,
    SIDEBAR_WIDTH, TOOLBAR_PADDING,
};
use super::tokens::{SPACE_1, SPACE_2};

/// The width of the smallest window the main window can be resized to.
pub const WINDOW_MIN_WIDTH: f32 = 640.0;
/// The height of the smallest window the main window can be resized to.
pub const WINDOW_MIN_HEIGHT: f32 = 400.0;
/// The width of the rail that stays of a collapsed sidebar: a compact button between two
/// `space-1` margins.
pub const SIDEBAR_RAIL_WIDTH: f32 = 2.0f32.mul_add(SPACE_1, BUTTON_XS_HEIGHT);
/// The width the room of a work tab below the tab bar keeps; a narrower window scrolls it.
///
/// The smallest window less the rail, the scrollbar a low window shows and the `space-2` gutter
/// on each side of the window.
pub const WORK_MIN_WIDTH: f32 =
    2.0f32.mul_add(-SPACE_2, WINDOW_MIN_WIDTH - SIDEBAR_RAIL_WIDTH - SCROLLBAR);
/// The height the Entry list of the Browse tab keeps above the preview.
///
/// Its toolbar with the search row, the actions in one row and the quick filters, the table head
/// and one row, and the two hairlines of its frame.
pub const BROWSE_LIST_MIN_HEIGHT: f32 = 2.0f32.mul_add(
    ROW_HEIGHT,
    2.0f32.mul_add(BUTTON_HEIGHT, 4.0 * TOOLBAR_PADDING) + SEGMENT_HEIGHT,
) + 2.0;
/// The height a second row of actions adds to a toolbar, reserved only while the actions wrap.
pub const WRAPPED_ACTIONS_HEIGHT: f32 = TOOLBAR_PADDING + BUTTON_HEIGHT;
/// The height the room of a work tab below the tab bar keeps; a lower window scrolls it.
///
/// The Entry list of the Browse tab and the preview below it.
pub const WORK_MIN_HEIGHT: f32 = BROWSE_LIST_MIN_HEIGHT + PREVIEW_HEIGHT;
/// The window width below which the sidebar hides behind its rail until the rail button shows it:
/// the sidebar and the smallest room of a work tab no longer fit side by side.
pub const SIDEBAR_COLLAPSE_WIDTH: f32 = SIDEBAR_WIDTH + WORK_MIN_WIDTH;
/// The height the toolbar of the Sounds tab keeps before it scrolls.
///
/// The Entity row, two rows of actions and the search row.
pub const SOUNDS_TOOLBAR_MIN_HEIGHT: f32 = 3.0f32.mul_add(TOOLBAR_PADDING, 4.0 * BUTTON_HEIGHT);
