// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
    MissionLocationTemplateSnapshot, with_mission_location_templates,
};
use p4kx_extraction::SourceValueState;
use p4kx_testkit::DataForgeBuilder;

use crate::catalog_contracts::location_vocabulary::fixture as classification_fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[path = "mission_location_sdk_arena.rs"]
mod arena;
#[path = "mission_location_sdk_configuration.rs"]
mod configuration;
#[path = "mission_location_sdk_examples.rs"]
mod examples;
#[path = "mission_location_sdk_fixture.rs"]
mod fixture;
#[path = "mission_location_sdk_input.rs"]
mod input;
#[path = "mission_location_sdk_proof.rs"]
mod proof;
#[path = "mission_location_sdk_rows.rs"]
mod rows;
#[path = "mission_location_sdk_spawn.rs"]
mod spawn;

#[test]
fn configuration_limits_and_security_reference_have_complete_typed_access() -> TestResult {
    verify_fixture(&super::database(), examples::limits)
}

#[test]
fn spawn_limits_and_tag_slots_have_complete_typed_access() -> TestResult {
    verify_fixture(&super::auto_spawn::database(), examples::spawn)
}

#[test]
fn repeated_inventory_has_typed_values_and_separate_original_occurrences() -> TestResult {
    verify_fixture(
        &super::auto_spawn::inventory_database(),
        examples::inventory,
    )
}

#[test]
fn module_inputs_properties_and_authored_payloads_have_complete_typed_access() -> TestResult {
    verify_fixture(&super::mission_module::database(), examples::module)
}

#[test]
fn classification_reference_headers_keep_resolved_null_and_dangling_states() -> TestResult {
    verify_fixture(
        &classification_fixture::template_database("MissionLocationData"),
        examples::classification,
    )
}

#[test]
fn tag_names_and_dangling_guid_cells_have_complete_typed_access() -> TestResult {
    verify_fixture(&crate::spawns_fixture::database(), examples::tags)
}

#[test]
fn entity_cluster_member_guid_preserves_an_original_nonnull_reference() -> TestResult {
    verify_fixture(&fixture::entity_cluster(), examples::entity_cluster)
}

fn verify_fixture(
    database: &DataForgeBuilder,
    verify: impl Fn(&MissionLocationTemplateSnapshot<'_>) -> TestResult,
) -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_directory, archive) = common::open(database, origin)?;
        with_mission_location_templates(
            &archive,
            CatalogRecordSelection::All,
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| {
                verify_snapshot(&snapshot, marker)?;
                verify(&snapshot)
            },
        )??;
    }
    Ok(())
}

fn verify_snapshot(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    marker: Option<bool>,
) -> TestResult {
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "declared family"
    );
    assert!(
        snapshot.build.is_none(),
        "the fixture has no installed Build"
    );
    assert!(
        snapshot.requested_languages.is_empty(),
        "explicit empty locale selection"
    );
    assert!(
        snapshot.available_languages.is_empty(),
        "no locale payloads needed"
    );
    assert!(!snapshot.rows.is_empty(), "at least one original template");
    assert!(
        snapshot.coverage.unreadable_headers.is_empty(),
        "synthetic headers remain readable"
    );
    assert!(
        snapshot.coverage.unreadable_bodies.is_empty(),
        "synthetic bodies remain readable"
    );
    assert!(
        snapshot.localization_notes.is_empty(),
        "no localization decoding was requested"
    );
    let mut view = proof::View::new(&snapshot.source_inventory);
    for row in &snapshot.rows {
        assert_eq!(
            view.row(row)?,
            serde_json::to_value(row)?,
            "every serialized nested field is reachable through the typed API"
        );
        if row.configuration().state() == SourceValueState::Present {
            assert_eq!(
                row.configuration().origin().contains_encrypted_source(),
                marker,
                "the caller sees the actual original DCB origin"
            );
        }
        let _position = row.record_index();
    }
    assert!(
        view.cells > 0,
        "typed sourced values were checked against this inventory"
    );
    Ok(())
}
