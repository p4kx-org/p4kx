// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    DataType, FieldOrigin, SourceInventory, SourceLocation, SourceUseRole, SourceValueState, Value,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{
    AuthoredAmmoInputs, BASE_CARGO_UNIT, BASE_PHYSICS, Observed, WRAPPER, physics_types, pointer,
};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_observation::RecordObservation;
use crate::source_fields::{FieldError, FieldIssue};
use crate::template_field_capture::TemplateFieldCapture;

const PROJECTILE: &str = "ProjectileParams";
const PROXIMITY: &str = "ProjectileProximityTriggerParams";

#[derive(Clone, Copy)]
enum AmmoField {
    Conversion,
    Physics,
    Projectile,
}

impl AmmoField {
    const fn name(self) -> &'static str {
        match self {
            Self::Conversion => "conversionRate",
            Self::Physics => "physicsControllerParams",
            Self::Projectile => "projectileParams",
        }
    }

    const fn target(self) -> &'static str {
        match self {
            Self::Conversion => BASE_CARGO_UNIT,
            Self::Physics => WRAPPER,
            Self::Projectile => PROJECTILE,
        }
    }

    const fn projected<'view>(
        self,
        output: &'view AuthoredAmmoInputs<'_>,
    ) -> (SourceValueState, &'view FieldOrigin) {
        match self {
            Self::Conversion => (
                output.conversion_rate.state(),
                output.conversion_rate.origin(),
            ),
            Self::Physics => (
                output.physics_controller_params.state(),
                output.physics_controller_params.origin(),
            ),
            Self::Projectile => (
                output.projectile_params.state(),
                output.projectile_params.origin(),
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum Storage {
    StrongPointer,
    Class,
}

impl Storage {
    const fn data_type(self) -> DataType {
        match self {
            Self::StrongPointer => DataType::StrongPointer,
            Self::Class => DataType::Class,
        }
    }

    fn test_type(self, target: &str) -> TestType {
        match self {
            Self::StrongPointer => pointer(target),
            Self::Class => TestType::Class(target.to_owned()),
        }
    }

    const fn state(self) -> SourceValueState {
        match self {
            Self::StrongPointer => SourceValueState::Present,
            Self::Class => SourceValueState::Invalid,
        }
    }
}

#[test]
fn ammo_scalar_storage_keeps_the_three_original_strong_pointers() -> TestResult {
    check(
        Storage::StrongPointer,
        &[
            AmmoField::Conversion,
            AmmoField::Physics,
            AmmoField::Projectile,
        ],
    )
}

#[test]
fn ammo_scalar_storage_rejects_same_type_inline_conversion() -> TestResult {
    check(Storage::Class, &[AmmoField::Conversion])
}

#[test]
fn ammo_scalar_storage_rejects_same_type_inline_physics_controller() -> TestResult {
    check(Storage::Class, &[AmmoField::Physics])
}

#[test]
fn ammo_scalar_storage_rejects_same_type_inline_projectile() -> TestResult {
    check(Storage::Class, &[AmmoField::Projectile])
}

fn check(storage: Storage, expected_fields: &[AmmoField]) -> TestResult {
    test_support::with_input(&bytes(storage), |forge, input| {
        let record = forge.record(0).ok_or("original Ammo Record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let originals = OriginalFlowFields::read(&scope, None)?;
        let capture = TemplateFieldCapture::new(&originals, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let mut notes = Vec::new();
        let output = AuthoredAmmoInputs::read(
            Observed::definition(&originals, input, &capture, &numbers),
            &mut notes,
        )?;
        let inventory = scope.sources().freeze()?;
        for &field in expected_fields {
            let position = original_storage(&originals, field, storage)?;
            let (state, origin) = field.projected(&output);
            assert_eq!(
                state,
                storage.state(),
                "{} follows original scalar storage",
                field.name()
            );
            check_origin(origin, &inventory, &position);
            check_failure(&notes, field, storage)?;
        }
        Ok(())
    })
}

fn original_storage(
    originals: &OriginalFlowFields<'_, '_, '_>,
    field: AmmoField,
    storage: Storage,
) -> TestResult<SourceLocation> {
    let (_, fact) = originals
        .field(None, field.name())?
        .ok_or("original scalar field")?;
    let declaration = fact.field().storage();
    assert_eq!(
        declaration.data_type(),
        storage.data_type(),
        "{} reads the actual stored declaration",
        field.name()
    );
    assert!(
        !declaration.is_array(),
        "{} is a scalar occurrence",
        field.name()
    );
    let Value::Struct(value) = fact.value() else {
        return Err(format!("{} must materialize as an original Struct", field.name()).into());
    };
    assert_eq!(
        value.type_name(),
        field.target(),
        "both stores have the same concrete name"
    );
    Ok(fact.location(OriginalValuePart::DataType))
}

fn check_origin(origin: &FieldOrigin, inventory: &SourceInventory, position: &SourceLocation) {
    assert!(
        inventory.uses().iter().any(|observed| {
            observed.location() == position
                && observed.role() == SourceUseRole::Selector
                && origin.source_uses_refs().contains(observed.id())
        }),
        "the field cites its actual data-type selector {position:?}"
    );
    assert!(
        origin.source_set_complete(),
        "the storage decision is fully observed"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "the original fixture is a verified plain Entry"
    );
}

fn check_failure(notes: &[FieldError], field: AmmoField, storage: Storage) -> TestResult {
    let failure = notes
        .iter()
        .find(|note| note.description().field == field.name());
    match storage {
        Storage::StrongPointer => assert!(failure.is_none(), "valid storage is readable"),
        Storage::Class => {
            let note = failure.ok_or("the exact malformed scalar has a diagnostic")?;
            assert!(
                matches!(note.description().issue, FieldIssue::Malformed),
                "a readable but wrong declaration is malformed"
            );
            assert!(
                note.body().is_some(),
                "the original offending site is retained"
            );
        }
    }
    Ok(())
}

fn bytes(storage: Storage) -> Vec<u8> {
    let ammo = TestStruct::new("AmmoParams")
        .field("conversionRate", storage.test_type(BASE_CARGO_UNIT))
        .field("physicsControllerParams", storage.test_type(WRAPPER))
        .field("projectileParams", storage.test_type(PROJECTILE));
    let physics = TestInstance::new(WRAPPER).value(
        "PhysType",
        TestValue::Struct(TestInstance::new(BASE_PHYSICS).value("Mass", TestValue::Single(1.0))),
    );
    let projectile = TestInstance::new(PROJECTILE).value(
        "proximityTriggerParams",
        TestValue::Struct(TestInstance::new(PROXIMITY).value("radius", TestValue::Single(1.0))),
    );
    physics_types(
        DataForgeBuilder::new()
            .version(8)
            .structure(ammo)
            .structure(TestStruct::new(BASE_CARGO_UNIT))
            .structure(
                TestStruct::new(PROJECTILE).field("proximityTriggerParams", pointer(PROXIMITY)),
            )
            .structure(TestStruct::new(PROXIMITY).field("radius", TestType::Single)),
    )
    .record(
        TestRecord::new(
            0x8121,
            "AmmoParams",
            "AmmoParams.ScalarStorage",
            "fixture/scalar_storage.xml",
        )
        .value(
            "conversionRate",
            TestValue::Struct(TestInstance::new(BASE_CARGO_UNIT)),
        )
        .value("physicsControllerParams", TestValue::Struct(physics))
        .value("projectileParams", TestValue::Struct(projectile)),
    )
    .build()
}
