// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use crate::forge::{AVENGER_TEXT, dataforge};
use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

#[test]
fn canonical_headers_and_leaves_keep_actual_record_and_pool_sources() -> TestResult {
    let fixture = dataforge(0, 0);
    let install = Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", fixture.clone()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call("get_record", &json!({"guid": AVENGER_TEXT}))?;
    let page = response.get("structuredContent").ok_or("record page")?;
    assert_eq!(
        page.pointer("/records/0/crew"),
        Some(&json!(2)),
        "inherited layout order"
    );
    assert_eq!(
        page.pointer("/records/0/tags/1"),
        Some(&json!("Avenger")),
        "array item"
    );
    for output in [
        "$type",
        "$guid",
        "$path",
        "$name",
        "crew",
        "tags/1",
        "maker/guid",
    ] {
        marker(field(page, &format!("records/0/{output}"))?, true);
    }
    assert_record_site(page, "records/0/maker/guid", "guid", 0)?;
    assert_record_site(page, "records/0/$guid", "guid", 1)?;
    assert_record_site(page, "records/0/maker/path", "fileName", 0)?;
    assert_site(
        page,
        "records/0/maker/type",
        "dataforgeStructDefinition",
        "name",
        &["0"],
    )?;
    let first = source_bytes(page, "records/0/tags/0", "arrayElement", &fixture)?;
    let second = source_bytes(page, "records/0/tags/1", "arrayElement", &fixture)?;
    assert_ne!(
        first, second,
        "each array element uses its original pool word"
    );
    assert_eq!(
        source_bytes(page, "records/0/tags/1", "text", &fixture)?,
        b"Avenger",
        "selected StringTable bytes"
    );
    assert_eq!(
        source_bytes(page, "records/0/crew", "slot", &fixture)?,
        2_i32.to_le_bytes(),
        "direct original scalar word"
    );
    assert_all_addresses(&response)
}

#[test]
fn the_writer_observes_the_actual_reserved_field_wrapper_and_pointer_escaping() -> TestResult {
    let fixture = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Reserved")
                .field("$type", TestType::Int32)
                .field("a/b~c", TestType::String),
        )
        .record(
            TestRecord::new(1, "Reserved", "Reserved.One", "wrapped.xml")
                .value("$type", TestValue::Int32(17))
                .value("a/b~c", TestValue::text("literal")),
        )
        .build();
    let install = crate::forge::install(fixture.clone())?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call("get_record", &json!({"path": "wrapped"}))?;
    let page = response.get("structuredContent").ok_or("wrapped page")?;
    assert_eq!(
        page.pointer("/records/0/$type"),
        Some(&json!("Reserved")),
        "canonical header"
    );
    assert_eq!(
        page.pointer("/records/0/$0/name"),
        Some(&json!("$type")),
        "reserved field wrapper"
    );
    assert_eq!(
        page.pointer("/records/0/$0/value"),
        Some(&json!(17)),
        "authored value"
    );
    assert_eq!(
        page.pointer("/records/0/a~1b~0c"),
        Some(&json!("literal")),
        "RFC 6901 address"
    );
    marker(field(page, "records/0/$0/name")?, false);
    marker(field(page, "records/0/$0/value")?, false);
    marker(field(page, "records/0/a~1b~0c")?, false);
    assert_eq!(
        source_bytes(page, "records/0/$0/value", "slot", &fixture)?,
        17_i32.to_le_bytes(),
        "wrapper value keeps its original property bytes"
    );
    assert_eq!(
        source_bytes(page, "records/0/$0/name", "text", &fixture)?,
        b"$type",
        "wrapper name keeps its original declaration text"
    );
    assert_all_addresses(&response)
}

fn assert_record_site(reply: &Value, output: &str, label: &str, record: u32) -> TestResult {
    assert!(
        uses(reply, output)?.iter().any(|site| {
            site.pointer("/location/type") == Some(&json!("dataForge"))
                && site.pointer("/location/recordIndex") == Some(&json!(record))
                && site.pointer("/location/field") == Some(&json!(label))
        }),
        "{output} must point to original Record {record} field {label}"
    );
    Ok(())
}

fn source_bytes<'a>(
    reply: &Value,
    output: &str,
    label: &str,
    original: &'a [u8],
) -> TestResult<&'a [u8]> {
    let sites = uses(reply, output)?;
    let span = sites
        .iter()
        .find(|site| {
            site.pointer("/location/namespace") == Some(&json!("dataforgeDecodedBytes"))
                && site.pointer("/location/field") == Some(&json!(label))
        })
        .ok_or_else(|| format!("{output} has no original {label} span"))?;
    let at = |address: &str| -> TestResult<usize> {
        Ok(span
            .pointer(address)
            .and_then(Value::as_str)
            .ok_or("source offset")?
            .parse()?)
    };
    original
        .get(at("/location/indices/0")?..at("/location/indices/1")?)
        .ok_or_else(|| format!("invalid original {label} range for {output}").into())
}

#[test]
fn duplicate_fields_keep_distinct_physical_slots_and_the_wrapped_name() -> TestResult {
    let fixture = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Twin")
                .field("same", TestType::Int32)
                .field("same", TestType::Int32),
        )
        .record(
            TestRecord::new(1, "Twin", "Twin.One", "same.xml").value("same", TestValue::Int32(37)),
        )
        .build();
    let install = crate::forge::install(fixture.clone())?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let page = server.ok("get_record", &json!({"path": "same"}))?;
    assert_eq!(
        page.pointer("/records/0/same"),
        Some(&json!(37)),
        "first declaration is unwrapped"
    );
    assert_eq!(
        page.pointer("/records/0/$1/value"),
        Some(&json!(0)),
        "second declaration keeps its zero-filled source value"
    );
    assert_eq!(
        page.pointer("/records/0/$1/name"),
        Some(&json!("same")),
        "wrapped authored name"
    );
    let first = source_bytes(&page, "records/0/same", "slot", &fixture)?;
    let second = source_bytes(&page, "records/0/$1/value", "slot", &fixture)?;
    assert_eq!(first, 37_i32.to_le_bytes(), "first authored word");
    assert_eq!(second, 0_i32.to_le_bytes(), "second authored word");
    assert_ne!(first.as_ptr(), second.as_ptr(), "separate physical slots");
    assert_site(
        &page,
        "records/0/$1/name",
        "dataforgePropertyDefinition",
        "name",
        &["1"],
    )
}

fn class_and_links() -> Vec<u8> {
    let item = |name, count| {
        TestValue::Struct(
            TestInstance::new("Item")
                .named(name)
                .value("count", TestValue::Int32(count)),
        )
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Item").field("count", TestType::Int32))
        .structure(
            TestStruct::new("Holder")
                .array("children", TestType::Class("Item".to_owned()))
                .field("selected", TestType::WeakPointer("Item".to_owned()))
                .array("empty", TestType::Int32)
                .field("invalid", TestType::Reference),
        )
        .record(
            TestRecord::new(1, "Holder", "Holder.One", "holder.xml")
                .tag("test-tag")
                .value(
                    "children",
                    TestValue::Array(vec![item("first", 17), item("second", 23)]),
                )
                .value("selected", TestValue::pointer("second"))
                .value("invalid", TestValue::Reference(0x77)),
        )
        .build()
}

#[test]
fn selected_weak_target_keeps_original_child_slot_without_the_other_child() -> TestResult {
    let fixture = class_and_links();
    let install = crate::forge::install(fixture.clone())?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call("get_record", &json!({"path":"holder"}))?;
    let page = response.get("structuredContent").ok_or("holder page")?;
    assert_eq!(
        page.pointer("/records/0/selected/at"),
        Some(&json!("children[1]")),
        "actual admitted target"
    );
    assert_eq!(
        page.pointer("/records/0/$tag"),
        Some(&json!("test-tag")),
        "canonical optional header"
    );
    assert_site(
        page,
        "records/0/selected/at",
        "dataforgeOwnedInstance",
        "path",
        &["0", "0", "1"],
    )?;
    assert_site(
        page,
        "records/0/selected/type",
        "dataforgeOwnedInstance",
        "path",
        &["0", "0", "1"],
    )?;
    assert!(
        uses(page, "records/0/selected/at")?
            .iter()
            .all(|site| { site.pointer("/location/indices") != Some(&json!(["0", "0", "0"])) }),
        "the other owned occurrence supplied no link target"
    );
    assert_eq!(
        source_bytes(page, "records/0/children/1/count", "slot", &fixture)?,
        23_i32.to_le_bytes(),
        "actual owned class element's original bytes"
    );
    assert_eq!(
        source_bytes(page, "records/0/empty", "arrayHeader", &fixture)?.len(),
        8,
        "the empty list still uses its stored count and first element"
    );
    assert_record_site(page, "records/0/$tag", "tag", 0)?;
    assert!(
        page.pointer("/records/0/invalid/guid").is_some(),
        "original dangling link output"
    );
    marker(field(page, "records/0/invalid/error")?, false);
    marker(field(page, "records/0/invalid/guid")?, false);
    assert_all_addresses(&response)
}

#[test]
fn record_cursor_observes_considered_slots_and_next_page_keeps_the_physical_row() -> TestResult {
    let fixture = dataforge(2, 150 << 10);
    let install = crate::forge::install(fixture)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let path = "libs/foundry/records/notes/long";
    let first = server.ok("get_record", &json!({"path": path}))?;
    let cursor = first.get("next_cursor").ok_or("next page")?;
    assert_eq!(
        first.get("total"),
        Some(&json!(2)),
        "same path matched two rows"
    );
    assert_site(
        &first,
        "total",
        "dataforgeRecordSurvey",
        "fileNameOffsets",
        &["0", "7"],
    )?;
    assert_site(
        &first,
        "next_cursor",
        "dataforgeRecordSurvey",
        "decodedRecordSlots",
        &["5", "6"],
    )?;
    assert_record_site(&first, "records/0/$guid", "guid", 5)?;
    let next = server.ok("get_record", &json!({"path": path, "cursor": cursor}))?;
    assert_record_site(&next, "records/0/$guid", "guid", 6)?;
    assert!(next.get("next_cursor").is_none(), "last page");
    for page in [&first, &next] {
        assert!(
            serde_json::to_vec(page)?.len() <= 256 * 1024,
            "complete reply fits"
        );
    }
    Ok(())
}

#[test]
fn one_small_payload_with_too_many_origins_is_refused_as_a_complete_record() -> TestResult {
    let fixture = DataForgeBuilder::new()
        .structure(TestStruct::new("List").array("items", TestType::Int32))
        .record(TestRecord::new(1, "List", "List.One", "list.xml").value(
            "items",
            TestValue::Array((0..1800).map(TestValue::Int32).collect()),
        ))
        .build();
    let install = crate::forge::install(fixture)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let error = server.error("get_record", &json!({"path": "list"}))?;
    assert!(
        error.contains("response cap"),
        "all field evidence must fit: {error}"
    );
    assert!(
        error.contains("run_query"),
        "the existing narrow-output hint remains available"
    );
    Ok(())
}
