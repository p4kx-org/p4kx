// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::equipment::resource_network::{
    ResourceComponent, ResourceConnection, ResourceConnectionFields, ResourceState,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, SourceValueState, SourcedValue, StructValue,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{Coverage, TestResult, compare_component};
use crate::authored_component_witness::ComponentEvidence;
use crate::real_archive_item_components as components;
use crate::real_archive_item_durability::Trace;
use crate::real_archive_verification::print_line;

#[path = "resource_network_sdk_sample.rs"]
mod sample;

const COMPONENT: &str = "ItemResourceComponentParams";
const POWER: &str = "EntityComponentPowerConnection";
const HEAT: &str = "EntityComponentHeatConnection";
const POWER_FIELDS: &[&str] = &[
    "PowerBase",
    "PowerDraw",
    "TimeToReachDrawRequest",
    "PowerToEM",
];
const HEAT_FIELDS: &[&str] = &[
    "ThermalEnergyBase",
    "ThermalEnergyDraw",
    "SpecificHeatCapacity",
    "SurfaceArea",
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and run this test with --ignored"]
fn installed_resource_network_sdk_reconciles_original_components_and_connections() -> TestResult {
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
        "the resource network witness requires all four Build identity fields"
    );
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify Archive before opening DataForge")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the installed DataForge")?;
    let forge = entry.open().map_err(|_| "open the installed DataForge")?;
    let [component, power, heat] = sample::discover(&forge, &watch)?;
    let evidence = ComponentEvidence::new(&forge, entry.origin(), build)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "prepare the resource network SDK session from the decoded DataForge")?;
    let sdk = session
        .parsed(None)
        .map_err(|_| "open the parsed resource network SDK session")?;
    let named_states = verify_components(&sdk, &evidence, component)
        .map_err(|error| format!("resource component sample: {error}"))?;
    let counts = verify_connections(&sdk, &evidence, BTreeSet::from([power, heat]))
        .map_err(|error| format!("resource connection samples: {error}"))?;
    watch
        .check()
        .map_err(|_| "verify Archive after the resource network witness")?;
    print_line(&format!(
        "Resource network SDK source witness: {}; {named_states} ordered named states, {} power values, {} heat values",
        identity.join(" "),
        counts.power,
        counts.heat
    ));
    Ok(())
}

fn verify_components(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &ComponentEvidence<'_>,
    owner: u32,
) -> TestResult<usize> {
    let mut named_states = 0;
    let mut coverage = Coverage::new();
    sdk.with_resource_components(
        CatalogRecordSelection::RecordIndex(owner),
        None,
        |snapshot| {
            evidence.verify(
                &snapshot,
                owner,
                &[COMPONENT],
                ResourceComponent::owner,
                |row, original, trace| {
                    verify_type(&row.source_type, original);
                    compare_component(original, &serde_json::to_value(row)?, &mut coverage)?;
                    named_states += verify_states(row, original, trace)?;
                    Ok(())
                },
            )
        },
    )
    .map_err(|_| "read the selected resource component SDK rows")??;
    assert!(
        named_states > 0,
        "the selected owner has authored named states"
    );
    Ok(named_states)
}

fn verify_states<'source>(
    row: &ResourceComponent<'_>,
    source: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let Some(raw @ Original::Array(originals)) = source.field("states") else {
        return Ok(0);
    };
    let slots = row
        .states
        .value()
        .ok_or("the original ordered state array")?;
    trace.check(&serde_json::to_value(&row.states)?, raw, "selector")?;
    let mut named_states = 0;
    for (position, (raw, slot)) in originals.iter().zip(slots).enumerate() {
        match raw {
            Original::Null => {
                assert!(
                    slot.state() == SourceValueState::Null,
                    "an original null state slot"
                );
                trace.check(&serde_json::to_value(slot)?, raw, "selector")?;
            }
            Original::Struct(state) => {
                named_states += usize::from(verify_state(slot, raw, state, position, trace)?);
            }
            _ => return Err("unreadable original resource state slot".into()),
        }
    }
    Ok(named_states)
}

fn verify_state<'source>(
    slot: &SourcedValue<ResourceState<'_>>,
    raw: &Original<'source>,
    source: &StructValue<'source>,
    position: usize,
    trace: &Trace<'_, 'source>,
) -> TestResult<bool> {
    let state = slot.value().ok_or("an original resource state")?;
    verify_type(&state.source_type, source);
    assert!(
        state.source_position.value() == Some(&u32::try_from(position)?),
        "the state retains its exact original array position"
    );
    trace.part(&serde_json::to_value(slot)?, raw, "$type", "selector")?;
    trace.check(
        &serde_json::to_value(&state.source_position)?,
        raw,
        "conversion",
    )?;
    let Some(name @ Original::String(expected)) = source.field("name") else {
        return Ok(false);
    };
    assert!(
        state.name.value() == Some(expected),
        "the original authored state name matches"
    );
    trace.check(&serde_json::to_value(&state.name)?, name, "payload")?;
    Ok(true)
}

#[derive(Default)]
struct ConnectionCounts {
    power: usize,
    heat: usize,
}

fn verify_connections(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &ComponentEvidence<'_>,
    owners: BTreeSet<u32>,
) -> TestResult<ConnectionCounts> {
    let mut counts = ConnectionCounts::default();
    for owner in owners {
        sdk.with_resource_connections(
            CatalogRecordSelection::RecordIndex(owner),
            None,
            |snapshot| {
                evidence.verify(
                    &snapshot,
                    owner,
                    &[POWER, HEAT],
                    ResourceConnection::owner,
                    |row, original, trace| compare_connection(row, original, trace, &mut counts),
                )
            },
        )
        .map_err(|_| "read the selected power and heat connection SDK rows")??;
    }
    assert!(
        counts.power > 0 && counts.heat > 0,
        "independently authored power and heat fields must both be represented"
    );
    Ok(counts)
}

fn compare_connection<'source>(
    row: &ResourceConnection<'_>,
    source: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
    counts: &mut ConnectionCounts,
) -> TestResult {
    verify_type(&row.source_type, source);
    let (fields, count) = match (source.type_name(), &row.fields) {
        (POWER, ResourceConnectionFields::Power(_)) => (POWER_FIELDS, &mut counts.power),
        (HEAT, ResourceConnectionFields::Heat(_)) => (HEAT_FIELDS, &mut counts.heat),
        _ => return Err("the SDK connection variant differs from the original declaration".into()),
    };
    let output = serde_json::to_value(row)?;
    for field in fields {
        if let Some(raw @ Original::Single(_)) = source.field(field) {
            let cell = components::cell(&output, field)?;
            components::single(source, field, cell)?;
            trace.check(cell, raw, "payload")?;
            *count += 1;
        }
    }
    Ok(())
}

fn verify_type(cell: &SourcedValue<&str>, source: &StructValue<'_>) {
    assert!(
        cell.value() == Some(&source.type_name()),
        "the SDK row retains its concrete DataForge declaration"
    );
}
