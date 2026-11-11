// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, StructValue, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value as JsonValue;

use crate::mission_property_auto_read;
use crate::mission_property_clue_read;
use crate::mission_property_investigation_read;
use crate::mission_property_name_read;
use crate::mission_property_option_read;
use crate::mission_property_race;
use crate::owned_reference_read;
use crate::record_read::ReadError;

type TestResult = Result<(), ReadError>;

fn source(
    builder: DataForgeBuilder,
    record: TestRecord,
    kind: &str,
    check: impl FnOnce(&StructValue<'_>) -> TestResult,
) -> TestResult {
    let bytes = builder.version(8).record(record).build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("original source Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the actual tree belongs to its original Record"
    );
    assert_eq!(
        tree.root().type_name(),
        kind,
        "the original root declaration"
    );
    check(tree.root())
}

#[test]
fn auto_shapes_accept_genuine_unresolved_references_without_target_materialization() -> TestResult {
    let kind = "MissionPropertyValue_AutoSpawns";
    let child = "SpawnClosetSearchOptions_EntityDeclaration";
    let builder = DataForgeBuilder::new()
        .structure(
            TestStruct::new(kind)
                .field("autoSpawns", TestType::Reference)
                .field("searchOptions", TestType::StrongPointer(child.to_owned())),
        )
        .structure(TestStruct::new(child).field("entityDeclaration", TestType::Reference));
    let record = TestRecord::new(0x8811, kind, "Source", "source.xml")
        .value("autoSpawns", TestValue::Reference(0x8821))
        .value(
            "searchOptions",
            TestValue::Struct(
                TestInstance::new(child).value("entityDeclaration", TestValue::Reference(0x8822)),
            ),
        );
    source(builder, record, kind, check_auto)
}

fn check_auto(root: &StructValue<'_>) -> TestResult {
    assert!(
        matches!(
            root.field("autoSpawns"),
            Some(Value::Error(ValueError::DanglingReference { .. }))
        ),
        "the original unresolved GUID is not a hand-built value"
    );
    let Some(Value::Struct(search)) = root.field("searchOptions") else {
        return Err("original search configuration".into());
    };
    assert_eq!(
        search.type_name(),
        "SpawnClosetSearchOptions_EntityDeclaration",
        "the actual selected search subtype"
    );
    mission_property_auto_read::validate(root)?;
    let mapped = serde_json::to_value(mission_property_auto_read::read(root, None)?)?;
    assert_eq!(
        mapped
            .pointer("/autoSpawns/status")
            .and_then(JsonValue::as_str),
        Some("dangling"),
        "the legacy presentation boundary keeps the same status"
    );
    assert_eq!(
        mapped
            .pointer("/searchOptions/entityDeclaration/status")
            .and_then(JsonValue::as_str),
        Some("dangling"),
        "the selected search target is materialized only by read"
    );
    Ok(())
}

#[test]
fn option_shapes_preserve_metadata_before_payload_failure_order() -> TestResult {
    let kind = "MissionPropertyValueOption_Integer";
    let builder = DataForgeBuilder::new().structure(
        TestStruct::new(kind)
            .field("textId", TestType::Locale)
            .field("weighting", TestType::Bool)
            .field("value", TestType::Int64),
    );
    let record = TestRecord::new(0x8812, kind, "Source", "source.xml")
        .value("textId", TestValue::text("@valid"))
        .value("weighting", TestValue::Bool(false))
        .value("value", TestValue::Int64(12));
    source(builder, record, kind, |root| {
        assert!(
            matches!(root.field("weighting"), Some(Value::Bool(false))),
            "the actual malformed numeric metadata"
        );
        assert!(
            matches!(root.field("value"), Some(Value::Int64(12))),
            "the later wrong-width original payload"
        );
        let error = mission_property_option_read::validate_integer(root, "options[2]")
            .expect_err("the original Bool is not a numeric weighting");
        assert!(
            error.to_string().contains("options[2].weighting"),
            "metadata remains the first field failure"
        );
        Ok(())
    })
}

#[test]
fn investigation_shapes_keep_null_slots_and_clue_subtype_failure_order() -> TestResult {
    let kind = "MissionPropertyValue_Investigation";
    let builder = DataForgeBuilder::new()
        .structure(
            TestStruct::new(kind)
                .array("startingJournalEntries", TestType::Reference)
                .array("suspects", TestType::StrongPointer("Suspect".to_owned())),
        )
        .structure(
            TestStruct::new("Suspect")
                .field("initialSuspect", TestType::Bool)
                .field("isGuilty", TestType::Bool),
        );
    let record = TestRecord::new(0x8813, kind, "Source", "source.xml")
        .value(
            "startingJournalEntries",
            TestValue::Array(vec![TestValue::Null, TestValue::Reference(0x8841)]),
        )
        .value(
            "suspects",
            TestValue::Array(vec![
                TestValue::Null,
                TestValue::Struct(
                    TestInstance::new("Suspect").value("isGuilty", TestValue::Bool(true)),
                ),
            ]),
        );
    source(builder, record, kind, |root| {
        let Some(Value::Array(values)) = root.field("suspects") else {
            return Err("original suspect array".into());
        };
        assert_eq!(values.len(), 2, "both original nullable slots");
        assert!(
            matches!(values.first(), Some(Value::Null)),
            "the first original slot stays null"
        );
        mission_property_investigation_read::validate(root)
    })?;
    clue_failure()
}

fn clue_failure() -> TestResult {
    let kind = "Investigation_ClueItem_Corpse";
    let builder = DataForgeBuilder::new().structure(
        TestStruct::new(kind)
            .field("suspectRepresented", TestType::Int32)
            .field("extra", TestType::Bool),
    );
    let record = TestRecord::new(0x8814, kind, "Source", "source.xml")
        .value("suspectRepresented", TestValue::Int32(7));
    source(builder, record, kind, |root| {
        assert!(
            matches!(root.field("suspectRepresented"), Some(Value::Int32(7))),
            "the original unsupported weak-link shape precedes the extra-field guard"
        );
        let error = mission_property_clue_read::validate_item(root, "clueItems[1]")
            .expect_err("the original Int32 is not a weak link");
        assert!(
            error
                .to_string()
                .contains("clueItems[1].suspectRepresented"),
            "subtype payload validation still precedes additional-field rejection"
        );
        Ok(())
    })
}

#[test]
fn investigation_name_shapes_keep_nested_exact_type_and_single_guards() -> TestResult {
    let kind = "MissionPropertyValue_AIName";
    let builder = DataForgeBuilder::new()
        .structure(TestStruct::new(kind).field("chanceOfNickName", TestType::Double));
    let record = TestRecord::new(0x8815, kind, "Source", "source.xml")
        .value("chanceOfNickName", TestValue::Double(0.25));
    source(builder, record, kind, |root| {
        assert!(
            matches!(root.field("chanceOfNickName"), Some(Value::Double(value)) if value.to_bits() == 0.25_f64.to_bits()),
            "the actual finite Double must not be accepted as Single"
        );
        let error = mission_property_name_read::validate(Some(root), "suspects[0].name")
            .expect_err("the original Double violates the nested Single contract");
        assert!(
            error
                .to_string()
                .contains("suspects[0].name.chanceOfNickName"),
            "the exact nested field error"
        );
        mission_property_name_read::validate(None, "suspects[1].name")?;
        Ok(())
    })?;
    let alias = "AliasMissionPropertyValue_AIName";
    source(
        DataForgeBuilder::new().structure(TestStruct::new(alias)),
        TestRecord::new(0x8818, alias, "Alias", "alias.xml"),
        alias,
        |root| {
            assert_eq!(root.type_name(), alias, "the actual foreign nested type");
            assert!(
                mission_property_name_read::validate(Some(root), "name").is_err(),
                "nested names keep their exact type guard rather than top-level suffix dispatch"
            );
            Ok(())
        },
    )
}

#[test]
fn race_shapes_retain_nullable_slots_and_exact_original_single_width() -> TestResult {
    let kind = "MissionPropertyValue_TimeTrialRace";
    let builder = DataForgeBuilder::new()
        .structure(
            TestStruct::new(kind)
                .array("targetSplits", TestType::StrongPointer("Split".to_owned())),
        )
        .structure(TestStruct::new("Split").field("targetSplit", TestType::Int32));
    let record = TestRecord::new(0x8816, kind, "Source", "source.xml").value(
        "targetSplits",
        TestValue::Array(vec![
            TestValue::Null,
            TestValue::Struct(TestInstance::new("Split").value("targetSplit", TestValue::Int32(5))),
        ]),
    );
    source(builder, record, kind, |root| {
        let Some(Value::Array(values)) = root.field("targetSplits") else {
            return Err("original split array".into());
        };
        assert_eq!(values.len(), 2, "both original split positions");
        assert!(
            matches!(values.first(), Some(Value::Null)),
            "the actual first split is null"
        );
        let Some(Value::Struct(split)) = values.get(1) else {
            return Err("original second split".into());
        };
        assert!(
            matches!(split.field("targetSplit"), Some(Value::Int32(5))),
            "the actual source numeric width"
        );
        let error =
            mission_property_race::validate(root).expect_err("Int32 is not the required Single");
        assert!(
            error.to_string().contains("targetSplits[1].targetSplit"),
            "the null slot does not shift the original error index"
        );
        Ok(())
    })
}

#[test]
fn weak_shape_checks_use_genuine_pointer_error_and_reference_creator_outcomes() -> TestResult {
    let kind = "Owner";
    let builder = DataForgeBuilder::new()
        .structure(
            TestStruct::new(kind)
                .field("weak", TestType::WeakPointer("Target".to_owned()))
                .field("link", TestType::Reference)
                .field("wrong", TestType::Bool),
        )
        .structure(TestStruct::new("Target"));
    let mut pointer = 77_u32.to_le_bytes().to_vec();
    pointer.extend(99_u32.to_le_bytes());
    let record = TestRecord::new(0x8817, kind, "Source", "source.xml")
        .value("weak", TestValue::Raw(pointer))
        .value("link", TestValue::Reference(0x8861));
    source(builder, record, kind, check_weak)
}

fn check_weak(root: &StructValue<'_>) -> TestResult {
    let weak = root.field("weak").ok_or("actual malformed pointer")?;
    assert!(
        matches!(
            weak,
            Value::Error(ValueError::InvalidPointer {
                struct_index: 77,
                instance_index: 99
            })
        ),
        "the genuine decoder retains both authored invalid pointer words"
    );
    let link = root.field("link").ok_or("actual missing Reference")?;
    assert!(
        matches!(link, Value::Error(ValueError::DanglingReference { .. })),
        "the genuine missing GUID outcome"
    );
    assert!(
        owned_reference_read::check(weak).is_ok(),
        "the actual invalid pointer is a supported unresolved weak shape"
    );
    assert!(
        owned_reference_read::check(link).is_ok(),
        "the actual dangling GUID is a supported unresolved weak shape"
    );
    let wrong = root.field("wrong").ok_or("actual Bool")?;
    assert!(
        matches!(wrong, Value::Bool(false)),
        "the original nonpointer Bool"
    );
    assert!(
        owned_reference_read::check(wrong).is_err(),
        "an actual Bool is not silently admitted as a weak link"
    );
    Ok(())
}
