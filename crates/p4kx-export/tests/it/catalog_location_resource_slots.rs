// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

const TAG_FIELD: &str = "resourceSlotTag";

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

const SLOT_TYPE: &str = "LocationResourceSlot";
const SLOT: u128 = 41;
const ALT_SLOT: u128 = 42;
const TAG: u128 = 43;
const MISSING: u128 = 44;
const COUNT: &str = "numberOfResourceSlotsPerEntity";
const SLOTS: &str = "/definitions/locationResourceSlots/rows";

#[derive(Clone, Copy)]
enum Layout {
    Tag,
    Count,
    DuplicateGuid,
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

    let current = export(Layout::Count, None)?;
    let current_rows = at(&current, SLOTS)?;
    for (index, expected) in [0, -3].into_iter().enumerate() {
        let row = at(current_rows, &format!("/{index}"))?;
        let count = at(row, "/numberOfResourceSlotsPerEntity")?;
        assert_eq!(at(count, "/value")?, expected, "raw signed Int32");
        assert_eq!(at(count, "/containsEncryptedSource")?, true, "count origin");
        assert_eq!(
            at(row, "/resourceSlotTag/state")?,
            "absent",
            "current Tag declaration removed"
        );
        assert_eq!(
            at(row, "/authoredFields/roots")?.as_array().map(Vec::len),
            Some(1),
            "only the unprojected original member stays in the arena"
        );
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
    let diagnostics = at(&diagnostic, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics array")?;
    assert!(
        diagnostics.iter().any(|row| row["kind"] == "locationResourceSlots"
            && row["code"] == "ambiguousIdentity"),
        "the canonical slot Kind diagnoses duplicate physical identities"
    );
    Ok(())
}

fn export(layout: Layout, filter: Option<&Filter>) -> TestResult<Value> {
    let (_directory, archive) = common::open(&database(layout), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    if matches!(layout, Layout::DuplicateGuid) {
        options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    }
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn database(layout: Layout) -> DataForgeBuilder {
    let slot = TestStruct::new(SLOT_TYPE).field("fixtureUnknown", TestType::Bool);
    let (slot, member, first, second) = match layout {
        Layout::Tag => (
            slot.field(TAG_FIELD, TestType::ReferenceTo("Tag".into())),
            TAG_FIELD,
            TestValue::Reference(TAG),
            TestValue::Null,
        ),
        Layout::Count | Layout::DuplicateGuid => (
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
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Tag"))
        .structure(slot)
        .record(TestRecord::new(
            TAG,
            "Tag",
            "FixtureTag",
            "fixture/tags/tag.xml",
        ))
        .record(
            TestRecord::new(SLOT, SLOT_TYPE, "First", "fixture/slots/first.xml")
                .value(member, first),
        )
        .record(
            TestRecord::new(second_guid, SLOT_TYPE, "Second", "fixture/slots/second.xml")
                .value(member, second),
        )
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
