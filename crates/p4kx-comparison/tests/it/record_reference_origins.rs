// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::record_body_origins::{field_path, origin_at, raw_bytes, reports, uses_at};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_stored_reference_does_not_borrow_its_guid_from_the_selected_record() -> TestResult {
    let before = reference(TestValue::Null);
    let stored_guid = [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0];
    let mut source_slot = vec![0xaa, 0xbb, 0xcc, 0xdd];
    source_slot.extend(stored_guid);
    let after = reference(TestValue::Raw(source_slot));
    let (doc, bare) = reports(&before, &after)?;
    let target = field_path(&doc, "link")?;
    let guid = uses_at(&doc, &format!("{target}/new/value/guid"), Some(false), true)?;
    let (_, bytes) = raw_bytes(&after, &guid, "slot", "payload")?;
    assert_eq!(bytes, stored_guid, "the original stored GUID suffix");
    assert!(
        guid.iter()
            .all(|site| site.pointer("/location/recordIndex").is_none()),
        "the authored GUID has no target header dependency"
    );
    let path = uses_at(
        &doc,
        &format!("{target}/new/value/record_path"),
        Some(false),
        true,
    )?;
    let (_, selector) = raw_bytes(&after, &path, "slot", "selector")?;
    assert_eq!(
        selector, bytes,
        "the same caller GUID selects target metadata"
    );
    expect_record(&path, 1, "fileName", "conversion");
    let (_, filename) = raw_bytes(&after, &path, "text", "payload")?;
    assert_eq!(
        filename, b"selected.xml",
        "the decoder's chosen duplicate GUID row"
    );
    let kind = uses_at(&doc, &format!("{target}/new/value/type"), Some(false), true)?;
    expect_record(&kind, 1, "sourceType", "selector");
    expect_struct(&kind, "1");
    let report_kind = uses_at(&doc, &format!("{target}/new/type"), Some(false), true)?;
    assert!(
        !report_kind.is_empty(),
        "the decoded value kind has its own proof"
    );
    let unknown = uses_at(&bare, &format!("{target}/new/value/guid"), None, false)?;
    assert!(
        unknown.is_empty(),
        "a bare comparison has no archive authority"
    );
    Ok(())
}

#[test]
fn a_weak_pointer_uses_the_admitted_record_and_physical_owned_route() -> TestResult {
    let before = weak(false);
    let after = weak(true);
    let (doc, _) = reports(&before, &after)?;
    let local = field_path(&doc, "local")?;
    let head = uses_at(&doc, &format!("{local}/old/value/guid"), Some(true), true)?;
    expect_record(&head, 0, "guid", "payload");
    let (_, pointer) = raw_bytes(&before, &head, "slot", "selector")?;
    assert_eq!(
        pointer,
        [0, 0, 0, 0, 2, 0, 0, 0],
        "actual first owned Node, after the two Node Records"
    );
    let node = uses_at(&doc, &format!("{local}/new/value/type"), Some(false), true)?;
    expect_struct(&node, "0");
    assert!(
        node.iter()
            .all(|site| site.pointer("/location/recordIndex").is_none()),
        "the local Node definition is independent of the owner header type"
    );
    let address = format!("{local}/new/value/at");
    assert_eq!(
        doc.pointer(&address),
        Some(&json!("nodes[1]")),
        "actual second local target"
    );
    let path = uses_at(&doc, &address, Some(false), true)?;
    let (_, header) = raw_bytes(&after, &path, "arrayHeader", "selector")?;
    assert_eq!(
        header,
        [2, 0, 0, 0, 2, 0, 0, 0],
        "original owners' class array header"
    );
    let (_, declaration) = raw_bytes(&after, &path, "text", "conversion")?;
    assert_eq!(
        declaration, b"nodes",
        "spelling comes from the actual path segment"
    );
    let external = field_path(&doc, "external")?;
    let guid = uses_at(
        &doc,
        &format!("{external}/new/value/guid"),
        Some(false),
        true,
    )?;
    expect_record(&guid, 2, "guid", "payload");
    assert!(
        origin_at(&doc, &format!("{external}/new/value/at")).is_none(),
        "external Record has no local position"
    );
    Ok(())
}

#[test]
fn a_dangling_reference_keeps_the_caller_word_with_an_incomplete_error() -> TestResult {
    let before = reference(TestValue::Null);
    let after = reference(TestValue::Reference(77));
    let (doc, _) = reports(&before, &after)?;
    let link = field_path(&doc, "link")?;
    assert_eq!(
        doc.pointer(&format!("{link}/new/type")),
        Some(&json!("error")),
        "ordinary error result"
    );
    let evidence = uses_at(&doc, &format!("{link}/new/value"), None, false)?;
    let (_, requested) = raw_bytes(&after, &evidence, "slot", "conversion")?;
    assert_eq!(
        requested,
        [0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 0, 0, 0],
        "actual failed lookup request"
    );
    assert!(
        evidence
            .iter()
            .all(|site| site.pointer("/location/recordIndex").is_none()),
        "no target Record was admitted"
    );
    Ok(())
}

#[test]
fn a_dangling_reference_element_uses_the_array_header_as_selector() -> TestResult {
    let make = |value| {
        DataForgeBuilder::new()
            .structure(TestStruct::new("Owner").array("links", TestType::Reference))
            .record(
                TestRecord::new(1, "Owner", "Owner.One", "owner.xml")
                    .value("links", TestValue::Array(vec![value])),
            )
            .build()
    };
    let before = make(TestValue::Null);
    let after = make(TestValue::Reference(77));
    let (doc, _) = reports(&before, &after)?;
    let link = field_path(&doc, "links[0]")?;
    let path = format!("{link}/new/value");
    assert_eq!(
        doc.pointer(&path),
        Some(&json!(
            "the Reference refers to the Record 00000000-0000-0000-0000-00000000004d, which does not exist"
        )),
        "the original error message is preserved"
    );
    let evidence = uses_at(&doc, &path, None, false)?;
    let (_, header) = raw_bytes(&after, &evidence, "arrayHeader", "selector")?;
    assert_eq!(header, [1, 0, 0, 0, 0, 0, 0, 0], "original array admission");
    assert!(
        evidence.iter().all(|site| {
            site.pointer("/location/field").and_then(Value::as_str) != Some("arrayHeader")
                || site.get("role").and_then(Value::as_str) == Some("selector")
        }),
        "the array header only admits the element and never spells the error"
    );
    let (_, guid) = raw_bytes(&after, &evidence, "arrayElement", "conversion")?;
    assert_eq!(
        guid,
        [0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 0, 0, 0],
        "exact GUID suffix used to render the original error"
    );
    Ok(())
}

fn reference(value: TestValue) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Owner").field("link", TestType::Reference))
        .structure(TestStruct::new("Selected"))
        .structure(TestStruct::new("Decoy"))
        .record(TestRecord::new(1, "Owner", "Owner.One", "owner.xml").value("link", value))
        .record(TestRecord::new(
            2,
            "Selected",
            "Selected.One",
            "selected.xml",
        ))
        .record(TestRecord::new(2, "Decoy", "Decoy.One", "decoy.xml"))
        .build()
}

fn weak(new_side: bool) -> Vec<u8> {
    let nodes = TestValue::Array(vec![
        TestValue::Struct(TestInstance::new("Node").named("first")),
        TestValue::Struct(TestInstance::new("Node").named("second")),
    ]);
    let owner = TestRecord::new(1, "Owner", "Owner.One", "owner.xml")
        .value("nodes", nodes)
        .value(
            "local",
            TestValue::pointer(if new_side { "second" } else { "first" }),
        )
        .value(
            "external",
            TestValue::pointer(if new_side { "Node.Other" } else { "Node.First" }),
        );
    DataForgeBuilder::new()
        .structure(TestStruct::new("Node").field("weight", TestType::Int32))
        .structure(
            TestStruct::new("Owner")
                .array("nodes", TestType::Class("Node".to_owned()))
                .field("local", TestType::WeakPointer("Node".to_owned()))
                .field("external", TestType::WeakPointer("Node".to_owned())),
        )
        .record(owner)
        .record(TestRecord::new(2, "Node", "Node.First", "first.xml"))
        .record(TestRecord::new(3, "Node", "Node.Other", "other.xml"))
        .build()
}

fn expect_record(uses: &[&Value], row: u32, field: &str, role: &str) {
    assert!(
        uses.iter().any(|site| {
            site.pointer("/location/recordIndex") == Some(&json!(row))
                && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && site.get("role").and_then(Value::as_str) == Some(role)
        }),
        "exact original Record {row} {field} used as {role}",
    );
}

fn expect_struct(uses: &[&Value], index: &str) {
    assert!(
        uses.iter().any(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeStructDefinition")
                && site.pointer("/location/indices/0").and_then(Value::as_str) == Some(index)
                && site.pointer("/location/field").and_then(Value::as_str) == Some("name")
                && site.get("role").and_then(Value::as_str) == Some("payload")
        }),
        "exact target structure definition {index}",
    );
}
