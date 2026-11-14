// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::desktop as d;

const DESKTOP: &str = include_str!("../../../../docs/design/p4kx-desktop-spec.json");

fn spec() -> Value {
    serde_json::from_str(DESKTOP).unwrap_or_else(|error| panic!("the desktop spec: {error}"))
}

#[test]
fn the_localization_table_metrics_match_the_desktop_spec() {
    let json = spec();

    for (key, value) in [
        ("keyColumnWidth", d::LOCALIZATION_KEY_WIDTH),
        ("textColumnWidth", d::LOCALIZATION_TEXT_WIDTH),
        ("columnMinWidth", d::QUERY_COLUMN_MIN_WIDTH),
    ] {
        let expected = json["localizationTable"][key].as_f64();
        assert!(
            expected.is_some_and(|want| (want - f64::from(value)).abs() < 0.01),
            "localizationTable.{key} is {expected:?}, the theme has {value}"
        );
    }
}

#[test]
fn the_localization_tab_is_listed_among_the_work_tabs() {
    let json = spec();

    let tabs: Vec<&str> = json["workTabs"]["tabs"]
        .as_array()
        .map_or_default(|tabs| tabs.iter().filter_map(Value::as_str).collect());

    assert_eq!(
        tabs,
        ["archive", "comparison", "query", "localization"],
        "work tabs"
    );
}
