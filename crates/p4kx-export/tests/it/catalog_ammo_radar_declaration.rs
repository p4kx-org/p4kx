// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, DataType, Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const ORIGINAL_FIELD: &str = "radarObjectParams";
const OUTPUT_FIELD: &str = "radarParams";
const EXPECTED: &str = "SSCSignatureSystemParams";
const DIFFERENT: &str = "FixtureOtherRadarTarget";
const INVALID_STRUCT_INDEX: u32 = u32::MAX - 1;

#[test]
fn radar_wrong_declared_target_keeps_selectors_without_unconsulted_payload() -> TestResult {
    let value =
        TestValue::Struct(TestInstance::new(DIFFERENT).value("readable", TestValue::Bool(true)));
    let (_folder, archive) = common::open(&database(DIFFERENT, value), DcbOrigin::Encrypted)?;
    with_original(&archive, DIFFERENT, |original| {
        assert!(
            matches!(original, Raw::Struct(value) if value.type_name() == DIFFERENT),
            "the original pointer reads a real Struct in the other declared family"
        );
    })?;
    let catalog = diagnostic(&archive)?;
    let cell = proof::cell(ammo(&catalog)?, OUTPUT_FIELD, "unsupported")?;
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?,
        &json!(true),
        "a readable declaration is sufficient to reject the field"
    );
    let uses = proof::uses(&catalog, cell)?;
    assert!(
        uses.iter()
            .any(|used| { has_use(used, "$target", "selector") }),
        "the original target declaration selected this rejection"
    );
    assert!(
        uses.iter().all(|used| {
            !matches!(
                used.pointer("/location/field").and_then(Value::as_str),
                Some("$value" | "$type")
            )
        }),
        "a rejected declaration cannot attribute the unconsulted payload or concrete type"
    );
    proof::closure(&catalog)
}

#[test]
fn radar_correct_declaration_with_failed_pointer_marks_the_actual_attempt() -> TestResult {
    let value = TestValue::Raw([INVALID_STRUCT_INDEX.to_le_bytes(), 0_u32.to_le_bytes()].concat());
    let (_folder, archive) = common::open(&database(EXPECTED, value), DcbOrigin::Plain)?;
    with_original(&archive, EXPECTED, |original| {
        assert!(
            matches!(
                original,
                Raw::Error(ValueError::InvalidPointer { struct_index, instance_index })
                    if *struct_index == INVALID_STRUCT_INDEX && *instance_index == 0
            ),
            "the original payload has the expected unreadable pointer words"
        );
    })?;
    let catalog = diagnostic(&archive)?;
    let cell = proof::cell(ammo(&catalog)?, OUTPUT_FIELD, "unreadable")?;
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?,
        &json!(false),
        "the readable declaration does not make the selected pointer read complete"
    );
    assert_eq!(
        proof::at(cell, "/containsEncryptedSource")?,
        &Value::Null,
        "an incomplete plain-source proof is unknown"
    );
    let uses = proof::uses(&catalog, cell)?;
    let declaration = uses
        .iter()
        .find(|used| has_use(used, "$target", "selector"))
        .ok_or("the actual declaration explains admission")?;
    assert!(
        uses.iter().all(|used| used["role"] != "attempted"),
        "the failed read is outside the contributing sources"
    );
    assert!(
        proof::rows(&catalog, "/provenance/uses")?
            .iter()
            .any(|used| {
                has_use(used, "$value", "attempted")
                    && used["source"] == declaration["source"]
                    && used["location"]["namespace"] == "dataforgeValue"
                    && used["location"]["indices"] == declaration["location"]["indices"]
            }),
        "the inventory retains the attempted read at the first original field"
    );
    proof::closure(&catalog)
}

fn has_use(used: &Value, field: &str, role: &str) -> bool {
    used.pointer("/location/field").and_then(Value::as_str) == Some(field)
        && used.get("role").and_then(Value::as_str) == Some(role)
}

fn ammo(catalog: &Value) -> TestResult<&Value> {
    proof::at(catalog, "/definitions/ammoDefinitions/rows/0")
}

fn with_original(
    archive: &Archive,
    expected_target: &str,
    check: impl FnOnce(&Raw<'_>),
) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let tree = forge.record(0).ok_or("original Ammo Record")?.tree()?;
    let field = tree
        .fields()
        .iter()
        .find(|field| field.name() == ORIGINAL_FIELD)
        .ok_or("original radar declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "the original root is a StrongPointer in both cases"
    );
    let declaration = forge
        .structs()
        .nth(usize::from(field.storage().target()))
        .ok_or("original target Struct declaration")?;
    assert_eq!(
        declaration.name()?,
        expected_target,
        "the fixture independently reads the target declaration before the projected branch"
    );
    check(field.value());
    Ok(())
}

fn diagnostic(archive: &Archive) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn database(target: &str, value: TestValue) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field(ORIGINAL_FIELD, TestType::StrongPointer(target.to_owned())),
        )
        .structure(TestStruct::new(EXPECTED))
        .structure(TestStruct::new(DIFFERENT).field("readable", TestType::Bool))
        .record(
            TestRecord::new(
                0x8280,
                "AmmoParams",
                "AmmoParams.RadarDeclarationFixture",
                "fixture/ammo_radar_declaration.xml",
            )
            .value(ORIGINAL_FIELD, value),
        )
}
