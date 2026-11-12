// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use egui::Color32;
use egui::epaint::Shadow;
use serde_json::Value;

use super::categorical::{self as c, ItemClassTint, PlanetTint};
use super::{desktop, palette as p, tokens as t, window_metrics};

const TOKENS: &str = include_str!("fixtures/tokens.json");
const DESKTOP: &str = include_str!("../../../../docs/design/p4kx-desktop-spec.json");

fn colours() -> BTreeMap<String, Color32> {
    let mut map: BTreeMap<String, Color32> = [
        ("color-primary", p::PRIMARY),
        ("color-white", p::WHITE),
        ("color-bg-black", p::BLACK),
        ("color-canvas", p::CANVAS),
        ("color-accent-light", p::ACCENT_LIGHT),
        ("color-accent-dark", p::ACCENT_DARK),
        ("color-gray-1", p::GRAY_1),
        ("color-gray-2", p::GRAY_2),
        ("color-gray-3", p::GRAY_3),
        ("color-gray-4", p::GRAY_4),
        ("color-bg-dark-gray", p::BG_DARK_GRAY),
        ("color-surface-input", p::SURFACE_INPUT),
        ("color-control", p::CONTROL),
        ("color-control-hover", p::CONTROL_HOVER),
        ("color-count-badge", p::COUNT_BADGE),
        ("color-dept-raumueberlegenheit", c::DEPT_RAUMUEBERLEGENHEIT),
        ("color-dept-forschung", c::DEPT_FORSCHUNG),
        ("color-dept-sub-radar", c::DEPT_SUB_RADAR),
        ("color-dept-marinekorps", c::DEPT_MARINEKORPS),
        ("color-dept-profit", c::DEPT_PROFIT),
        ("color-dept-search-rescue", c::DEPT_SEARCH_RESCUE),
        ("color-dept-combat", c::DEPT_COMBAT),
        ("color-dept-research", c::DEPT_RESEARCH),
        ("color-dept-marine", c::DEPT_MARINE),
        ("color-danger", p::DANGER),
        ("color-success", p::SUCCESS),
        ("color-warning", p::WARNING),
        ("color-info", p::INFO),
        ("color-secondary", p::SECONDARY),
        ("color-danger-text", p::DANGER_TEXT),
        ("color-info-text", p::INFO_TEXT),
        ("color-success-text", p::SUCCESS_TEXT),
        ("color-gray-2-text", p::GRAY_2_TEXT),
        ("action-primary", p::ACTION_PRIMARY),
        ("action-emphasis", p::ACTION_EMPHASIS),
        ("action-neutral", p::ACTION_NEUTRAL),
        ("data-fg", p::DATA_FG),
        ("surface-canvas", p::SURFACE_CANVAS),
        ("surface-panel", p::SURFACE_PANEL),
        ("surface-inset", p::SURFACE_INSET),
        ("surface-bar", p::SURFACE_BAR),
        ("surface-raised", p::SURFACE_RAISED),
        ("control-bg", p::CONTROL_BG),
        ("control-bg-hover", p::CONTROL_BG_HOVER),
        ("table-stripe-bg", p::TABLE_STRIPE_BG),
        ("control-border", p::CONTROL_BORDER),
        ("control-fg", p::CONTROL_FG),
        ("control-selected-bg", p::CONTROL_SELECTED_BG),
        ("control-selected-border", p::CONTROL_SELECTED_BORDER),
        ("control-selected-fg", p::CONTROL_SELECTED_FG),
        ("color-danger-hover", p::DANGER_HOVER),
        ("color-success-hover", p::SUCCESS_HOVER),
    ]
    .into_iter()
    .map(|(name, colour)| (name.to_owned(), colour))
    .collect();
    add_categorical(&mut map);
    map
}

fn add_categorical(map: &mut BTreeMap<String, Color32>) {
    let mut planet = |name: &str, tint: PlanetTint| {
        map.insert(format!("planet-{name}"), tint.fill);
        map.insert(format!("planet-{name}-stripe"), tint.stripe);
    };
    planet("hurston", c::PLANET_HURSTON);
    planet("crusader", c::PLANET_CRUSADER);
    planet("arccorp", c::PLANET_ARCCORP);
    planet("microtech", c::PLANET_MICROTECH);
    planet("terra", c::PLANET_TERRA);
    planet("delamar", c::PLANET_DELAMAR);
    for (index, tint) in c::PLANET_PYRO.into_iter().enumerate() {
        planet(&format!("pyro-{}", index + 1), tint);
    }
    for (index, tint) in c::PLANET_HASH.into_iter().enumerate() {
        planet(&format!("hash-{index}"), tint);
    }
    let mut item_class = |name: &str, tint: ItemClassTint| {
        map.insert(format!("item-class-{name}"), tint.hue);
        map.insert(format!("item-class-{name}-text"), tint.text);
    };
    item_class("military", c::ITEM_CLASS_MILITARY);
    item_class("civilian", c::ITEM_CLASS_CIVILIAN);
    item_class("industrial", c::ITEM_CLASS_INDUSTRIAL);
    item_class("stealth", c::ITEM_CLASS_STEALTH);
    item_class("competition", c::ITEM_CLASS_COMPETITION);
}

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn tokens_of<'a>(json: &'a Value, group: &str) -> Vec<(&'a str, &'a str)> {
    json[group]["tokens"].as_array().map_or_default(|tokens| {
        tokens
            .iter()
            .filter_map(|token| Some((token["name"].as_str()?, token["value"].as_str()?)))
            .collect()
    })
}

fn resolve<'a>(values: &BTreeMap<&str, &'a str>, value: &'a str) -> &'a str {
    value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .map_or(value, |alias| {
            values
                .get(alias)
                .map_or("", |target| resolve(values, target))
        })
}

fn parse_colour(value: &str) -> Option<[u8; 4]> {
    if let Some(hex) = value.strip_prefix('#') {
        let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        return Some([channel(0)?, channel(2)?, channel(4)?, 255]);
    }
    let inner = value.strip_prefix("rgba(")?.strip_suffix(')')?;
    let parts: Vec<f32> = inner
        .split(',')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    let [r, g, b, a] = parts.as_slice() else {
        return None;
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is clamped to the u8 range first"
    )]
    let byte = |value: f32| value.round().clamp(0.0, 255.0) as u8;
    Some([byte(*r), byte(*g), byte(*b), byte(a * 255.0)])
}

fn close(actual: Color32, expected: [u8; 4]) -> bool {
    actual
        .to_srgba_unmultiplied()
        .iter()
        .zip(expected)
        .all(|(&got, want)| got.abs_diff(want) <= 2)
}

fn length(value: &str) -> f32 {
    let number = |text: &str| text.parse::<f32>().unwrap_or(f32::NAN);
    value.strip_suffix("rem").map_or_else(
        || {
            let plain = value
                .strip_suffix("px")
                .or_else(|| value.strip_suffix("em"))
                .unwrap_or(value);
            number(plain)
        },
        |rem| number(rem) * t::REM,
    )
}

#[test]
fn every_colour_token_of_the_theme_is_mirrored_with_its_value() {
    let json = fixture(TOKENS);
    let tokens = tokens_of(&json, "color");
    let values: BTreeMap<&str, &str> = tokens.iter().copied().collect();
    let mirrored = colours();

    assert!(!tokens.is_empty(), "the fixture holds colour tokens");
    for (name, value) in &tokens {
        let expected = parse_colour(resolve(&values, value));
        let actual = mirrored.get(*name);
        assert!(actual.is_some(), "{name} is mirrored");
        assert!(
            expected
                .zip(actual)
                .is_some_and(|(want, got)| close(*got, want)),
            "{name} is {value}"
        );
    }
    assert_eq!(mirrored.len(), tokens.len(), "no colour beyond the tokens");
}

#[test]
fn type_spacing_and_radius_tokens_match_the_theme() {
    let json = fixture(TOKENS);
    let mirrored: BTreeMap<&str, f32> = [
        ("fs-h1", t::FS_H1),
        ("fs-h2", t::FS_H2),
        ("fs-h3", t::FS_H3),
        ("fs-body", t::FS_BODY),
        ("fs-sm", t::FS_SM),
        ("fs-xs", t::FS_XS),
        ("fs-2xs", t::FS_2XS),
        ("fw-thin", f32::from(t::FW_THIN)),
        ("fw-regular", f32::from(t::FW_REGULAR)),
        ("fw-bold", f32::from(t::FW_BOLD)),
        ("fw-black", f32::from(t::FW_BLACK)),
        ("tracking-display", t::TRACKING_DISPLAY),
        ("tracking-overline", t::TRACKING_OVERLINE),
        ("lh-body", t::LH_BODY),
        ("space-1", t::SPACE_1),
        ("space-2", t::SPACE_2),
        ("space-3", t::SPACE_3),
        ("space-4", t::SPACE_4),
        ("space-6", t::SPACE_6),
        ("space-8", t::SPACE_8),
        ("touch-target", t::TOUCH_TARGET),
        ("content-max", t::CONTENT_MAX),
        ("radius-none", t::RADIUS_NONE),
        ("radius-sm", t::RADIUS_SM),
        ("radius-pill", t::RADIUS_PILL),
    ]
    .into();

    for group in [
        "fontSize",
        "fontWeight",
        "letterSpacing",
        "lineHeight",
        "spacing",
        "radius",
    ] {
        for (name, value) in tokens_of(&json, group) {
            let expected = length(value);
            let actual = mirrored.get(name).copied();
            assert!(actual.is_some(), "{name} is mirrored");
            assert!(
                actual.is_some_and(|got| (got - expected).abs() < 0.01),
                "{name} is {value}"
            );
        }
    }
}

#[test]
fn glows_and_shadows_match_the_theme() {
    let json = fixture(TOKENS);
    let mirrored: BTreeMap<&str, Shadow> = [
        ("glow-primary", t::GLOW_PRIMARY),
        ("glow-primary-lg", t::GLOW_PRIMARY_LG),
        ("glow-danger-lg", t::GLOW_DANGER_LG),
        ("shadow-drawer", t::SHADOW_DRAWER),
        ("shadow-dropdown", t::SHADOW_DROPDOWN),
    ]
    .into();

    for (name, value) in tokens_of(&json, "shadow") {
        let (geometry, colour) = value.split_once("rgba").unwrap_or((value, ""));
        let numbers: Vec<f32> = geometry.split_whitespace().map(length).collect();
        let shadow = mirrored.get(name);
        assert!(shadow.is_some(), "{name} is mirrored");
        let Some(shadow) = shadow else { continue };
        assert_eq!(
            numbers,
            [
                f32::from(shadow.offset[0]),
                f32::from(shadow.offset[1]),
                f32::from(shadow.blur)
            ],
            "{name} offset and blur"
        );
        let expected = parse_colour(&format!("rgba{colour}"));
        assert!(
            expected.is_some_and(|want| close(shadow.color, want)),
            "{name} colour"
        );
    }
}

const DESKTOP_METRICS: [(&str, &str, f32); 70] = [
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

    for (element, key, value) in DESKTOP_METRICS {
        let expected = number(element, key);
        assert!(
            expected.is_some_and(|want| (want - f64::from(value)).abs() < 0.01),
            "{element}.{key} is {expected:?}, the theme has {value}"
        );
    }
}
