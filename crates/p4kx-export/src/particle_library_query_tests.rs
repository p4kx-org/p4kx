// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, CancelToken, InputScope};
use serde_json::Value;

use super::{BROKEN, NEW, OLD, TestResult, assert_rain_slots, synthetic_archive};
use crate::catalog_queries::particle_libraries::{
    ParticleLibrarySelection, read_particle_libraries,
};
use crate::catalog_table::DatasetAvailability;
use crate::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};

#[test]
fn selected_particle_entry_matches_catalog_cells_and_source_roles() -> TestResult {
    let (_folder, archive) = synthetic_archive(true)?;
    let new_index = entry_index(&archive, NEW)?;
    let selected = read_particle_libraries(
        &archive,
        ParticleLibrarySelection::EntryIndex(new_index),
        None,
    )?;
    let [row] = selected.rows.as_slice() else {
        return Err("one selected physical library".into());
    };
    assert_eq!(row.identity().entry_index(), new_index);
    assert!(selected.is_selection_complete());
    assert_eq!(selected.entries.selected, 1, "siblings are not read");
    assert_eq!(selected.entries.projected, 1, "the selected Entry projects");
    assert!(
        selected
            .source_inventory
            .iter_uses()
            .filter(|used| used.scope() == InputScope::Payload)
            .all(|used| used.source() == row.identity().input_ref()),
        "payload uses are bound to the selected physical Entry"
    );
    assert_rain_slots(row, &selected.source_inventory)?;

    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let projected = catalog
        .pointer("/definitions/particleLibraries/rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row["identity"]["entryIndex"] == new_index)
        })
        .ok_or("selected physical row in the canonical Catalog")?;
    let mut expected = projected.clone();
    resolve_uses(&mut expected, &catalog["provenance"]["uses"])?;
    let source_inventory = serde_json::to_value(&selected.source_inventory)?;
    let mut actual = serde_json::to_value(row)?;
    resolve_uses(&mut actual, &source_inventory["uses"])?;
    assert_eq!(
        actual, expected,
        "typed SDK and Catalog cells retain the same values and per-use evidence"
    );
    Ok(())
}

#[test]
fn particle_query_distinguishes_failed_selection_absence_and_cancellation() -> TestResult {
    let (_folder, archive) = synthetic_archive(false)?;
    let old = ParticleLibrarySelection::EntryIndex(entry_index(&archive, OLD)?);
    let all = read_particle_libraries(&archive, ParticleLibrarySelection::All, None)?;
    let selected = read_particle_libraries(&archive, old, None)?;
    assert_eq!(
        all.entries.failed_required_reads, 1,
        "encrypted sibling requires a key"
    );
    assert_eq!(
        all.entries.failed_projections, 1,
        "malformed sibling is diagnosed"
    );
    assert!(!all.is_selection_complete());
    assert!(
        selected.is_selection_complete(),
        "the exact query avoids both siblings"
    );
    assert_eq!(selected.entries.selected, 1);
    let [row] = selected.rows.as_slice() else {
        return Err("selected clear library".into());
    };
    let root = row.nodes().first().ok_or("root element")?;
    assert_eq!(
        root.name().origin().contains_encrypted_source(),
        Some(false)
    );

    let broken = ParticleLibrarySelection::EntryIndex(entry_index(&archive, BROKEN)?);
    let failed = read_particle_libraries(&archive, broken, None)?;
    assert!(matches!(
        failed.availability,
        DatasetAvailability::Unavailable
    ));
    assert_eq!(failed.entries.failed_projections, 1);
    assert!(!failed.is_selection_complete());

    let missing = ParticleLibrarySelection::EntryIndex(u32::MAX);
    let absent = read_particle_libraries(&archive, missing, None)?;
    assert!(matches!(absent.availability, DatasetAvailability::Absent));
    assert_eq!(absent.entries.selected, 0);
    assert!(absent.is_selection_complete());

    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(matches!(
        read_particle_libraries(&archive, old, Some(&cancel)),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}

fn entry_index(archive: &Archive, path: &str) -> TestResult<u32> {
    let index = archive
        .entries()
        .iter()
        .position(|entry| entry.path().eq_ignore_ascii_case(path))
        .ok_or("the fixture contains the selected Entry")?;
    Ok(u32::try_from(index)?)
}

fn resolve_uses(row: &mut Value, uses: &Value) -> TestResult {
    match row {
        Value::Object(fields) => {
            for (key, field) in fields {
                if key == "sourceUsesRefs" {
                    *field = Value::Array(
                        field
                            .as_array()
                            .ok_or("source-use references")?
                            .iter()
                            .map(|id| use_facts(uses, id))
                            .collect::<TestResult<_>>()?,
                    );
                } else {
                    resolve_uses(field, uses)?;
                }
            }
        }
        Value::Array(rows) => {
            for row in rows {
                resolve_uses(row, uses)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn use_facts(uses: &Value, id: &Value) -> TestResult<Value> {
    let mut fields = uses
        .as_array()
        .ok_or("source-use inventory")?
        .iter()
        .find(|source_use| source_use["id"] == *id)
        .and_then(Value::as_object)
        .ok_or("source use in its own inventory")?
        .clone();
    fields.remove("id");
    Ok(Value::Object(fields))
}
