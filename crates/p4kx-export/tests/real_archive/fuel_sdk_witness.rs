// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::fuel::{FuelIntake, FuelTank, ResourceContainer};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
    ProvenanceMode,
};
use p4kx_export::{Filter, Kind};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, StructValue, Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{TestResult, witness};
use crate::authored_component_witness::{
    ComponentCandidate, ComponentEvidence, select_component_owners,
};
use crate::real_archive_item_components as components;
use crate::real_archive_verification::print_line;

#[path = "fuel_sdk_witness_fields.rs"]
mod fields;

const TANK: &str = "SCItemFuelTankParams";
const INTAKE: &str = "SCItemFuelIntakeParams";
const CONTAINER: &str = "ResourceContainer";
const FLOW: &[&str] = &["hydrogenMaxFlowMultiplier", "quantumMaxFlowMultiplier"];
const INTAKE_RATES: &[&str] = &["fuelPushRate", "minimumRate"];
const RESTRICTIONS: &[&str] = &[
    "inclusiveResources",
    "exclusiveResources",
    "inclusiveGroups",
    "exclusiveGroups",
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and run this test with --ignored"]
fn installed_fuel_sdk_preserves_original_values_link_slots_and_origins() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())
        .map_err(|_| "open the installed Archive")?;
    let build = archive
        .build()
        .ok_or("the installed Archive needs an exact Build")?;
    let identity = [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ];
    assert!(
        identity.iter().all(|part| !part.trim().is_empty()),
        "the fuel SDK witness requires all four Build identity fields"
    );
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify Archive before opening DataForge")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "decode the installed DataForge")?;
    let forge = entry.open().map_err(|_| "open the installed DataForge")?;
    let [tank, intake, container] = discover(&forge, &watch)?;
    let evidence = ComponentEvidence::new(&forge, entry.origin(), build)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "prepare a session from the decoded DataForge")?
    .with_provenance_mode(ProvenanceMode::Full);
    let sdk = session
        .parsed(None)
        .map_err(|_| "open the parsed fuel SDK session")?;
    let flow = verify_tanks(&sdk, &evidence, tank)?;
    let rates = verify_intakes(&sdk, &evidence, intake)?;
    let (capacities, slots) = verify_containers(&sdk, &evidence, &forge, container)?;
    watch
        .check()
        .map_err(|_| "verify Archive after the fuel witness")?;
    print_line(&format!(
        "Fuel SDK source witness: {}; {flow} tank multipliers, {rates} intake rates, {capacities} numeric capacities, {slots} original link slots",
        identity.join(" ")
    ));
    Ok(())
}

fn discover<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<[u32; 3]> {
    let mut candidates = [
        ComponentCandidate::new(forge, TANK, |value| has_single(value, FLOW))?,
        ComponentCandidate::new(forge, INTAKE, |value| has_single(value, INTAKE_RATES))?,
        ComponentCandidate::new(forge, CONTAINER, is_linked_numeric_container)?,
    ];
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    select_component_owners(forge, watch, &mut candidates, |record| {
        components::selected_item(record, &rule, &filter)
    })?;
    let [tank, intake, container] = candidates;
    Ok([
        tank.owner("a fuel tank with authored flow multipliers")?,
        intake.owner("a fuel intake with authored rates")?,
        container.owner("a resource container with numeric capacity and an authored link")?,
    ])
}

fn has_single(value: &StructValue<'_>, fields: &[&str]) -> bool {
    fields
        .iter()
        .any(|field| matches!(value.field(field), Some(Original::Single(_))))
}

fn is_linked_numeric_container(value: &StructValue<'_>) -> bool {
    let Some(Original::Struct(capacity)) = value.field("capacity") else {
        return false;
    };
    fields::amount_field(capacity).is_some()
        && (RESTRICTIONS.iter().any(|field| {
            matches!(value.field(field), Some(Original::Array(slots)) if slots.iter().any(is_link))
        }) || matches!(value.field("defaultComposition"), Some(Original::Array(slots)) if slots.iter().any(|slot| {
            matches!(slot, Original::Struct(entry) if entry.field("entry").is_some_and(is_link))
        })))
}

const fn is_link(value: &Original<'_>) -> bool {
    matches!(
        value,
        Original::Reference(_) | Original::Error(ValueError::DanglingReference { .. })
    )
}

fn verify_tanks(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &ComponentEvidence<'_>,
    owner: u32,
) -> TestResult<usize> {
    let mut count = 0;
    sdk.with_fuel_tanks(
        CatalogRecordSelection::RecordIndex(owner),
        None,
        |snapshot| {
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "the original tank scope is readable"
            );
            evidence.verify(
                &snapshot,
                owner,
                &[TANK],
                FuelTank::owner,
                |row, original, trace| {
                    let output = serde_json::to_value(row)?;
                    fields::compare_type(original, &output)?;
                    witness::compare_tank(original, &output)?;
                    count += fields::compare_singles(original, &output, FLOW, trace)?;
                    Ok(())
                },
            )
        },
    )
    .map_err(|_| "read the selected fuel tank SDK rows")??;
    assert!(
        count > 0,
        "the tank sample must prove authored multiplier values"
    );
    Ok(count)
}

fn verify_intakes(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &ComponentEvidence<'_>,
    owner: u32,
) -> TestResult<usize> {
    let mut count = 0;
    sdk.with_fuel_intakes(
        CatalogRecordSelection::RecordIndex(owner),
        None,
        |snapshot| {
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "the original intake scope is readable"
            );
            evidence.verify(
                &snapshot,
                owner,
                &[INTAKE],
                FuelIntake::owner,
                |row, original, trace| {
                    let output = serde_json::to_value(row)?;
                    fields::compare_type(original, &output)?;
                    count += fields::compare_singles(original, &output, INTAKE_RATES, trace)?;
                    Ok(())
                },
            )
        },
    )
    .map_err(|_| "read the selected fuel intake SDK rows")??;
    assert!(count > 0, "the intake sample must prove authored rates");
    Ok(count)
}

fn verify_containers(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &ComponentEvidence<'_>,
    forge: &DataForge<'_>,
    owner: u32,
) -> TestResult<(usize, usize)> {
    let mut capacities = 0;
    let mut slots = 0;
    sdk.with_resource_containers(
        CatalogRecordSelection::RecordIndex(owner),
        None,
        |snapshot| {
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "the original container scope is readable"
            );
            evidence.verify(
                &snapshot,
                owner,
                &[CONTAINER],
                ResourceContainer::owner,
                |row, original, trace| {
                    let output = serde_json::to_value(row)?;
                    fields::compare_type(original, &output)?;
                    witness::compare_policy(forge, original, &output)?;
                    super::compare_container(original, &output)?;
                    capacities += fields::trace_capacity(original, &output, trace)?;
                    slots += fields::trace_links(original, &output, trace)?;
                    Ok(())
                },
            )
        },
    )
    .map_err(|_| "read the selected resource container SDK rows")??;
    assert!(
        capacities > 0 && slots > 0,
        "the container sample must prove a numeric capacity and original link slots"
    );
    Ok((capacities, slots))
}
