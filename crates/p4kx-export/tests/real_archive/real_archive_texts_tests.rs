// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

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
    assert_eq!(
        failures.len(),
        1,
        "a favourable ratio cannot hide a mismatch"
    );
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
    assert_eq!(
        coverage.fallback, 2,
        "empty and missing source values stay null"
    );
    assert_eq!(
        coverage.contradictions, 3,
        "malformed or invented texts fail"
    );
}

#[test]
fn a_mission_rewrite_still_requires_nonempty_text() {
    let oracle = Oracle::parse("title=Go to ~mission(Location)\n");
    let mut coverage = Coverage::default();
    coverage.count(&oracle, "@title", Some(&json!("Go to [LOCATION]")));
    coverage.count(&oracle, "@title", Some(&json!("")));
    coverage.count(&oracle, "@title", Some(&Value::Null));
    assert_eq!(coverage.resolved, 1, "the known token rewrite is present");
    assert_eq!(
        coverage.contradictions, 2,
        "empty and missing rewrites fail"
    );
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

fn canonical_catalog() -> Result<Value, Box<dyn Error>> {
    use p4kx_export::{CatalogOptions, Language};
    use p4kx_extraction::Archive;
    use p4kx_testkit::{
        ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct,
        TestType, TestValue,
    };
    use std::time::UNIX_EPOCH;
    let mut forge = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field("Localization", TestType::Class("SCItemLocalization".into())),
        );
    for guid in [1, 2] {
        forge = forge.record(
            TestRecord::new(
                guid,
                "SCItemManufacturer",
                format!("SCItemManufacturer.M{guid}"),
                format!("libs/foundry/records/scitemmanufacturer/m{guid}.xml"),
            )
            .value("Code", TestValue::text("M"))
            .value(
                "Localization",
                TestValue::Struct(
                    TestInstance::new("SCItemLocalization")
                        .value("Name", TestValue::text("@name"))
                        .value("Description", TestValue::text("@description")),
                ),
            ),
        );
    }
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", forge.build()))
        .entry(TestEntry::zstd(
            Language::English.global_ini_path(),
            "name=Correct\ndescription=  \n",
        ))
        .entry(TestEntry::zstd(
            Language::German.global_ini_path(),
            "name=Correct\ndescription=  \n",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let built = p4kx_export::build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    assert!(
        built.summary().export_health().is_complete(),
        "the source fixture has complete production health"
    );
    Ok(serde_json::from_slice(built.json())?)
}

fn canonical_coverage(catalog: &Value) -> Result<(Coverage, Coverage), Box<dyn Error>> {
    let oracle = Oracle::parse("name=Correct\ndescription=  \n");
    super::kind_coverage(catalog, Kind::Manufacturers, &[], &oracle, &oracle)
}

#[test]
fn canonical_text_coverage_counts_each_source_use_and_retains_empty_source_texts()
-> Result<(), Box<dyn Error>> {
    let catalog = canonical_catalog()?;
    for counts in [
        canonical_coverage(&catalog)?.0,
        canonical_coverage(&catalog)?.1,
    ] {
        assert_eq!(
            (counts.resolved, counts.fallback, counts.contradictions),
            (2, 2, 0)
        );
    }
    let oracle = Oracle::parse("blank=  \n");
    assert_eq!(oracle.canonical_text("@blank"), Some(""));
    assert_eq!(oracle.text("@blank"), None);
    assert_eq!(oracle.canonical_text("@missing"), None);
    Ok(())
}

#[test]
fn a_present_canonical_reference_cannot_disappear_or_cross_its_domain() -> Result<(), Box<dyn Error>>
{
    let catalog = canonical_catalog()?;
    for reference in ["missing", "text:12:@description"] {
        let mut broken = catalog.clone();
        broken["definitions"]["manufacturers"]["rows"][0]["localization"]["value"]["name"]["value"] =
            json!(reference);
        assert!(
            canonical_coverage(&broken).is_err(),
            "invalid canonical text ownership fails"
        );
    }
    Ok(())
}

#[test]
fn a_missing_or_malformed_canonical_wrapper_is_not_a_zero_key_success() -> Result<(), Box<dyn Error>>
{
    let catalog = canonical_catalog()?;
    for value in [
        Value::Null,
        json!({"state": "present"}),
        json!({"state": "present", "value": 3}),
    ] {
        let mut broken = catalog.clone();
        broken["definitions"]["manufacturers"]["rows"][0]["localization"] = value;
        assert!(
            canonical_coverage(&broken).is_err(),
            "invalid canonical text ownership fails"
        );
    }
    Ok(())
}

#[test]
fn omitted_language_carriers_and_duplicate_canonical_targets_fail() -> Result<(), Box<dyn Error>> {
    let catalog = canonical_catalog()?;
    let mut missing = catalog.clone();
    missing["texts"]["publicNames"][0]["translations"]["value"]
        .as_object_mut()
        .ok_or("translations")?
        .remove("de")
        .ok_or("German carrier")?;
    assert!(
        canonical_coverage(&missing).is_err(),
        "a missing language is not a text gap"
    );
    let mut duplicate = catalog;
    let target = duplicate["texts"]["publicNames"][0].clone();
    duplicate["texts"]["publicNames"]
        .as_array_mut()
        .ok_or("name table")?
        .push(target);
    assert!(
        canonical_coverage(&duplicate).is_err(),
        "duplicate canonical targets are ambiguous"
    );
    Ok(())
}

#[test]
fn wrong_translations_and_empty_text_without_a_source_key_are_contradictions()
-> Result<(), Box<dyn Error>> {
    let catalog = canonical_catalog()?;
    let mut wrong = catalog.clone();
    wrong["texts"]["publicNames"][0]["translations"]["value"]["en"]["value"] = json!("Wrong");
    assert_eq!(canonical_coverage(&wrong)?.0.contradictions, 2);
    let mut lost_empty = catalog.clone();
    lost_empty["texts"]["gameTexts"][0]["translations"]["value"]["en"]["state"] = json!("absent");
    lost_empty["texts"]["gameTexts"][0]["translations"]["value"]["en"]["value"] = Value::Null;
    assert_eq!(canonical_coverage(&lost_empty)?.0.contradictions, 2);
    let mut missing_source = catalog;
    missing_source["texts"]["gameTexts"][0]["key"]["value"] = json!("@missing");
    assert_eq!(canonical_coverage(&missing_source)?.0.contradictions, 2);
    Ok(())
}

#[test]
fn absent_localization_has_no_invented_child_reference() -> Result<(), Box<dyn Error>> {
    let mut catalog = canonical_catalog()?;
    let localization = catalog
        .pointer_mut("/definitions/manufacturers/rows/0/localization")
        .ok_or("actual Localization carrier")?;
    localization["state"] = json!("absent");
    localization["value"] = Value::Null;
    let row = &catalog["definitions"]["manufacturers"]["rows"][0];
    assert!(
        super::manufacturer_text(&catalog, row, "name")?.is_none(),
        "an absent parent invents no child reference"
    );
    Ok(())
}

#[test]
fn canonical_raw_mission_tokens_are_compared_exactly() {
    let oracle = Oracle::parse("raw=Go to ~mission(Location)\n");
    let mut coverage = Coverage::default();
    coverage.count_canonical(&oracle, "@raw", &json!("Go to ~mission(Location)"));
    coverage.count_canonical(&oracle, "@raw", &json!("Go to [LOCATION]"));
    assert_eq!(
        coverage.resolved, 1,
        "raw canonical text retains its actual tokens"
    );
    assert_eq!(
        coverage.contradictions, 1,
        "legacy mission rewriting does not excuse altered canonical text"
    );
}

#[path = "real_archive_texts_baseline_tests.rs"]
mod baseline;
