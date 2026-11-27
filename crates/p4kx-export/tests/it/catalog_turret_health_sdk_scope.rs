// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::component_definitions::TurretHealthModifier;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ResourceFloat32,
};
use p4kx_extraction::{DataForge, Guid};
use p4kx_testkit::DataForgeBuilder;

use super::{DEFINITION, DERIVED, DERIVED_OWNER, DERIVED_TARGET, MISSING, MODIFIER, OWNER};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::sdk_full_proof as full_proof;

const ENTITY_TYPE: &str = "EntityClassDefinition";

#[test]
fn base_guid_and_empty_selections_skip_turret_definition_discovery() -> TestResult {
    let fixture = UnclassifiableOwners::new()?;
    let (_folder, archive) = common::open_bytes(fixture.bytes, DcbOrigin::Encrypted)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let [first_definition, _] = fixture.base_definitions;
    let [_, unclassifiable_owner] = fixture.owners;
    let cases: [(CatalogRecordSelection, &[u32], &[u32]); 4] = [
        (
            CatalogRecordSelection::Guid(guid(DEFINITION)?),
            &fixture.base_definitions,
            &[],
        ),
        (
            CatalogRecordSelection::RecordIndex(first_definition),
            std::slice::from_ref(&first_definition),
            &[],
        ),
        (CatalogRecordSelection::Guid(guid(MISSING)?), &[], &[]),
        (
            CatalogRecordSelection::Guid(guid(DERIVED_OWNER)?),
            &[],
            std::slice::from_ref(&unclassifiable_owner),
        ),
    ];
    for (selection, physical_indices, unreadable_headers) in cases {
        parsed.with_turret_health_modifiers(selection, None, |snapshot| {
            let actual: Option<Vec<_>> = snapshot
                .rows()
                .iter()
                .map(|row| row.source().record_index().value().copied())
                .collect();
            assert_eq!(
                actual.as_deref(),
                Some(physical_indices),
                "{selection:?} retains the selected physical aliases in source order"
            );
            assert_eq!(
                snapshot.coverage().unreadable_headers.as_slice(),
                unreadable_headers,
                "{selection:?} retains an unclassifiable selected header"
            );
            assert!(
                snapshot.definition_discovery().is_complete(),
                "{selection:?} requires no survey of potentially referencing owners"
            );
        })?;
    }
    Ok(())
}

#[test]
fn derived_modifier_scope_reports_unclassifiable_turret_dependency_headers() -> TestResult {
    let fixture = UnclassifiableOwners::new()?;
    let (_folder, archive) = common::open_bytes(fixture.bytes, DcbOrigin::Encrypted)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    for selection in [
        CatalogRecordSelection::RecordIndex(fixture.derived_definition),
        CatalogRecordSelection::Guid(guid(DERIVED_TARGET)?),
    ] {
        parsed.with_turret_health_modifiers(selection, None, |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "the derived definition needs a compatible observed incoming reference"
            );
            assert!(
                snapshot.coverage().unreadable_headers.is_empty(),
                "the selected definition header remains readable"
            );
            let discovery = snapshot.definition_discovery();
            assert_eq!(
                discovery.failed_records(),
                fixture.owners.as_slice(),
                "unclassifiable potential owners cannot prove discovery complete"
            );
            assert!(!discovery.is_complete());
        })?;
    }
    Ok(())
}

struct UnclassifiableOwners {
    bytes: Vec<u8>,
    owners: [u32; 2],
    base_definitions: [u32; 2],
    derived_definition: u32,
}

impl UnclassifiableOwners {
    fn new() -> TestResult<Self> {
        let mut bytes = super::derived_database().build();
        let (type_byte, owners, base_definitions, derived_definition) = {
            let forge = DataForge::open(&bytes)?;
            let owner = forge.record_by_guid(guid(OWNER)?).ok_or("first owner")?;
            let derived_owner = forge
                .record_by_guid(guid(DERIVED_OWNER)?)
                .ok_or("owner of the compatible derived reference")?;
            let name = owner.type_name()?;
            assert_eq!(name, ENTITY_TYPE, "the original owner is an Entity");
            assert!(
                std::ptr::eq(derived_owner.type_name()?, name),
                "both original Entity headers share one type name"
            );
            let type_byte = name
                .as_ptr()
                .addr()
                .checked_sub(bytes.as_ptr().addr())
                .ok_or("original type name is in the fixture")?;
            let base_guid = guid(DEFINITION)?;
            let base_definitions: Vec<_> = forge
                .records()
                .filter(|record| record.guid() == base_guid)
                .map(|record| record.index())
                .collect();
            let [first, second] = base_definitions.as_slice() else {
                return Err("two physical base definitions with one GUID".into());
            };
            let derived = forge
                .record_by_guid(guid(DERIVED_TARGET)?)
                .ok_or("compatible derived definition")?;
            (
                type_byte,
                [owner.index(), derived_owner.index()],
                [*first, *second],
                derived.index(),
            )
        };
        let first_byte = bytes.get_mut(type_byte).ok_or("original type-name byte")?;
        assert_eq!(*first_byte, b'E', "the edit addresses the Entity type name");
        *first_byte = 0xff;
        let fixture = Self {
            bytes,
            owners,
            base_definitions,
            derived_definition,
        };
        fixture.verify()?;
        Ok(fixture)
    }

    fn verify(&self) -> TestResult {
        let forge = DataForge::open(&self.bytes)?;
        for index in self.owners {
            let record = forge.record(index).ok_or("original physical owner")?;
            assert!(record.record_path().is_ok(), "owner path remains readable");
            assert!(
                record.type_name().is_err(),
                "actual owner type is unreadable"
            );
        }
        for index in self.base_definitions {
            let record = forge.record(index).ok_or("original base definition")?;
            assert_eq!(
                record.type_name()?,
                MODIFIER,
                "the base definition stays readable"
            );
        }
        let derived = forge
            .record(self.derived_definition)
            .ok_or("original derived definition")?;
        assert_eq!(
            derived.type_name()?,
            DERIVED,
            "the derived definition stays readable"
        );
        Ok(())
    }
}

fn guid(value: u128) -> TestResult<Guid> {
    Ok(common::guid(value).parse()?)
}

pub(super) fn check_derived(fixture: &DataForgeBuilder, index: u32) -> TestResult {
    let (_folder, archive) = common::open(fixture, DcbOrigin::Encrypted)?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_turret_health_modifiers(
        CatalogRecordSelection::RecordIndex(index),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one canonically admitted derived definition".into());
            };
            assert_eq!(
                row.source().source_type().value().copied(),
                Some(DERIVED),
                "the original derived type is retained"
            );
            let physical = row.source().record_index();
            assert_eq!(
                physical.value().copied(),
                Some(index),
                "RecordIndex selects the exact definition"
            );
            assert_eq!(
                physical.origin().contains_encrypted_source(),
                Some(true),
                "the physical header comes from the encrypted Entry"
            );
            assert!(
                snapshot.definition_discovery().is_complete(),
                "the fixture proves the compatible type"
            );
            assert_curve(row)?;
            super::check_definition(&serde_json::to_value(row)?, true)
        },
    )?
}

fn assert_curve(row: &TurretHealthModifier<'_>) -> TestResult {
    let curve = row
        .damage_movement_modifier()
        .value()
        .ok_or("typed movement curve")?;
    assert_eq!(
        curve.use_lut().value().copied(),
        Some(true),
        "the authored LUT flag is retained"
    );
    let points = curve.points().value().ok_or("typed control-point slots")?;
    let [first, second] = points.as_slice() else {
        return Err("two ordered control-point slots".into());
    };
    let first = first.value().ok_or("first typed control point")?;
    let second = second.value().ok_or("second typed control point")?;
    for (y, expected) in [(first.y(), 4.5_f32), (second.y(), -0.0_f32)] {
        assert_eq!(
            y.value().map(ResourceFloat32::raw_bits),
            Some(expected.to_bits()),
            "authored point bits in original order"
        );
        assert_eq!(
            y.origin().contains_encrypted_source(),
            Some(true),
            "the coordinate retains its field evidence"
        );
    }
    Ok(())
}
