// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

const SLOT_TYPE: &str = "LocationResourceSlot";
const SLOT: u128 = 41;
const ALT_SLOT: u128 = 42;
const TAG: u128 = 43;
const MISSING: u128 = 44;
const TAG_FIELD: &str = "resourceSlotTag";
const COUNT: &str = "numberOfResourceSlotsPerEntity";
const AUTO_CONSUME: &str = "autoConsumeIfPlayerAtLocation";
const SLOTS: &str = "/definitions/locationResourceSlots/rows";

#[derive(Clone, Copy)]
enum Layout {
    Tag,
    Count,
    DuplicateGuid,
    IncompatibleFlag,
}

#[test]
fn slot_definitions_keep_both_authored_layouts_and_member_origins() -> TestResult {
    let legacy = export(Layout::Tag, None)?;
    let legacy_rows = at(&legacy, SLOTS)?;
    let tag = at(legacy_rows, "/0/resourceSlotTag/value")?;
    assert_eq!(
        at(tag, "/guid/value")?,
        &json!(guid(TAG)),
        "legacy Tag identity"
    );
    assert_eq!(
        at(tag, "/targetRecordIndex/value")?,
        0,
        "physical Tag owner"
    );
    assert_eq!(
        at(legacy_rows, "/1/resourceSlotTag/state")?,
        "null",
        "explicit null Tag"
    );
    assert_eq!(
        at(legacy_rows, "/0/numberOfResourceSlotsPerEntity/state")?,
        "absent",
        "legacy schema lacks the count"
    );

    assert_eq!(
        at(legacy_rows, "/0/autoConsumeIfPlayerAtLocation/state")?,
        "absent",
        "legacy schema lacks the consumption flag"
    );

    let current = export(Layout::Count, None)?;
    let current_rows = at(&current, SLOTS)?;
    for (index, (expected_count, expected_flag)) in [(0, false), (-3, true)].into_iter().enumerate()
    {
        let row = at(current_rows, &format!("/{index}"))?;
        verify_current_slot_row(row, expected_count, expected_flag)?;
    }
    for catalog in [legacy, current] {
        let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
        if let Err(error) = validator.validate(&catalog) {
            return Err(format!("{}: {}", error.instance_path(), error).into());
        }
        crate::commodity_v3_source_inventory::reference_closure(&catalog)?;
    }
    Ok(())
}

fn verify_current_slot_row(row: &Value, expected_count: i32, expected_flag: bool) -> TestResult {
    let auto_consume = at(row, "/autoConsumeIfPlayerAtLocation")?;
    assert_eq!(
        at(auto_consume, "/state")?,
        "present",
        "authored Bool state"
    );
    assert_eq!(
        at(auto_consume, "/value")?,
        expected_flag,
        "authored boolean retains false and true"
    );
    assert_eq!(
        at(auto_consume, "/containsEncryptedSource")?,
        true,
        "flag origin follows the encrypted DataForge input"
    );
    assert_eq!(
        at(auto_consume, "/sourceSetComplete")?,
        true,
        "complete original flag evidence"
    );
    let count = at(row, "/numberOfResourceSlotsPerEntity")?;
    assert_eq!(at(count, "/value")?, expected_count, "raw signed Int32");
    assert_eq!(at(count, "/containsEncryptedSource")?, true, "count origin");
    assert_eq!(
        at(row, "/resourceSlotTag/state")?,
        "absent",
        "current Tag declaration removed"
    );
    let roots = at(row, "/authoredFields/roots")?
        .as_array()
        .ok_or("original arena roots")?;
    let [root] = roots.as_slice() else {
        return Err("only the unprojected original member stays in the arena".into());
    };
    assert_eq!(
        at(root, "/name/value")?,
        "fixtureUnknown",
        "the unrelated original member survives projection"
    );
    Ok(())
}

#[test]
fn incompatible_consumption_flag_retains_its_original_storage_and_value() -> TestResult {
    let catalog = export(Layout::IncompatibleFlag, None)?;
    let row = at(at(&catalog, SLOTS)?, "/0")?;
    let cell = at(row, "/autoConsumeIfPlayerAtLocation")?;
    assert_eq!(
        at(cell, "/state")?,
        "invalid",
        "Int32 is incompatible with the authored Bool"
    );
    assert_eq!(
        at(cell, "/value")?,
        &Value::Null,
        "invalid storage has no typed Bool payload"
    );
    assert_eq!(
        at(cell, "/containsEncryptedSource")?,
        true,
        "invalid cell still cites its original source"
    );
    let roots = at(row, "/authoredFields/roots")?
        .as_array()
        .ok_or("original arena roots")?;
    let root = roots
        .iter()
        .find(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(AUTO_CONSUME))
        .ok_or("incompatible original arena field")?;
    let value_index = at(root, "/value/valueIndex")?
        .as_str()
        .ok_or("arena value index")?;
    let original = at(
        row,
        &format!("/authoredFields/values/{value_index}/value/value/value"),
    )?;
    assert_eq!(
        original,
        &json!({"type": "int32", "value": -4}),
        "the raw arena retains the incompatible signed value"
    );
    Ok(())
}

#[test]
fn module_slots_keep_shared_filtered_and_dangling_physical_links() -> TestResult {
    let complete = export(Layout::Count, None)?;
    let filtered = export(Layout::Count, Some(&Filter::new("fixture/modules/")))?;
    for (catalog, expected_slots) in [(&complete, 2), (&filtered, 0)] {
        let modules = at(catalog, "/definitions/missionModuleDeclarations/rows")?;
        for index in 0..2 {
            let link = at(
                modules,
                &format!("/{index}/moduleType/value/requiredResourceSlot/value"),
            )?;
            assert_eq!(
                at(link, "/guid/value")?,
                &json!(guid(SLOT)),
                "shared source GUID"
            );
            assert_eq!(
                at(link, "/targetRecordIndex/value")?,
                1,
                "shared physical target index"
            );
            assert_eq!(
                at(link, "/status")?,
                "present",
                "source target still resolves"
            );
        }
        let dangling = at(modules, "/2/moduleType/value/requiredResourceSlot/value")?;
        assert_eq!(
            at(dangling, "/status")?,
            "dangling",
            "absent physical target"
        );
        assert_eq!(
            at(dangling, "/guid/value")?,
            &json!(guid(MISSING)),
            "original dangling GUID"
        );
        assert_eq!(
            at(catalog, SLOTS)?.as_array().map(Vec::len),
            Some(expected_slots),
            "normal filter selects owners independently from references"
        );
    }
    let unresolved = at(&filtered, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved References array")?;
    assert!(
        unresolved.iter().any(|entry| {
            entry["status"] == "excludedByFilter"
                && entry.pointer("/targetGuid/value") == Some(&json!(guid(SLOT)))
        }),
        "filter marks the canonical target selection, preserving source identity"
    );
    Ok(())
}

#[test]
fn duplicate_slot_guids_keep_distinct_physical_headers() -> TestResult {
    let diagnostic = export(Layout::DuplicateGuid, None)?;
    let rows = at(&diagnostic, SLOTS)?;
    assert_eq!(at(rows, "/0/guid/value")?, &json!(guid(SLOT)), "first GUID");
    assert_eq!(
        at(rows, "/1/guid/value")?,
        &json!(guid(SLOT)),
        "duplicate GUID"
    );
    assert_ne!(
        at(rows, "/0/recordIndex/value")?,
        at(rows, "/1/recordIndex/value")?,
        "the physical occurrences remain distinct"
    );
    let collision = slot_alias_diagnostic(&diagnostic)?
        .ok_or("the canonical slot Kind diagnoses duplicate physical identities")?;
    assert_eq!(at(collision, "/severity")?, "information");
    assert_eq!(at(&diagnostic, "/exportHealth/status")?, "complete");
    assert_eq!(at(&diagnostic, "/exportHealth/publicationMode")?, "normal");
    let examples = at(collision, "/examples")?
        .as_array()
        .ok_or("two diagnostic examples")?;
    let positions: Vec<_> = examples
        .iter()
        .map(|example| at(example, "/recordIndex/value"))
        .collect::<TestResult<_>>()?;
    assert_eq!(
        positions,
        [&json!(1), &json!(2)],
        "both physical alias owners"
    );
    assert_eq!(at(collision, "/count")?, 2, "one diagnostic unit per owner");
    assert_slot_links_remain_bound(&diagnostic)
}

fn slot_alias_diagnostic(catalog: &Value) -> TestResult<Option<&Value>> {
    let diagnostics = at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics array")?;
    Ok(diagnostics.iter().find(|row| {
        row["kind"] == "locationResourceSlots"
            && row["stage"] == "projection"
            && row["code"] == "ambiguousIdentity"
    }))
}

fn assert_slot_links_remain_bound(catalog: &Value) -> TestResult {
    let references = at(catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved References array")?;
    assert!(
        references.iter().all(|entry| {
            entry["status"] != "ambiguousIdentity"
                || entry.pointer("/targetGuid/value") != Some(&json!(guid(SLOT)))
        }),
        "a physical module target is independent of GUID-only alias diagnostics"
    );
    Ok(())
}

#[test]
fn a_single_selected_slot_has_no_duplicate_owner_diagnostic() -> TestResult {
    let isolated = export(
        Layout::DuplicateGuid,
        Some(&Filter::new("fixture/slots/first")),
    )?;
    assert_eq!(
        at(&isolated, SLOTS)?.as_array().map(Vec::len),
        Some(1),
        "the filter selects exactly one physical slot owner"
    );
    assert!(
        slot_alias_diagnostic(&isolated)?.is_none(),
        "an excluded alias does not create an owner collision"
    );
    Ok(())
}

fn export(layout: Layout, filter: Option<&Filter>) -> TestResult<Value> {
    let (_directory, archive) = common::open(&database(layout), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    if matches!(layout, Layout::IncompatibleFlag) {
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    let built = build_catalog(&archive, options.with_full_provenance())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn add_slots(layout: Layout, builder: DataForgeBuilder) -> DataForgeBuilder {
    let slot = TestStruct::new(SLOT_TYPE).field("fixtureUnknown", TestType::Bool);
    let (slot, member, first, second) = match layout {
        Layout::Tag => (
            slot.field(TAG_FIELD, TestType::ReferenceTo("Tag".into())),
            TAG_FIELD,
            TestValue::Reference(TAG),
            TestValue::Null,
        ),
        Layout::Count | Layout::DuplicateGuid | Layout::IncompatibleFlag => (
            slot.field(COUNT, TestType::Int32),
            COUNT,
            TestValue::Int32(0),
            TestValue::Int32(-3),
        ),
    };
    let second_guid = if matches!(layout, Layout::DuplicateGuid) {
        SLOT
    } else {
        ALT_SLOT
    };
    let first =
        TestRecord::new(SLOT, SLOT_TYPE, "First", "fixture/slots/first.xml").value(member, first);
    let second = TestRecord::new(second_guid, SLOT_TYPE, "Second", "fixture/slots/second.xml")
        .value(member, second);
    let (slot, first, second) = match layout {
        Layout::Tag => (slot, first, second),
        Layout::Count | Layout::DuplicateGuid => (
            slot.field(AUTO_CONSUME, TestType::Bool),
            first.value(AUTO_CONSUME, TestValue::Bool(false)),
            second.value(AUTO_CONSUME, TestValue::Bool(true)),
        ),
        Layout::IncompatibleFlag => (
            slot.field(AUTO_CONSUME, TestType::Int32),
            first.value(AUTO_CONSUME, TestValue::Int32(-4)),
            second.value(AUTO_CONSUME, TestValue::Int32(2)),
        ),
    };
    builder.structure(slot).record(first).record(second)
}

fn database(layout: Layout) -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Tag"))
        .record(TestRecord::new(
            TAG,
            "Tag",
            "FixtureTag",
            "fixture/tags/tag.xml",
        ));
    add_slots(layout, builder)
        .structure(TestStruct::new("ModuleDeclarationType_Base"))
        .structure(
            TestStruct::new("ModuleDeclarationType_Mission")
                .parent("ModuleDeclarationType_Base")
                .field(
                    "requiredResourceSlot",
                    TestType::ReferenceTo(SLOT_TYPE.into()),
                ),
        )
        .structure(TestStruct::new("ModuleDeclaration").field(
            "moduleType",
            TestType::StrongPointer("ModuleDeclarationType_Base".into()),
        ))
        .record(module(51, "first", SLOT))
        .record(module(52, "second", SLOT))
        .record(module(53, "missing", MISSING))
}

fn module(id: u128, name: &str, target: u128) -> TestRecord {
    TestRecord::new(
        id,
        "ModuleDeclaration",
        name,
        format!("fixture/modules/{name}.xml"),
    )
    .value(
        "moduleType",
        TestValue::Struct(
            TestInstance::new("ModuleDeclarationType_Mission")
                .value("requiredResourceSlot", TestValue::Reference(target)),
        ),
    )
}
