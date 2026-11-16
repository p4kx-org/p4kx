// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::projection_schema;
use crate::source_details::SourceCoordinateOmission;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};
use crate::source_use::{
    InputScope, SourceLocation, SourceUse, SourceUseDescription, SourceUseRef, SourceUseRole,
};
use crate::{SourceInput, SourceScope, SourceUses, SourceValue};

use super::StoredXmlSite;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// One source-use inventory row, kept separately from the field's actual contribution set.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "SourceUseDescription", transform = projection_schema::require_properties)]
struct WireUse<'location> {
    id: &'location SourceUseRef,
    source: &'location PhysicalSourceRef,
    scope: InputScope,
    role: SourceUseRole,
    location: &'location SourceLocation,
    effective_encryption: SourceEncryptionState,
}

fn subresource(namespace: &str, indices: &[u64], field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices: indices.to_vec(),
        field: field.to_owned(),
    }
}

#[test]
fn compact_coordinates_keep_the_original_source_use_schema() {
    assert_eq!(
        schemars::schema_for!(SourceUseDescription).to_value(),
        schemars::schema_for!(WireUse<'static>).to_value(),
        "the private coordinate storage preserves the complete original source-use schema"
    );
}

#[test]
fn compact_and_owned_locations_keep_original_bytes_and_roles() -> TestResult {
    let uses = SourceUses::with_compact_coordinates();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for (location, compact) in [
        (subresource("dataforgeValue", &[0, 0], "$value"), true),
        (
            subresource(
                "dataforgeValue",
                &[u64::from(u32::MAX), u64::MAX],
                "$error.guid",
            ),
            true,
        ),
        (
            subresource("dataforgeRecordHeader", &[13], "recordIndex"),
            true,
        ),
        (
            subresource("dataforgeValue", &[2, 3], "$unrecognized_鋼"),
            false,
        ),
        (subresource("dataforgeValue", &[1, 2, 3], "$value"), false),
        (
            subresource("dataforgeRecordHeader", &[u64::MAX], "recordIndex"),
            false,
        ),
        (subresource("xml", &[273_907, 13], "@pos"), true),
    ] {
        verify_roles(&uses, &scope, &location, compact)?;
    }
    let frozen = uses.freeze()?;
    for row in frozen.iter_uses() {
        let _ = serde_json::to_vec(row)?;
    }
    assert_eq!(
        frozen
            .uses()
            .iter()
            .filter(|row| row.location.compact_is_materialized().is_some())
            .count(),
        8,
        "one compact payload and one compact selector exist for each compact location"
    );
    assert!(
        frozen
            .uses()
            .iter()
            .all(|row| row.location.compact_is_materialized() != Some(true)),
        "the compatibility slice clone keeps compact locations unexpanded"
    );
    Ok(())
}

fn verify_roles(
    uses: &SourceUses,
    scope: &SourceScope,
    location: &SourceLocation,
    compact: bool,
) -> TestResult {
    let payload = scope.at(location.clone(), SourceUseRole::Payload)?;
    let selector = scope.at_observed_location(&payload, SourceUseRole::Selector)?;
    let reference = scope.at(location.clone(), SourceUseRole::Selector)?;
    assert!(
        std::ptr::eq(selector.description(), reference.description()),
        "repeat and direct observation retain the same authoritative Arc"
    );
    let cell = uses.value(
        SourceValue::Present(42),
        &[payload.clone(), selector.clone()],
        true,
    );
    assert!(
        cell.origin().source_set_complete(),
        "the actual compact coordinate stays complete"
    );
    assert_eq!(
        cell.origin().source_uses_refs().len(),
        2,
        "both exact role IDs contribute"
    );
    for usage in [&payload, &selector] {
        verify_bytes(usage, location, compact.then_some(false))?;
    }
    let row = payload.description();
    assert_eq!(
        row.location(),
        location,
        "the borrowed accessor reconstructs the original coordinate"
    );
    verify_bytes(&payload, location, compact.then_some(true))?;
    assert_eq!(
        row.clone().location.compact_is_materialized(),
        compact.then_some(false),
        "cloning does not retain or initiate an expanded compact view"
    );
    Ok(())
}

fn verify_bytes(
    usage: &SourceUse,
    location: &SourceLocation,
    materialized: Option<bool>,
) -> TestResult {
    let row = usage.description();
    let expected = WireUse {
        id: row.id(),
        source: row.source(),
        scope: row.scope(),
        role: row.role(),
        location,
        effective_encryption: row.effective_encryption(),
    };
    assert_eq!(
        serde_json::to_vec(row)?,
        serde_json::to_vec(&expected)?,
        "the stored location serializes with the same exact spelling and order as the original"
    );
    assert_eq!(
        row.location.compact_is_materialized(),
        materialized,
        "serialization and field marker calculation do not expand the stored location"
    );
    Ok(())
}

#[test]
fn compact_dataforge_ids_stay_exact_and_authority_cannot_follow_a_matching_id() -> TestResult {
    let uses = SourceUses::with_compact_coordinates();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let location = subresource("dataforgeValue", &[7, u64::MAX], "$value");
    let payload_use = scope.at(location.clone(), SourceUseRole::Payload)?;
    assert_eq!(
        payload_use.reference().as_str(),
        "c2:u0ppsv7.18446744073709551615.:$value",
        "the stored ID stays byte-identical"
    );
    let repeated = scope.at_observed_location(&payload_use, SourceUseRole::Selector)?;
    assert_eq!(
        repeated.reference().as_str(),
        "c2:u0pssv7.18446744073709551615.:$value",
        "changing role leaves every coordinate byte intact"
    );

    let foreign = SourceUses::with_compact_coordinates();
    let other_scope = foreign.observe(SourceInput::Unknown, InputScope::Payload)?;
    let foreign_use = other_scope.at(location, SourceUseRole::Payload)?;
    assert_eq!(
        foreign_use.reference(),
        payload_use.reference(),
        "the fixture has the same opaque ID in two inventories"
    );
    assert!(
        matches!(
            scope.at_observed_location(&foreign_use, SourceUseRole::Selector),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "coincident IDs cannot import a foreign role observation"
    );
    let metadata = scope.for_scope(InputScope::ArchiveDirectoryMetadata);
    assert!(
        matches!(
            metadata.at_observed_location(&payload_use, SourceUseRole::Selector),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "one physical source cannot lend authority to a different byte scope"
    );
    Ok(())
}

#[test]
fn opaque_observations_stay_incomplete_inside_a_compact_inventory() -> TestResult {
    let uses = SourceUses::with_compact_coordinates();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let location = subresource("dataforgeRecordHeader", &[4], "fileName");
    let field = scope.at(location, SourceUseRole::Payload)?;
    let opaque = uses.opaque_use(&field, SourceCoordinateOmission::CaptureLimit)?;
    let cell = uses.value(SourceValue::Present(true), &[field.clone(), opaque], true);
    assert!(
        !cell.origin().source_set_complete(),
        "omitted coordinates still weaken completeness"
    );
    assert_eq!(
        field.description().location.compact_is_materialized(),
        Some(false),
        "measuring completeness does not expand adjacent known coordinates"
    );
    Ok(())
}

#[test]
fn compact_xml_locations_keep_the_original_keys_and_shapes() -> TestResult {
    let uses = SourceUses::with_compact_coordinates();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for (location, expected_payload) in [
        (
            subresource("xml", &[273_907, 13], "@pos"),
            "c2:u0ppxpBC3z.N",
        ),
        (
            subresource("xml", &[273_907, 13], "@Pos"),
            "c2:u0ppsx273907.13.:@Pos",
        ),
        (
            subresource("xmlSource.text", &[5, u64::MAX], "@n\u{e4}m\u{e9}"),
            "c2:u0pps?14:xmlSource.text5.18446744073709551615.:@n\u{e4}m\u{e9}",
        ),
        (
            subresource("xmlSource.cryXml", &[7, 2], "@Pos"),
            "c2:u0pps?16:xmlSource.cryXml7.2.:@Pos",
        ),
        (
            subresource("xmlSource.cryXml", &[7, 3], "@Pos"),
            "c2:u0pps?16:xmlSource.cryXml7.3.:@Pos",
        ),
        (
            subresource("xmlSource.cryXml", &[7], "$node"),
            "c2:u0pps?16:xmlSource.cryXml7.:$node",
        ),
    ] {
        let payload = scope.at(location.clone(), SourceUseRole::Payload)?;
        assert_eq!(
            payload.reference().as_str(),
            expected_payload,
            "the original ID includes exact spelling and occurrence",
        );
        verify_roles(&uses, &scope, &location, true)?;
    }
    Ok(())
}

#[test]
fn compact_xml_falls_back_for_other_shapes_and_preserves_full_ids() -> TestResult {
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for (location, compact) in [
        (subresource("xml", &[64], "$absent:@pos"), true),
        (subresource("xml", &[0, u64::MAX], "$text"), true),
        (subresource("xml", &[u64::MAX], "$name"), false),
        (subresource("xml", &[2, 3, 4], "$name"), false),
        (subresource("xmlSource.text", &[12, 11], "@a"), false),
        (subresource("xmlSource.text", &[12, 13], "$unknown"), false),
        (subresource("xmlSource.cryXml", &[7, u64::MAX], "@a"), false),
        (subresource("xmlSource.cryxml", &[7, 2], "@a"), false),
    ] {
        verify_roles(&uses, &scope, &location, compact)?;
    }
    Ok(())
}

#[test]
fn compact_xml_site_fits_in_the_existing_owned_location_space() {
    assert!(
        size_of::<StoredXmlSite>() <= size_of::<SourceLocation>(),
        "compact XML does not make unrelated retained variants larger",
    );
}
