// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{DESKTOP, desktop, fixture, t};
use crate::theme::window_metrics;

const DESKTOP_METRICS: &[(&str, &str, f32)] = &[
    ("meta", "rowDensity", desktop::ROW_HEIGHT),
    ("header", "height", desktop::HEADER_HEIGHT),
    ("header", "brandMarkSize", desktop::BRAND_MARK),
    ("header", "brandGap", desktop::BRAND_GAP),
    ("header", "wordmarkHeight", desktop::BRAND_HEIGHT),
    ("menuLink", "paddingX", desktop::MENU_LINK_PADDING_X),
    ("dropdown", "minWidth", desktop::DROPDOWN_MIN_WIDTH),
    ("sidebar", "defaultWidth", desktop::SIDEBAR_WIDTH),
    ("sidebar", "minWidth", desktop::SIDEBAR_MIN_WIDTH),
    ("sidebar", "maxWidth", desktop::SIDEBAR_MAX_WIDTH),
    (
        "sidebar",
        "collapseWidth",
        window_metrics::SIDEBAR_COLLAPSE_WIDTH,
    ),
    ("sidebar", "railWidth", window_metrics::SIDEBAR_RAIL_WIDTH),
    ("workSurface", "minWidth", window_metrics::WORK_MIN_WIDTH),
    ("workSurface", "minHeight", window_metrics::WORK_MIN_HEIGHT),
    ("workSurface", "panelInset", window_metrics::PANEL_INSET),
    (
        "workSurface",
        "browseListMinHeight",
        window_metrics::BROWSE_LIST_MIN_HEIGHT,
    ),
    (
        "workSurface",
        "wrappedActionsHeight",
        window_metrics::WRAPPED_ACTIONS_HEIGHT,
    ),
    (
        "workSurface",
        "soundsToolbarMinHeight",
        window_metrics::SOUNDS_TOOLBAR_MIN_HEIGHT,
    ),
    ("treeRow", "indentPerLevel", desktop::TREE_INDENT),
    ("treeRow", "paddingLeftBase", desktop::TREE_PADDING_LEFT),
    ("treeRow", "fontSize", desktop::ROW_FONT),
    ("toolbar", "padding", desktop::TOOLBAR_PADDING),
    ("searchField", "flexBasis", desktop::SEARCH_WIDTH),
    ("searchField", "paddingLeft", desktop::SEARCH_PADDING_LEFT),
    ("quickFilter", "height", desktop::SEGMENT_HEIGHT),
    ("buttonBase", "height", desktop::BUTTON_HEIGHT),
    ("buttonBase", "paddingX", desktop::BUTTON_PADDING_X),
    ("buttonCompact", "height", desktop::BUTTON_XS_HEIGHT),
    ("buttonCompact", "paddingX", desktop::BUTTON_XS_PADDING_X),
    ("buttonCompact", "fontSize", desktop::BUTTON_XS_SIZE),
    ("tableHeader", "fontSize", desktop::TABLE_HEAD_SIZE),
    ("tableHeader", "paddingX", desktop::CELL_PADDING_X),
    ("checkbox", "size", desktop::TOGGLE_SIZE),
    ("radio", "dotSize", desktop::RADIO_DOT),
    ("tabBar", "fontSize", desktop::TAB_SIZE),
    ("previewPanel", "defaultHeight", desktop::PREVIEW_HEIGHT),
    ("previewPanel", "minHeight", desktop::PREVIEW_MIN_HEIGHT),
    ("hexRow", "height", desktop::HEX_ROW_HEIGHT),
    ("hexRow", "offsetWidth", desktop::HEX_OFFSET_WIDTH),
    ("hexRow", "byteCellWidth", desktop::HEX_BYTE_WIDTH),
    ("hexRow", "groupGap", desktop::HEX_GROUP_GAP),
    ("hexRow", "asciiCellWidth", desktop::HEX_ASCII_WIDTH),
    ("statusBar", "height", desktop::STATUS_HEIGHT),
    ("statusBar", "pathMaxWidth", desktop::STATUS_PATH_MAX_WIDTH),
    ("emptyState", "padding", desktop::EMPTY_PADDING),
    ("emptyState", "titleFontSize", desktop::TITLE_SIZE),
    ("modal", "width", desktop::MODAL_WIDTH),
    ("modal", "wideWidth", desktop::MODAL_WIDE_WIDTH),
    ("modal", "bodyGroupGap", desktop::MODAL_GROUP_GAP),
    (
        "collapsibleSection",
        "listMaxHeight",
        desktop::SECTION_LIST_HEIGHT,
    ),
    ("progressBar", "height", desktop::PROGRESS_HEIGHT),
    ("progressBar", "sweepShare", desktop::PROGRESS_SWEEP_SHARE),
    ("spinner", "size", desktop::SPINNER),
    ("spinner", "sizeInline", desktop::SPINNER_INLINE),
    ("detailList", "keyColumnMin", desktop::DETAIL_KEY_MIN_WIDTH),
    ("detailList", "rowGap", desktop::DETAIL_ROW_GAP),
    ("detailList", "columnGap", desktop::DETAIL_COLUMN_GAP),
    (
        "detailList",
        "keyLetterSpacingEm",
        desktop::DETAIL_KEY_TRACKING,
    ),
    ("detailList", "valueFontSize", desktop::DETAIL_VALUE_SIZE),
    ("sectionTitle", "fontSize", desktop::PANEL_TITLE_SIZE),
    ("sectionTitle", "letterSpacingEm", t::TRACKING_DISPLAY),
    ("sectionTitle", "marginTop", desktop::PANEL_TITLE_MARGIN_TOP),
    (
        "sectionTitle",
        "marginBottom",
        desktop::PANEL_TITLE_MARGIN_BOTTOM,
    ),
    (
        "sectionTitle",
        "paddingBottom",
        desktop::PANEL_TITLE_PADDING_BOTTOM,
    ),
    ("tooltip", "maxWidth", desktop::TOOLTIP_MAX_WIDTH),
    ("tooltip", "paddingX", desktop::TOOLTIP_PADDING_X),
    ("tooltip", "paddingY", desktop::TOOLTIP_PADDING_Y),
    ("tooltip", "fontSize", desktop::TOOLTIP_SIZE),
    ("tooltip", "lineHeight", desktop::TOOLTIP_LINE_HEIGHT),
    ("scrollbar", "width", desktop::SCROLLBAR),
    ("scrollbar", "thumbMinLength", desktop::SCROLL_THUMB_MIN),
];

#[test]
fn the_desktop_metrics_match_the_desktop_spec() {
    let json = fixture(DESKTOP);
    let number = |element: &str, key: &str| json[element][key].as_f64();

    for &(element, key, value) in DESKTOP_METRICS {
        let expected = number(element, key);
        assert!(
            expected.is_some_and(|want| (want - f64::from(value)).abs() < 0.01),
            "{element}.{key} is {expected:?}, the theme has {value}"
        );
    }
}
