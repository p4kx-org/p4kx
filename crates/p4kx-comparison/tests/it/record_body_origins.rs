// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_comparison::{
    Archive, CompareOptions, DataForgeEntry, DataForges, Levels, compare,
    compare_with_dataforge_entries,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn reports(before: &[u8], after: &[u8]) -> TestResult<(Value, Value)> {
    let dir = tempfile::tempdir()?;
    let open = |filename, bytes: &[u8], encrypted| -> TestResult<Archive> {
        let path = dir.path().join(filename);
        let entry = TestEntry::deflated("Data\\Game2.dcb", bytes);
        ArchiveBuilder::new()
            .entry(if encrypted { entry.encrypted() } else { entry })
            .write_to(&path)?;
        Ok(Archive::open_with_key(
            path,
            Some(p4kx_testkit::test_key()),
        )?)
    };
    let old = open("old.p4k", before, true)?;
    let new = open("new.p4k", after, false)?;
    let old_entry = DataForgeEntry::read(&old)?;
    let new_entry = DataForgeEntry::read(&new)?;
    let options = CompareOptions {
        levels: Levels::RECORDS,
        ..CompareOptions::default()
    };
    let old_forge = old_entry.open()?;
    let new_forge = new_entry.open()?;
    let forges = DataForges {
        old: &old_forge,
        new: &new_forge,
    };
    let bound =
        compare_with_dataforge_entries(&old, &new, forges, [&old_entry, &new_entry], &options)?;
    let bare = compare(&old, &new, Some(forges), &options)?;
    assert_eq!(
        bound, bare,
        "evidence preserves domain equality and ordered changes"
    );
    let result = (
        serde_json::to_value(bound.report())?,
        serde_json::to_value(bare.report())?,
    );
    assert_eq!(
        result.0.pointer("/records/changes"),
        result.1.pointer("/records/changes"),
        "bound and bare reports serialize the same changed values",
    );
    Ok(result)
}

fn duplicates(value: i32) -> TestResult<Vec<u8>> {
    let mut bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("shared", TestType::Int32))
        .structure(
            TestStruct::new("Root")
                .parent("Base")
                .field("shared", TestType::Int32),
        )
        .record(
            TestRecord::new(1, "Root", "Root.One", "one.xml")
                .value("shared", TestValue::Int32(value)),
        )
        .build();
    bytes
        .last_chunk_mut::<4>()
        .ok_or("second duplicate raw slot")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(bytes)
}

#[test]
fn record_body_origins_distinguish_duplicate_equal_named_slots() -> TestResult {
    let before = duplicates(11)?;
    let after = duplicates(22)?;
    let (bound, bare) = reports(&before, &after)?;
    for (index, property) in [(0, "0"), (1, "1")] {
        let row = format!("/records/changes/0/fields/{index}");
        assert_eq!(
            bound.pointer(&format!("{row}/path")),
            Some(&json!("shared")),
            "same display path"
        );
        let path = uses_at(&bound, &format!("{row}/path"), Some(true), true)?;
        require_property(&path, property, "name", "selector");
        for (source, encrypted, bytes, expected) in [
            ("old", true, before.as_slice(), 11_i32.to_le_bytes()),
            ("new", false, after.as_slice(), 22_i32.to_le_bytes()),
        ] {
            let value = format!("{row}/{source}/value");
            let kind = uses_at(
                &bound,
                &format!("{row}/{source}/type"),
                Some(encrypted),
                true,
            )?;
            require_property(&kind, property, "instanceLayout", "conversion");
            let sources = uses_at(&bound, &value, Some(encrypted), true)?;
            require_property(&sources, property, "instanceLayout", "conversion");
            let (start, actual) = raw_bytes(bytes, &sources, "slot", "payload")?;
            assert_eq!(
                start,
                bytes.len() - 8 + 4 * index,
                "physical same-name slot"
            );
            assert_eq!(actual, expected, "actual selected raw word");
            assert!(
                uses_at(&bare, &value, None, false)?.is_empty(),
                "unbound DataForge bytes acquire no source"
            );
        }
    }
    Ok(())
}

#[test]
fn record_body_origins_retain_reordered_nested_fields_and_array_admission() -> TestResult {
    let before = nested(false);
    let after = nested(true);
    let (bound, _) = reports(&before, &after)?;
    let one_sided = field_path(&bound, "groups[0].weights[1]")?;
    assert!(
        bound.pointer(&format!("{one_sided}/old")).is_none(),
        "no old array element"
    );
    assert!(
        origin_at(&bound, &format!("{one_sided}/old/value")).is_none(),
        "no fabricated old operand"
    );
    let path = uses_at(&bound, &format!("{one_sided}/path"), Some(true), true)?;
    let headers = path
        .iter()
        .filter(|site| {
            site.pointer("/location/field").and_then(Value::as_str) == Some("arrayHeader")
                && site.get("role").and_then(Value::as_str) == Some("selector")
        })
        .count();
    assert_eq!(
        headers, 3,
        "two outer admissions plus the present side's inner admission"
    );
    let sources = uses_at(&bound, &format!("{one_sided}/new/value"), Some(false), true)?;
    require_property(&sources, "0", "instanceLayout", "conversion");
    let (_, value) = raw_bytes(&after, &sources, "arrayElement", "payload")?;
    assert_eq!(value, 12_i32.to_le_bytes(), "exact new pool slot");
    let title = field_path(&bound, "groups[0].title")?;
    let evidence = uses_at(&bound, &format!("{title}/new/value"), Some(false), true)?;
    require_property(&evidence, "1", "instanceLayout", "conversion");
    let (_, name) = raw_bytes(&after, &evidence, "text", "payload")?;
    assert_eq!(
        name, b"New title",
        "the resolved string contributes, separate from its index"
    );
    let category = field_path(&bound, "kind")?;
    let enum_evidence = uses_at(&bound, &format!("{category}/old/value"), Some(true), true)?;
    require_property(&enum_evidence, "3", "targetedInstanceLayout", "conversion");
    let (_, text) = raw_bytes(&before, &enum_evidence, "text", "payload")?;
    assert_eq!(text, b"Alpha", "original enum option text");
    Ok(())
}

#[test]
fn record_body_origins_distinguish_null_and_unproved_resolved_reference_fields() -> TestResult {
    let make = |value| {
        DataForgeBuilder::new()
            .structure(TestStruct::new("Owner").field("target", TestType::Reference))
            .structure(TestStruct::new("Target"))
            .record(TestRecord::new(1, "Owner", "Owner.One", "owner.xml").value("target", value))
            .record(TestRecord::new(2, "Target", "Target.One", "target.xml"))
            .build()
    };
    let before = make(TestValue::Null);
    let after = make(TestValue::Reference(2));
    let (report, _) = reports(&before, &after)?;
    let target = field_path(&report, "target")?;
    let _ = uses_at(&report, &format!("{target}/old/type"), Some(true), true)?;
    assert!(
        origin_at(&report, &format!("{target}/old/value")).is_none(),
        "null has only type"
    );
    for member in ["guid", "record_path", "type"] {
        assert!(
            uses_at(
                &report,
                &format!("{target}/new/value/{member}"),
                None,
                false
            )?
            .is_empty(),
            "resolution evidence for {member} remains unknown",
        );
    }
    assert!(
        origin_at(&report, &format!("{target}/new/value/at")).is_none(),
        "ordinary Reference has no owned position"
    );
    Ok(())
}

fn nested(new: bool) -> Vec<u8> {
    let group = TestStruct::new("Group");
    let group = if new {
        group
            .array("weights", TestType::Int32)
            .field("title", TestType::String)
    } else {
        group
            .field("title", TestType::String)
            .array("weights", TestType::Int32)
    };
    let values = if new { vec![9, 12] } else { vec![7] };
    let item = TestInstance::new("Group")
        .value(
            "weights",
            TestValue::Array(values.into_iter().map(TestValue::Int32).collect()),
        )
        .value(
            "title",
            TestValue::text(if new { "New title" } else { "Old title" }),
        );
    DataForgeBuilder::new()
        .enumeration("Kind", ["Alpha", "Beta"])
        .structure(group)
        .structure(
            TestStruct::new("Root")
                .array("groups", TestType::Class("Group".to_owned()))
                .field("kind", TestType::Enum("Kind".to_owned())),
        )
        .record(
            TestRecord::new(1, "Root", "Root.One", "one.xml")
                .value("groups", TestValue::Array(vec![TestValue::Struct(item)]))
                .value("kind", TestValue::text(if new { "Beta" } else { "Alpha" })),
        )
        .build()
}

fn field_path(report: &Value, path: &str) -> TestResult<String> {
    let prefix = "/records/changes/0/fields";
    let index = report
        .pointer(prefix)
        .and_then(Value::as_array)
        .ok_or("changed fields")?
        .iter()
        .position(|field| field.get("path").and_then(Value::as_str) == Some(path))
        .ok_or("requested field change")?;
    Ok(format!("{prefix}/{index}"))
}

fn origin_at<'report>(report: &'report Value, path: &str) -> Option<&'report Value> {
    report.pointer("/records/fieldOrigins")?.get(path)
}

fn uses_at<'report>(
    report: &'report Value,
    path: &str,
    encrypted: Option<bool>,
    complete: bool,
) -> TestResult<Vec<&'report Value>> {
    let origin = origin_at(report, path).ok_or("immediate field evidence")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "{path} encryption proof"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(complete)),
        "{path} completeness"
    );
    let sources = report
        .pointer("/records/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("shared uses")?;
    origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("use refs")?
        .iter()
        .map(|reference| {
            sources
                .iter()
                .find(|site| site.get("id") == Some(reference))
                .ok_or_else(|| "registered source use".into())
        })
        .collect()
}

fn require_property(uses: &[&Value], index: &str, field: &str, role: &str) {
    assert!(
        uses.iter().any(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgePropertyDefinition")
                && site.pointer("/location/indices/0").and_then(Value::as_str) == Some(index)
                && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && site.get("role").and_then(Value::as_str) == Some(role)
        }),
        "property {index} {field} as {role}",
    );
}

fn raw_bytes<'input>(
    input: &'input [u8],
    uses: &[&Value],
    field: &str,
    role: &str,
) -> TestResult<(usize, &'input [u8])> {
    let location = uses
        .iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeDecodedBytes")
                && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && site.get("role").and_then(Value::as_str) == Some(role)
        })
        .and_then(|site| site.get("location"))
        .ok_or("precise byte location")?;
    let coordinate = |path| -> TestResult<usize> {
        Ok(location
            .pointer(path)
            .and_then(Value::as_str)
            .ok_or("byte coordinate")?
            .parse()?)
    };
    let start = coordinate("/indices/0")?;
    let end = coordinate("/indices/1")?;
    Ok((
        start,
        input.get(start..end).ok_or("range in original DataForge")?,
    ))
}
