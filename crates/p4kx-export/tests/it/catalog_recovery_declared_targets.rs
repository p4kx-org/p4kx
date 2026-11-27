// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, ExportDiagnosticCode};
use p4kx_extraction::{DataForge, DataForgeError, DataType, Struct, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{ECONOMY, OVERRIDE, economy, finish, overrides};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::TestResult;
use crate::insurance_fixture::{RECOVERY_PATH, build_from, diagnostic_from};

const COOLDOWN: &str = "ItemRecoveryCooldownParams";
const UNRELATED: &str = "UnrelatedRecoveryShape";
const BASE_TIME: f32 = 9.25;

#[derive(Clone, Copy)]
enum Case {
    Cooldown,
    Overrides,
}

impl Case {
    const fn field(self) -> &'static str {
        match self {
            Self::Cooldown => "cooldownParams",
            Self::Overrides => "overrides",
        }
    }

    const fn kind(self) -> DataType {
        match self {
            Self::Cooldown => DataType::StrongPointer,
            Self::Overrides => DataType::Class,
        }
    }

    const fn expected_target(self) -> &'static str {
        match self {
            Self::Cooldown => COOLDOWN,
            Self::Overrides => OVERRIDE,
        }
    }

    fn cell(self, catalog: &Value) -> TestResult<&Value> {
        match self {
            Self::Cooldown => proof::at(economy(catalog)?, "/cooldownParams"),
            Self::Overrides => overrides(catalog),
        }
    }
}

#[test]
fn recovery_rejects_a_nominally_unrelated_populated_strong_pointer() -> TestResult {
    check_rejected(Case::Cooldown)
}

#[test]
fn recovery_rejects_a_nominally_unrelated_empty_class_array() -> TestResult {
    check_rejected(Case::Overrides)
}

#[test]
fn recovery_diagnoses_the_unreadable_original_struct_declaration() -> TestResult {
    let case = Case::Cooldown;
    let mut bytes = fixture(case, UNRELATED).build();
    proof::declaration_fixture::corrupt_definition_name(&mut bytes, UNRELATED)?;
    original_target_in(&bytes, case, |target| {
        assert!(
            matches!(target.name(), Err(DataForgeError::InvalidString { .. })),
            "the physical declared Struct name fails while its pointed-to value is readable"
        );
        Ok(())
    })?;
    let catalog = proof::declaration_fixture::diagnostic_bytes(bytes)?;
    let cell = case.cell(&catalog)?;
    assert_eq!(
        proof::at(cell, "/state")?,
        "unreadable",
        "actual declaration state"
    );
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?,
        &json!(false),
        "the missing declaration proof is incomplete"
    );
    assert!(
        proof::rows(&catalog, "/exportHealth/diagnostics")?
            .iter()
            .any(|row| row.get("code").and_then(Value::as_str) == Some("unreadableSource")),
        "the original declaration read is diagnosed as unreadable"
    );
    proof::closure(&catalog)
}

#[test]
fn recovery_accepts_correctly_declared_populated_pointer_and_empty_array() -> TestResult {
    for case in [Case::Cooldown, Case::Overrides] {
        let fixture = fixture(case, case.expected_target());
        original_target(&fixture, case, case.expected_target())?;
        let (built, catalog) = build_from(&fixture, None)?;
        assert!(
            built.summary().export_health().is_complete(),
            "a compatible authored declaration can publish: {}",
            case.field()
        );
        let cell = case.cell(&catalog)?;
        assert_eq!(
            proof::at(cell, "/state")?,
            "present",
            "an actual compatible field remains present: {}",
            case.field()
        );
        match case {
            Case::Cooldown => assert_eq!(
                proof::at(cell, "/value/baseTime/value/rawBits")?,
                format!("{:08x}", BASE_TIME.to_bits()).as_str(),
                "the actual pointed-to Single is preserved"
            ),
            Case::Overrides => assert_eq!(
                proof::at(cell, "/value")?,
                &json!([]),
                "the actual empty Class array remains present"
            ),
        }
    }
    Ok(())
}

fn check_rejected(case: Case) -> TestResult {
    let fixture = fixture(case, UNRELATED);
    let target = original_target(&fixture, case, UNRELATED)?;
    let ordinary = build_from(&fixture, None);
    assert!(
        matches!(
            ordinary
                .err()
                .and_then(|error| error.downcast::<CatalogError>().ok())
                .as_deref(),
            Some(CatalogError::IncompleteExport(_))
        ),
        "a readable but incompatible original declaration cannot publish: {}",
        case.field()
    );
    let (built, catalog) = diagnostic_from(&fixture, None)?;
    assert!(
        built
            .summary()
            .export_health()
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == ExportDiagnosticCode::UnsupportedValue }),
        "the incompatibility has its own typed source diagnosis"
    );
    let cell = case.cell(&catalog)?;
    assert_eq!(
        proof::at(cell, "/state")?,
        "unsupported",
        "actual declaration"
    );
    assert_eq!(
        proof::at(cell, "/value")?,
        &Value::Null,
        "no misleading payload"
    );
    assert_eq!(
        proof::at(cell, "/containsEncryptedSource")?,
        true,
        "the actual encrypted declaration controls the result"
    );
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?,
        true,
        "all facts behind the readable rejection were retained"
    );
    check_selectors(&catalog, cell, target)?;
    proof::closure(&catalog)
}

fn check_selectors(catalog: &Value, cell: &Value, target: u16) -> TestResult {
    let uses = proof::uses(catalog, cell)?;
    for (namespace, field, indices) in [
        ("dataforgeValue", "$target", json!(["0", "1"])),
        ("dataforgeStruct", "name", json!([target.to_string()])),
    ] {
        assert!(
            uses.iter().any(|used| {
                used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                    && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && used.pointer("/location/indices") == Some(&indices)
                    && used.get("role").and_then(Value::as_str) == Some("selector")
            }),
            "actual {namespace} declaration selector {field} at {indices}"
        );
    }
    Ok(())
}

fn original_target(fixture: &DataForgeBuilder, case: Case, expected: &str) -> TestResult<u16> {
    original_target_in(&fixture.build(), case, |target| {
        assert_eq!(target.name()?, expected, "the stored nominal target");
        Ok(())
    })
}

fn original_target_in(
    bytes: &[u8],
    case: Case,
    check: impl FnOnce(Struct<'_>) -> TestResult,
) -> TestResult<u16> {
    let forge = DataForge::open(bytes)?;
    let record = forge.record(0).ok_or("physical recovery Record")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual selected physical Record");
    assert_eq!(tree.record_path(), RECOVERY_PATH, "policy selection path");
    assert_eq!(tree.error_count(), 0, "no canonical Record read failure");
    let Some(Raw::Struct(economy)) = tree.field("economyParams") else {
        return Err("original economy block".into());
    };
    let field = economy
        .fields()
        .iter()
        .find(|field| field.name() == case.field())
        .ok_or("original field declaration")?;
    let storage = field.storage();
    assert_eq!(storage.data_type(), case.kind(), "actual storage");
    assert_eq!(
        storage.is_array(),
        matches!(case, Case::Overrides),
        "actual shape"
    );
    let target = forge
        .structs()
        .nth(usize::from(storage.target()))
        .ok_or("actual declared target index")?;
    check(target)?;
    match case {
        Case::Cooldown => {
            let Raw::Struct(block) = field.value() else {
                return Err("actual populated original pointer".into());
            };
            assert_eq!(block.type_name(), COOLDOWN, "the real concrete target");
            assert!(
                matches!(block.field("baseTime"), Some(Raw::Single(value)) if value.to_bits() == BASE_TIME.to_bits()),
                "the original pointed-to value is readable"
            );
        }
        Case::Overrides => assert!(
            matches!(field.value(), Raw::Array(elements) if elements.is_empty()),
            "the actual array has no element with which to validate its declaration"
        ),
    }
    Ok(storage.target())
}

fn fixture(case: Case, declared: &str) -> DataForgeBuilder {
    let target = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(UNRELATED))
        .structure(TestStruct::new(OVERRIDE))
        .structure(TestStruct::new(COOLDOWN).field("baseTime", TestType::Single))
        .instance(
            "actualCooldown",
            TestInstance::new(COOLDOWN).value("baseTime", TestValue::Single(BASE_TIME)),
        );
    let field = case.field();
    let (shape, value) = match case {
        Case::Cooldown => (
            TestStruct::new(ECONOMY).field(field, TestType::StrongPointer(declared.to_owned())),
            TestValue::pointer("actualCooldown"),
        ),
        Case::Overrides => (
            TestStruct::new(ECONOMY).array(field, TestType::Class(declared.to_owned())),
            TestValue::Array(Vec::new()),
        ),
    };
    finish(
        target,
        shape,
        TestInstance::new(ECONOMY).value(field, value),
    )
}
