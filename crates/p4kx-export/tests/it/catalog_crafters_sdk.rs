// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::crafting::CrafterComponent;
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceNumber};
use p4kx_extraction::{Archive, SourceInventory};
use serde_json::Value;

use crate::commodity_v3_fixture::guid;
use crate::crafters_fixture::{BENCH, write_archive};

use super::{TestResult, crafter, options};

#[test]
fn crafting_station_source_fields_and_sdk_rows_match() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive(directory.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(&archive, options(None).with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let bench = crafter(&catalog, BENCH)?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_crafters(
        CatalogRecordSelection::Guid(guid(BENCH).parse()?),
        None,
        |snapshot| {
            let [station] = snapshot.rows.as_slice() else {
                return Err("one selected original crafter".into());
            };
            assert_eq!(
                &serde_json::to_value(station)?,
                bench,
                "the SDK and Catalog emit the same sourced row"
            );
            let component = station.component().value().ok_or("original component")?;
            verify_component(component, &snapshot.source_inventory)
        },
    )?
}

fn verify_component(component: &CrafterComponent<'_>, inventory: &SourceInventory) -> TestResult {
    assert_eq!(
        component.component_ordinal().value(),
        Some(&1),
        "the original slot follows the unrelated component"
    );
    let limit = component.max_jobs_in_history();
    assert!(
        matches!(limit.value(), Some(ResourceNumber::Int32 { value: 10 })),
        "the original history limit retains its storage"
    );
    assert_eq!(
        limit.origin().contains_encrypted_source(),
        Some(true),
        "the field keeps its encrypted DCB origin"
    );
    let queues = component.queues().value().ok_or("original queue array")?;
    let [first, second] = queues.as_slice() else {
        return Err("two original ordered queues".into());
    };
    assert_eq!(
        (
            first.value().and_then(|queue| queue.name().value()),
            second.value().and_then(|queue| queue.name().value())
        ),
        (Some(&"Fabricator"), Some(&"Workbench")),
        "original queue positions"
    );
    assert!(
        limit
            .origin()
            .source_uses_refs()
            .iter()
            .all(|id| { inventory.iter_uses().any(|source| source.id() == id) }),
        "all source uses resolve in the SDK inventory"
    );
    Ok(())
}
