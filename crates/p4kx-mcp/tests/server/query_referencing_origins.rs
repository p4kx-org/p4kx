// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, ObservedReferenceIndex, ReferenceScanGap};
use p4kx_testkit::{DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

fn fixture(gap: bool) -> Vec<u8> {
    let mut excluded = TestRecord::new(7, "Source", "Source.Excluded", "source/excluded.xml")
        .value("links", TestValue::Array(vec![TestValue::Reference(3)]));
    if gap {
        let mut invalid = (u32::MAX - 1).to_le_bytes().to_vec();
        invalid.extend(0_u32.to_le_bytes());
        excluded = excluded.value("unknown", TestValue::Raw(invalid));
    }
    DataForgeBuilder::new()
        .structure(TestStruct::new("Target"))
        .structure(
            TestStruct::new("Source")
                .field("score", TestType::Int32)
                .array("links", TestType::ReferenceTo("Target".to_owned()))
                .field("unknown", TestType::StrongPointer("Source".to_owned())),
        )
        .record(TestRecord::new(
            1,
            "Target",
            "Target.First",
            "targets/group.xml",
        ))
        .record(TestRecord::new(
            2,
            "Target",
            "Target.Second",
            "targets/group.xml",
        ))
        .record(TestRecord::new(
            3,
            "Target",
            "Target.Excluded",
            "targets/excluded.xml",
        ))
        .record(TestRecord::new(
            4,
            "Target",
            "Target.Empty",
            "targets/empty.xml",
        ))
        .record(
            TestRecord::new(5, "Source", "Source.Alpha", "source/alpha.xml")
                .value("score", TestValue::Int32(12))
                .value(
                    "links",
                    TestValue::Array(
                        [1, 1, 3, 99]
                            .into_iter()
                            .map(TestValue::Reference)
                            .collect(),
                    ),
                ),
        )
        .record(
            TestRecord::new(6, "Source", "Source.Beta", "source/beta.xml")
                .value("score", TestValue::Int32(23))
                .value("links", TestValue::Array(vec![TestValue::Reference(2)])),
        )
        .record(excluded)
        .build()
}

#[test]
fn actual_target_edges_keep_duplicate_original_occurrences_across_cached_pages() -> TestResult {
    let install = Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", fixture(false)).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let query = r#"from Source referencing "targets/group" select score"#;
    let first = server.ok("run_query", &json!({"query":query, "limit":1}))?;
    let cursor = first.get("next_cursor").ok_or("cursor")?;
    let second = server.ok(
        "run_query",
        &json!({"query":query, "limit":1, "cursor":cursor}),
    )?;
    assert_eq!(
        first.pointer("/rows/0/score"),
        Some(&json!(12)),
        "first selected source"
    );
    assert_eq!(
        second.pointer("/rows/0/score"),
        Some(&json!(23)),
        "second selected source"
    );
    for page in [&first, &second] {
        assert_eq!(
            page.get("total"),
            Some(&json!(2)),
            "duplicate edges select one row"
        );
        for coordinates in [["4", "2", "0"], ["4", "3", "0"], ["5", "2", "1"]] {
            assert_site(
                page,
                "total",
                "dataforgeReferenceIndex",
                "incoming",
                &coordinates,
            )?;
        }
        assert_target(page, 0, "fileName")?;
        assert_target(page, 1, "fileName")?;
        marker(field(page, "total")?, true);
        let actual = uses(page, "total")?
            .iter()
            .filter(|site| site.pointer("/location/field") == Some(&json!("incoming")))
            .count();
        assert_eq!(
            actual, 3,
            "unrelated and unresolved edges supply no selected edge"
        );
    }
    Ok(())
}

#[test]
fn a_guid_selects_its_actual_target_and_an_empty_index_lookup_is_proved() -> TestResult {
    let install = crate::forge::install(fixture(false))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let by_guid = server.ok("run_query", &json!({
        "query": r#"from Source referencing "00000000-0000-0000-0000-000000000001" select score"#,
    }))?;
    assert_eq!(
        by_guid.get("total"),
        Some(&json!(1)),
        "only the named GUID's sources"
    );
    assert_target(&by_guid, 0, "guid")?;
    let empty = server.ok(
        "run_query",
        &json!({
            "query": r#"from Source referencing "targets/empty" select score"#,
        }),
    )?;
    assert_eq!(
        empty.get("total"),
        Some(&json!(0)),
        "existing target with no incoming edge"
    );
    assert_target(&empty, 3, "fileName")?;
    assert_site(
        &empty,
        "total",
        "dataforgeReferenceIndex",
        "scannedRecords",
        &["0", "7"],
    )?;
    marker(field(&empty, "total")?, false);
    Ok(())
}

#[test]
fn an_unreadable_original_pointer_leaves_the_encrypted_reference_scan_incomplete() -> TestResult {
    let bytes = fixture(true);
    let forge = DataForge::open(&bytes)?;
    let observed = ObservedReferenceIndex::build(&forge)?;
    assert_eq!(
        observed.gaps(),
        [ReferenceScanGap::Value {
            record_index: 6,
            value_ordinal: 3
        }],
        "the authored pointer leaves an actual original gap"
    );
    let install = Install::new(vec![TestEntry::zstd("Data/Game2.dcb", bytes).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let page = server.ok(
        "run_query",
        &json!({
            "query": r#"from Source referencing "targets/group" select score"#,
        }),
    )?;
    assert_eq!(
        page.get("total"),
        Some(&json!(2)),
        "readable selected edges still count"
    );
    assert_site(
        &page,
        "total",
        "dataforgeReferenceIndex",
        "unreadableValue",
        &["6", "3"],
    )?;
    let origin = field(&page, "total")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(true)),
        "known encrypted source retained"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(false)),
        "unreadable candidate could hide an edge"
    );
    Ok(())
}

fn assert_target(page: &Value, index: u32, label: &str) -> TestResult {
    assert!(
        uses(page, "total")?.iter().any(|site| {
            site.pointer("/location/type") == Some(&json!("dataForge"))
                && site.pointer("/location/recordIndex") == Some(&json!(index))
                && site.pointer("/location/field") == Some(&json!(label))
        }),
        "physical target Record {index} supplies {label}"
    );
    Ok(())
}
