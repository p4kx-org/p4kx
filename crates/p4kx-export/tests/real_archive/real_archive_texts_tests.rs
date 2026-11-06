// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::{Coverage, Oracle, floor_verdict, keyed_texts, verdict};

#[test]
fn source_lines_define_duplicate_suffix_case_and_display_rules() {
    let oracle = Oracle::parse(
        "\u{feff} Title,P=Suffix\r\nTITLE=old\r\ntitle=  Last\\nLine  \r\n\
         fallback=\nfallback,P=  Other  \nblank=  \ninvalid\n=ignored\n\
         choice=plain\nchoice,P=suffix\nequals=a=b\n",
    );
    for (key, expected) in [
        ("@TITLE", Some("Last\nLine")),
        ("fallback", Some("Other")),
        ("choice", Some("plain")),
        ("equals", Some("a=b")),
        ("blank", None),
        ("missing", None),
    ] {
        assert_eq!(oracle.text(key), expected, "source rule for {key}");
    }
}

#[test]
fn one_wrong_translation_fails_despite_other_resolved_texts() {
    let oracle = Oracle::parse("name=Correct\n");
    let mut coverage = Coverage::default();
    for _ in 0..100 {
        coverage.count(&oracle, "@name", Some(&json!("Correct")));
    }
    coverage.count(&oracle, "@name", Some(&json!("Wrong")));
    let mut failures = Vec::new();
    verdict(Kind::Items, "de", &coverage, Some(1.0), &mut failures);
    assert_eq!(coverage.resolved, 100, "unrelated keys resolve");
    assert_eq!(coverage.contradictions, 1, "the wrong value is detected");
    assert_eq!(failures.len(), 1, "a favourable ratio cannot hide a mismatch");
}

#[test]
fn only_explicit_null_can_represent_a_source_gap() {
    let oracle = Oracle::parse("empty= \n");
    let mut coverage = Coverage::default();
    coverage.count(&oracle, "@empty", Some(&Value::Null));
    coverage.count(&oracle, "@missing", Some(&Value::Null));
    coverage.count(&oracle, "@missing", None);
    coverage.count(&oracle, "@missing", Some(&json!(4)));
    coverage.count(&oracle, "@missing", Some(&json!("invented")));
    assert_eq!(coverage.fallback, 2, "empty and missing source values stay null");
    assert_eq!(coverage.contradictions, 3, "malformed or invented texts fail");
}

#[test]
fn a_mission_rewrite_still_requires_nonempty_text() {
    let oracle = Oracle::parse("title=Go to ~mission(Location)\n");
    let mut coverage = Coverage::default();
    coverage.count(&oracle, "@title", Some(&json!("Go to [LOCATION]")));
    coverage.count(&oracle, "@title", Some(&json!("")));
    coverage.count(&oracle, "@title", Some(&Value::Null));
    assert_eq!(coverage.resolved, 1, "the known token rewrite is present");
    assert_eq!(coverage.contradictions, 2, "empty and missing rewrites fail");
}

#[test]
fn placeholders_are_excluded_without_hiding_other_missing_keys() {
    let entity = json!({
        "name": {"key": "@loc_placeholder", "en": null, "de": null},
        "nested": {"name": {"key": "@missing", "en": null, "de": null}}
    });
    let mut found = Vec::new();
    keyed_texts(&entity, &["name".to_owned()], &mut found);
    let keys: Vec<_> = found.iter().map(|(key, _)| *key).collect();
    assert_eq!(keys, ["@missing"], "only placeholder keys are excluded");
}

#[test]
fn a_per_kind_floor_survives_unlimited_source_gaps() {
    let mut failures = Vec::new();
    let coverage = Coverage::default();
    verdict(
        Kind::MissionBrokerEntries,
        "de",
        &coverage,
        Some(1.0),
        &mut failures,
    );
    assert!(failures.is_empty(), "the share alone has no floor");
    floor_verdict(Kind::MissionBrokerEntries, &coverage, &mut failures);
    assert_eq!(failures.len(), 1, "the resolved-text floor remains binding");
}
