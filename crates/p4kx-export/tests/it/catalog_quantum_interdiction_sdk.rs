// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::ship_systems::{
    QuantumInterdictionGeneratorSnapshot, with_quantum_interdiction_generators_from,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceNumber};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::{CatalogError, DcbOrigin, ROWS, TestResult, build_catalog, common, fixture, proof};

#[test]
fn quantum_interdiction_sdk_preserves_original_graph_and_encryption() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture::database(false), origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        let [expected] = proof::rows(&catalog, ROWS)? else {
            return Err("one canonical interdiction generator is expected".into());
        };
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
            .with_quantum_interdiction_generators(
                CatalogRecordSelection::All,
                None,
                |snapshot| verify(&snapshot, &catalog, expected, encrypted),
            )??;
        let entry = DataForgeEntry::read(&archive)?;
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_quantum_interdiction_generators(
                CatalogRecordSelection::RecordIndex(0),
                None,
                |snapshot| verify(&snapshot, &catalog, expected, encrypted),
            )??;
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_quantum_interdiction_generators(
                CatalogRecordSelection::RecordIndex(1),
                None,
                |snapshot| {
                    assert!(
                        snapshot.rows().is_empty(),
                        "a tag Record is not a device owner"
                    );
                },
            )?;
    }
    Ok(())
}

#[test]
fn quantum_interdiction_sdk_rejects_a_foreign_dataforge_origin() -> TestResult {
    let fixture = fixture::database(false);
    let (_local_folder, archive) = common::open(&fixture, DcbOrigin::Plain)?;
    let (_foreign_folder, foreign) = common::open(&fixture, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let called = std::cell::Cell::new(false);
    let result = with_quantum_interdiction_generators_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "identical bytes in another Archive cannot act as the original source"
    );
    assert!(!called.get(), "a rejected source produces no rows");
    Ok(())
}

fn verify(
    snapshot: &QuantumInterdictionGeneratorSnapshot<'_>,
    catalog: &Value,
    expected: &Value,
    encrypted: bool,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("expected one original interdiction generator".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&0),
        "the SDK preserves the physical Item owner"
    );
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "the original authored graph is fully available"
    );
    let jammer = row.jammer_settings().value().ok_or("original jammer")?;
    assert!(
        matches!(jammer.jammer_range().value(), Some(ResourceNumber::Single(value)) if value.raw_bits() == 28.0_f32.to_bits()),
        "the original range is available as a stored Single"
    );
    assert_eq!(
        jammer.jammer_range().origin().contains_encrypted_source(),
        Some(encrypted),
        "the jammer range keeps its own source marker"
    );
    let graph = row.visual_graph().value().ok_or("original visual graph")?;
    let states = graph.states().value().ok_or("ordered visual states")?;
    let state = states
        .first()
        .and_then(|slot| slot.value())
        .ok_or("first authored visual state")?;
    assert_eq!(
        state.transitions().value().map(Vec::len),
        Some(2),
        "the graph exposes each authored transition"
    );
    let sdk = json!({"provenance":snapshot.source_inventory()});
    let row = serde_json::from_slice(&serde_json::to_vec(row)?)?;
    assert_eq!(
        proof::with_origins(row, &sdk)?,
        proof::with_origins(expected.clone(), catalog)?,
        "the SDK and catalog have identical authored values and original receipts"
    );
    Ok(())
}
