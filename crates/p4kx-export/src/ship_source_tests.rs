// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceValueState, Value};

use super::ShipSourceRead;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::digest::Digest;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::variants::ShipKey;

#[path = "ship_source_fixture.rs"]
mod fixture;
#[path = "ship_source_states_tests.rs"]
mod states;
#[path = "ship_variant_cohort_input_tests.rs"]
mod variant_input_tests;

use fixture::{Fault, Origin, TestResult};

#[test]
fn ship_original_reader_keeps_physical_aliases_raw_selectors_and_exact_numbers() -> TestResult {
    fixture::with_projection(
        &fixture::database(Fault::None),
        Origin::Plain,
        |forge, projection| {
            let primary = forge.record(0).ok_or("original primary ship")?;
            let alias = forge.record(1).ok_or("original alias ship")?;
            assert_eq!(
                primary.guid(),
                alias.guid(),
                "the control really shares a nominal GUID"
            );
            assert_eq!(
                primary.record_path()?,
                alias.record_path()?,
                "the control also shares the original path"
            );
            for (record, crew, raw) in [
                (primary, 4, "  Data\\Vehicles\\Original.xml  "),
                (alias, 9, ""),
            ] {
                let tree = record.tree()?;
                let admission = RecordObservation::capture(projection.original(), &record)?;
                let read = super::read(&tree, projection, &admission, None)?;
                assert_eq!(
                    read.header.record_index.value(),
                    Some(&record.index()),
                    "physical identity stays original"
                );
                assert_eq!(
                    read.vehicle_definition.value(),
                    Some(&raw),
                    "authored text is neither trimmed nor path-normalized"
                );
                let Some(ResourceNumber::UInt64 { value }) = read.crew_size.value() else {
                    return Err("original crew UInt64".into());
                };
                assert_eq!(value.get(), crew, "the two original bodies stay distinct");
                let Some(ResourceNumber::UInt64 { value }) = read.presentation.size.value() else {
                    return Err("original Size UInt64".into());
                };
                assert_eq!(
                    value.get(),
                    u64::MAX,
                    "the original size is not narrowed to i64"
                );
                check_insurance(&read)?;
                check_crew_manifest(&read)?;
            }
            assert!(
                forge
                    .record(2)
                    .ok_or("original crew target")?
                    .tree()
                    .is_err(),
                "target body is genuinely unreadable without preventing an authored link"
            );
            Ok(())
        },
    )
}

#[test]
fn variant_keys_use_the_original_definition_with_its_prefix_evidence() -> TestResult {
    fixture::with_projection(
        &fixture::database(Fault::EarlierComponent),
        Origin::Encrypted,
        |forge, projection| {
            let implementation = Digest::from_bytes([1; 16]);
            let expected = ShipKey::new("  data/vehicles/original.xml  ", implementation);
            let primary = forge.record(0).ok_or("original primary ship")?;
            let alias = forge.record(1).ok_or("original alias ship")?;
            assert_eq!(
                primary.guid(),
                alias.guid(),
                "the Records are physical aliases"
            );
            for (record, wanted, complete) in
                [(primary, Some(&expected), false), (alias, None, true)]
            {
                let tree = record.tree()?;
                let admission = RecordObservation::capture(projection.original(), &record)?;
                let row = super::read(&tree, projection, &admission, None)?;
                let definition = &row.vehicle_definition;
                let key = ShipKey::from_read(definition.value().copied(), Some(implementation));
                assert_eq!(
                    key.as_ref(),
                    wanted,
                    "normalize the original nonempty text without trimming it",
                );
                assert_eq!(
                    definition.origin().source_set_complete(),
                    complete,
                    "an earlier failed component remains distinct from a known empty definition",
                );
                assert_eq!(
                    definition.origin().contains_encrypted_source(),
                    Some(true),
                    "prefix uncertainty does not erase encrypted inputs",
                );
            }
            Ok(())
        },
    )
}

fn check_insurance(read: &ShipSourceRead<'_, '_>) -> TestResult {
    let insurance = read
        .insurance
        .value()
        .ok_or("first original insurance declaration")?;
    assert_eq!(
        insurance.static_element_ordinal.value(),
        Some(&1),
        "actual static occurrence ordinal"
    );
    assert_eq!(
        insurance.restore_on_claim.value(),
        Some(&false),
        "the first declaration wins over a later true value"
    );
    let params = insurance.params.value().ok_or("original params")?;
    assert_eq!(
        params.ship_entity_class_name.value(),
        Some(&""),
        "a present empty original string is preserved"
    );
    let Some(ResourceNumber::UInt64 { value }) = params.expedite_fee.value() else {
        return Err("original fee UInt64".into());
    };
    assert_eq!(value.get(), u64::MAX, "fee width and value survive");
    assert!(
        read.numbers.iter().any(|number| number.token
            == crate::catalog_number_capture::FloatToken::Double(fixture::NAN_BITS)),
        "unknown insurance parameter bits retain a real numeric receipt"
    );
    Ok(())
}

fn check_crew_manifest(read: &ShipSourceRead<'_, '_>) -> TestResult {
    let manifest = read
        .playability_inputs
        .crew_manifest
        .value()
        .ok_or("authored crew link")?;
    assert_eq!(
        manifest.target_record_index().value(),
        Some(&2),
        "the actual header target stays physical"
    );
    assert_eq!(
        read.playability_inputs
            .crew_manifest_component_ordinal
            .value(),
        Some(&3),
        "the original component slot is retained"
    );
    Ok(())
}

#[test]
fn ship_original_reader_rejects_same_guid_foreign_frames_and_keeps_cancellation_fatal() -> TestResult
{
    fixture::with_projection(
        &fixture::database(Fault::None),
        Origin::Plain,
        |forge, projection| {
            let primary = forge.record(0).ok_or("original primary")?;
            let alias = forge.record(1).ok_or("original alias")?;
            let tree = primary.tree()?;
            let alias_admission = RecordObservation::capture(projection.original(), &alias)?;
            assert!(
                matches!(
                    super::read(&tree, projection, &alias_admission, None),
                    Err(crate::CatalogError::SourceEvidence(
                        p4kx_extraction::SourceEvidenceError::ForeignEvidence
                    ))
                ),
                "equal nominal identity cannot admit another physical tree"
            );
            let admission = RecordObservation::capture(projection.original(), &primary)?;
            let cancel = CancelToken::new();
            cancel.cancel();
            assert!(
                matches!(
                    super::read(&tree, projection, &admission, Some(&cancel)),
                    Err(crate::CatalogError::Cancelled)
                ),
                "cancelled source capture returns an outer error"
            );
            Ok(())
        },
    )
}

#[test]
fn ship_original_cells_keep_actual_encryption_and_named_selector_closure_without_locales()
-> TestResult {
    for (origin, marker) in [
        (Origin::Plain, Some(false)),
        (Origin::Encrypted, Some(true)),
        (Origin::Unknown, None),
    ] {
        fixture::with_projection(
            &fixture::database(Fault::None),
            origin,
            |forge, projection| {
                let record = forge.record(0).ok_or("original ship")?;
                let tree = record.tree()?;
                let admission = RecordObservation::capture(projection.original(), &record)?;
                let read = super::read(&tree, projection, &admission, None)?;
                check_encryption_origins(&read, marker);
                let scope = projection.original().record_scope(&record, &tree)?;
                let originals = OriginalFlowFields::read(&scope, None)?;
                let source = read.vehicle.ok_or("original selected vehicle")?;
                let owner = originals.owner_of(source)?;
                let name = originals
                    .field(owner, "vehicleDefinition")?
                    .ok_or("actual original selector")?
                    .1;
                let selector = name.observe(
                    OriginalValuePart::FieldName,
                    p4kx_extraction::SourceUseRole::Selector,
                )?;
                assert!(
                    read.vehicle_definition
                        .origin()
                        .source_uses_refs()
                        .contains(selector.reference()),
                    "the immediate cell retains its actual named field selection"
                );
                assert!(
                    matches!(
                        name.value(),
                        Value::String("  Data\\Vehicles\\Original.xml  ")
                    ),
                    "the control retains the exact authored field"
                );
                assert_eq!(
                    read.vehicle_definition.state(),
                    SourceValueState::Present,
                    "source state stays independent of encryption knowledge"
                );
                assert!(
                    read.notes.is_empty(),
                    "the unreadable unrelated locale is not a source-field failure"
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}

fn check_encryption_origins(read: &ShipSourceRead<'_, '_>, marker: Option<bool>) {
    for origin in [
        read.header.guid.origin(),
        read.crew_size.origin(),
        read.vehicle_definition.origin(),
        read.presentation.size.origin(),
        read.insurance.origin(),
        read.playability_inputs.crew_manifest.origin(),
    ] {
        assert_eq!(
            origin.contains_encrypted_source(),
            marker,
            "each actual original datum owns its marker"
        );
        assert!(
            origin.source_set_complete(),
            "the marker does not substitute for source completeness"
        );
    }
}
