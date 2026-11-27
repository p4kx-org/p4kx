// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The colour tokens of the P4KX design, named as in the token fixture.

use egui::Color32;

/// `color-primary`, the house orange: the filled CTA, headings, links, focus and HUD brackets.
pub const PRIMARY: Color32 = Color32::from_rgb(0xE7, 0x7E, 0x23);
/// `color-white`: data readouts, selected-control text, text on danger and success fills.
pub const WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// `color-bg-black`: the text on the orange CTA and on light fills, never a background.
pub const BLACK: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
/// `color-canvas`: the background around the working surfaces.
pub const CANVAS: Color32 = Color32::from_rgb(0x0D, 0x0F, 0x12);
/// `brand-tile`: the fixed background of the shipped logo artwork.
pub const BRAND_TILE: Color32 = Color32::from_rgb(0x0F, 0x0F, 0x0F);
/// `color-accent-light`: the hover state of every orange element.
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0xEE, 0xB6, 0x4B);
/// `color-accent-dark`: admin chrome and deep accents, never text.
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0xC4, 0x5C, 0x00);
/// `color-gray-1`: body text, labels and table text.
pub const GRAY_1: Color32 = Color32::from_rgb(0xD7, 0xDA, 0xDE);
/// `color-gray-2`: the border of every control, the scrollbar thumb and status dots, never text.
pub const GRAY_2: Color32 = Color32::from_rgb(0x8F, 0x93, 0x99);
/// `color-gray-3`: decorative hairlines and panel frames, never a control border.
pub const GRAY_3: Color32 = Color32::from_rgb(0x2A, 0x30, 0x38);
/// `color-gray-4`: the panel, the standard surface of tables, modals and the sidebar.
pub const GRAY_4: Color32 = Color32::from_rgb(0x1A, 0x1E, 0x24);
/// `color-bg-dark-gray`, alias of [`GRAY_4`].
pub const BG_DARK_GRAY: Color32 = GRAY_4;
/// `color-surface-input`: the bar of header, footer, tab bar and table head, and inset fields.
pub const SURFACE_INPUT: Color32 = Color32::from_rgb(0x14, 0x17, 0x1C);
/// `color-control`: the resting fill of neutral controls.
pub const CONTROL: Color32 = Color32::from_rgb(0x2F, 0x36, 0x40);
/// `color-control-hover`: the hover fill of controls and segments.
pub const CONTROL_HOVER: Color32 = Color32::from_rgb(0x35, 0x3C, 0x48);
/// `color-count-badge`: the fill of a title counter.
pub const COUNT_BADGE: Color32 = Color32::from_rgb(0x2F, 0x39, 0x4C);
/// `color-danger`: danger as a fill or border.
pub const DANGER: Color32 = Color32::from_rgb(0xA3, 0x00, 0x0A);
/// `color-success`: success as a fill or border.
pub const SUCCESS: Color32 = Color32::from_rgb(0x23, 0x9E, 0x33);
/// `color-warning`: warning as a border and as text.
pub const WARNING: Color32 = Color32::from_rgb(0xFF, 0xD2, 0x3F);
/// `color-info`: info as a fill or border.
pub const INFO: Color32 = Color32::from_rgb(0x35, 0x5D, 0xDC);
/// `color-secondary`: the cross-squadron highlight.
pub const SECONDARY: Color32 = Color32::from_rgb(0xFF, 0xD2, 0x3F);
/// `color-danger-text`: danger as text.
pub const DANGER_TEXT: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x70);
/// `color-info-text`: info as text.
pub const INFO_TEXT: Color32 = Color32::from_rgb(0x8F, 0xB2, 0xF5);
/// `color-success-text`: success as text.
pub const SUCCESS_TEXT: Color32 = Color32::from_rgb(0x4C, 0xCB, 0x5C);
/// `color-title`: box titles and section headings.
pub const TITLE: Color32 = Color32::from_rgb(0xC9, 0xCE, 0xD5);
/// `color-gray-2-text`: muted text, hints, placeholders and inactive tabs.
pub const GRAY_2_TEXT: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xAB);
/// `color-danger-hover`: the hover fill of the danger button.
pub const DANGER_HOVER: Color32 = Color32::from_rgb(0xD4, 0x1A, 0x25);
/// `color-success-hover`: the hover fill of the success button.
pub const SUCCESS_HOVER: Color32 = Color32::from_rgb(0x2E, 0xBC, 0x3D);
/// `action-primary`: the one filled CTA per context.
pub const ACTION_PRIMARY: Color32 = PRIMARY;
/// `action-emphasis`: the outline secondary action.
pub const ACTION_EMPHASIS: Color32 = PRIMARY;
/// `action-neutral`: the ghost border.
pub const ACTION_NEUTRAL: Color32 = GRAY_2;
/// `data-fg`: bright neutral data readouts.
pub const DATA_FG: Color32 = WHITE;
/// `surface-canvas`, step 0 of the surface ladder: the page background.
pub const SURFACE_CANVAS: Color32 = CANVAS;
/// `surface-panel`, step 1: the work surface of toolbar, table and footer.
pub const SURFACE_PANEL: Color32 = GRAY_4;
/// `surface-card`: a tile inside a panel.
pub const SURFACE_CARD: Color32 = Color32::from_rgb(0x20, 0x25, 0x2C);
/// `surface-card-hover`: the hovered tile and table-row surface.
pub const SURFACE_CARD_HOVER: Color32 = Color32::from_rgb(0x27, 0x2D, 0x35);
/// `surface-inset`: a recessed surface for content inside a tile or panel.
pub const SURFACE_INSET: Color32 = SURFACE_INPUT;
/// `surface-bar`: header, footer, tab bar and table head.
pub const SURFACE_BAR: Color32 = SURFACE_INPUT;
/// `surface-raised`: the `.btn-secondary` fill.
pub const SURFACE_RAISED: Color32 = GRAY_3;
/// `control-bg`: the resting fill of every non-CTA control.
pub const CONTROL_BG: Color32 = CONTROL;
/// `control-bg-hover`: the hover fill of controls.
pub const CONTROL_BG_HOVER: Color32 = CONTROL_HOVER;
/// `table-stripe-bg`: the neutral zebra stripe on a panel.
pub const TABLE_STRIPE_BG: Color32 = Color32::from_rgb(0x1E, 0x23, 0x29);
/// `table-row-hover`: the hovered table row on a panel.
pub const TABLE_ROW_HOVER: Color32 = SURFACE_CARD_HOVER;
/// `control-border`: the border of every non-CTA control.
pub const CONTROL_BORDER: Color32 = GRAY_2;
/// `control-fg`: control label text.
pub const CONTROL_FG: Color32 = GRAY_1;
/// `control-selected-bg`: the orange tint of a selected segment.
pub const CONTROL_SELECTED_BG: Color32 = Color32::from_rgba_unmultiplied_const(231, 126, 35, 66);
/// `control-selected-border`: the border and underline of a selected segment.
pub const CONTROL_SELECTED_BORDER: Color32 = PRIMARY;
/// `control-selected-fg`: the text of a selected segment.
pub const CONTROL_SELECTED_FG: Color32 = WHITE;
