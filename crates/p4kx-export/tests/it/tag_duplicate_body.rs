// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::TagBuildEvent;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const GUID: u128 = 0x5714;
const GUID_TEXT: &str = "00000000-0000-0000-0000-000000005714";

fn database() -> Vec<u8> {
    let mut database = DataForgeBuilder::new().version(8).structure(
        TestStruct::new("Tag")
            .field("tagName", TestType::String)
            .array("children", TestType::ReferenceTo("Tag".to_owned())),
    );
    for name in ["First", "Second"] {
        database = database.record(
            TestRecord::new(
                GUID,
                "Tag",
                format!("Tag.{name}"),
                format!("source/{name}.xml"),
            )
            .value("tagName", TestValue::text(name))
            .value("children", TestValue::Array(Vec::new())),
        );
    }
    database.build()
}

fn original(source: &Fixture) -> TestResult {
    let forge = source.entry.open()?;
    assert_eq!(
        forge.records().count(),
        2,
        "two actual physical declarations share one GUID"
    );
    for (index, name) in [(0, "First"), (1, "Second")] {
        let record = forge.record(index).ok_or("the original physical Tag")?;
        assert_eq!(
            record.guid().to_string(),
            GUID_TEXT,
            "the actual shared stored GUID"
        );
        assert_eq!(
            record.type_name()?,
            "Tag",
            "the actual readable source family"
        );
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the actual canonical owner is paired"
        );
        assert_eq!(
            tree.field("tagName"),
            Some(&p4kx_extraction::Value::String(name)),
            "distinct original names stay separate"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "the original source is fully readable"
        );
    }
    let mut duplicate = Vec::new();
    p4kx_extraction::TagTree::build_observed(&forge, &|| false, &mut |event| {
        if let TagBuildEvent::DuplicateSkipped { record } = event {
            duplicate.push(record.index());
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })?
    .ok_or("the original pass was not cancelled")?;
    assert_eq!(
        duplicate,
        [1],
        "the actual duplicate event belongs to the second physical declaration"
    );
    Ok(())
}

#[test]
fn duplicate_tag_diagnostic_preserves_header_facts_and_independent_control() -> TestResult {
    let mut inputs = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = Fixture::open(&database(), kind, Vec::new())?;
        original(&input)?;
        inputs.push((input, kind));
    }
    for (input, kind) in inputs {
        let (built, catalog) = input.diagnostic()?;
        let diagnostics = check::rows(&catalog, "/exportHealth/diagnostics")?;
        let found: Vec<_> = diagnostics
            .iter()
            .filter(|row| row.get("warningCode") == Some(&json!("tag_duplicate_guid")))
            .collect();
        assert_eq!(
            found.len(),
            1,
            "one original duplicate operation has one diagnostic aggregate"
        );
        let diagnostic = *found.first().ok_or("the duplicate diagnostic")?;
        assert_eq!(
            diagnostic.get("count"),
            Some(&json!(1)),
            "one duplicate physical Record"
        );
        let examples = diagnostic
            .get("examples")
            .and_then(Value::as_array)
            .ok_or("the actual retained example")?;
        assert_eq!(examples.len(), 1, "one actual original example");
        let example = examples.first().ok_or("the actual duplicate example")?;
        check::present(example, "recordIndex", &json!(1), kind.marker())?;
        let field = check::present_text(&catalog, example, "field", &json!("$guid"), Some(false))?;
        check::independent(field)?;
        let source_type = check::present_text(
            &catalog,
            example,
            "sourceType",
            &json!("Tag"),
            kind.marker(),
        )?;
        header(&catalog, source_type, "sourceType", "selector")?;
        let target = check::present(example, "targetGuid", &json!(GUID_TEXT), kind.marker())?;
        header(&catalog, target, "guid", "payload")?;
        assert_eq!(
            built
                .summary()
                .kind(Kind::Tags)
                .map(|summary| summary.count),
            Some(2),
            "both physical raw definitions survive the convenience duplicate policy"
        );
        assert_eq!(
            diagnostic.get("severity"),
            Some(&json!("information")),
            "the duplicate itself remains informational"
        );
        check::closure(&catalog)?;
    }
    Ok(())
}

fn header(catalog: &Value, cell: &Value, expected: &str, role: &str) -> TestResult {
    let uses = check::uses(catalog, cell)?;
    assert_eq!(uses.len(), 1, "only this original header datum contributes");
    let contribution = uses.first().ok_or("the original header use")?;
    assert_eq!(
        contribution.pointer("/location/namespace"),
        Some(&json!("dataforgeRecordHeader")),
        "the coordinate identifies an actual original header"
    );
    assert_eq!(
        contribution.pointer("/location/indices"),
        Some(&json!(["1"])),
        "the duplicate physical owner supplies the datum"
    );
    assert_eq!(
        contribution.pointer("/location/field"),
        Some(&json!(expected)),
        "the exact original header member"
    );
    assert_eq!(
        contribution.get("role"),
        Some(&json!(role)),
        "the original header primitive preserves its established role"
    );
    Ok(())
}
