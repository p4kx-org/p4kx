// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The metrics of the WEM list of a soundbank in the Audio tab, taken from the `soundbankList`
//! entry of `docs/design/p4kx-desktop-spec.json`.

use super::desktop::{BUTTON_XS_HEIGHT, PREVIEW_PATH_HEIGHT, ROW_HEIGHT};
use super::tokens::TOUCH_TARGET;

/// The rows of the list that stay visible while a WEM is selected.
pub const SOUNDBANK_VISIBLE_ROWS: f32 = 4.0;
/// The height of the facts bar, about 32.
pub(super) const FACTS_HEIGHT: f32 = 32.0;
/// The vertical padding of the preview body around the controls, twice 8.
const CONTROLS_PADDING: f32 = 16.0;
/// What the panel adds around its content: the resize gap, the frame padding and the rules,
/// measured in a render of the list.
pub(super) const PANEL_CHROME: f32 = 37.0;
/// The default height of the preview while it lists the WEMs of a soundbank: the tab bar, the path
/// line, the facts bar, the controls, the head and [`SOUNDBANK_VISIBLE_ROWS`] rows.
pub const PREVIEW_SOUNDBANK_HEIGHT: f32 = ROW_HEIGHT.mul_add(
    1.0 + SOUNDBANK_VISIBLE_ROWS,
    TOUCH_TARGET + PREVIEW_PATH_HEIGHT + FACTS_HEIGHT + CONTROLS_PADDING + BUTTON_XS_HEIGHT,
) + PANEL_CHROME;
/// The width of the ID column of the WEM list: the widest ID of ten digits in bold at the row
/// font, 105.4, and the cell padding on both sides, rounded up.
pub const SOUND_ID_COLUMN: f32 = 106.0;
/// The initial width of the codec column: the widest codec name, "Wwise Vorbis", or mark in bold
/// at the row font, 110.6 with the cell padding, rounded up.
pub const SOUND_CODEC_COLUMN: f32 = 112.0;
/// The initial width of the duration column: the widest time, "9:59:59.999", in bold at the row
/// font, 99.9 with the cell padding, rounded up.
pub const SOUND_DURATION_COLUMN: f32 = 100.0;
/// The narrowest a resized column of the list gets.
pub const SOUND_COLUMN_MIN: f32 = 80.0;
/// The initial width of the name column of the trigger and sound tables of the Sounds tab.
///
/// A name of forty characters, `Play_SC_Ship_Weapon_Ballistic_Fire_Heavy`, in bold at the row
/// font, 293.0 with the cell padding on both sides, rounded up. Longer names clip; the column is
/// resizable.
pub const SOUND_NAME_COLUMN: f32 = 294.0;
/// The initial width of the status column of the trigger table.
///
/// The widest status or head in German or British English in bold at the row font, 131.4 with the
/// cell padding on both sides, rounded up.
pub const SOUND_STATE_COLUMN: f32 = 132.0;
/// The initial width of the language column of the sound table.
///
/// The default language with the fallback mark, "English(US) (Ersatz)", in bold at the row font,
/// 155.0 with the cell padding on both sides, rounded up.
pub const SOUND_LANGUAGE_COLUMN: f32 = 155.0;
