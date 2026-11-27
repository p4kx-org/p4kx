// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::FacilityInteriorPlacementSnapshot;
use p4kx_export::catalog_queries::{CatalogQuerySession, DatasetAvailability, ProvenanceMode};
use p4kx_extraction::SourceValueState;
use serde_json::{Value, json};

use super::{TestResult, build, fixture, interior_rows, proof};
use crate::commodity_v3_fixture as common;
use crate::sdk_full_proof as full_proof;

#[test]
fn sdk_interior_placements_match_the_canonical_ship_rows_and_sources() -> TestResult {
    for (case, count, attached) in [
        (fixture::Case::Direct, 2, false),
        (fixture::Case::ItemRooms, 4, true),
        (fixture::Case::HelperFrame, 2, false),
        (fixture::Case::AmbiguousClass, 0, false),
    ] {
        let (_folder, archive) = fixture::archive(case)?;
        let canonical = build(&archive)?;
        let session = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?;
        let parsed = session.parsed(None)?;
        let sdk = parsed.read_facility_interior_placements(0, None)?;
        assert!(
            matches!(sdk.availability, DatasetAvailability::Available),
            "the fixture declares original facility sources"
        );
        assert_eq!(
            sdk.coverage.root_state,
            SourceValueState::Present,
            "the selected physical Ship owns these rooms"
        );
        assert_eq!(
            sdk.rows.len(),
            count,
            "the direct, attached and conflicting fixtures preserve their distinct occurrences"
        );
        for row in &sdk.rows {
            let placement = row.interior_placement().ok_or("typed Interior placement")?;
            assert!(
                placement.position().value().is_some(),
                "each placement has a physical SOC Entity position"
            );
            assert_eq!(
                placement.loadout_position().is_some(),
                attached,
                "only the attached rooms carry their exact Loadout path"
            );
        }
        verify_wire_and_evidence(&sdk, &canonical, case)?;
        let foreign_root = parsed.read_facility_interior_placements(u32::MAX, None)?;
        assert!(
            foreign_root.coverage.root_state != SourceValueState::Present
                && foreign_root.rows.is_empty()
                && foreign_root.coverage.rows.is_empty(),
            "an invalid physical selector cannot widen to another Ship"
        );
    }
    Ok(())
}

fn verify_wire_and_evidence(
    sdk: &FacilityInteriorPlacementSnapshot,
    canonical: &Value,
    case: fixture::Case,
) -> TestResult {
    let selected = json!({
        "data": {
            "placements": sdk.rows,
            "coverage": sdk.coverage.rows,
        },
        "provenance": sdk.coverage.source_inventory,
    });
    let expected = json!({
        "placements": interior_rows(canonical)?,
        "coverage": proof::at(canonical, "/placements/facilityInteriorCoverage")?,
    });
    let rows = proof::at(&selected, "/data")?;
    assert_eq!(
        rows, &expected,
        "the selected row order, source values, transforms, coverage and configuration joins match"
    );
    assert_eq!(
        proof::with_origins(rows.clone(), &selected)?,
        proof::with_origins(expected, canonical)?,
        "each reference resolves to the same original coordinates and encryption evidence"
    );
    if matches!(case, fixture::Case::HelperFrame) {
        let inventory = serde_json::to_string(&sdk.coverage.source_inventory)?;
        assert!(
            inventory.to_ascii_lowercase().contains("room_hull.cg"),
            "selected placements retain the original model helper evidence"
        );
    }
    if matches!(case, fixture::Case::AmbiguousClass) {
        assert!(
            !sdk.coverage.complete,
            "the ambiguous physical class remains visible as incomplete coverage"
        );
    }
    proof::closure(&selected)
}

#[test]
fn default_interior_placement_query_preserves_the_canonical_markers() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::Direct)?;
    let built = build_catalog(&archive, &common::options())?;
    let envelope: Value = serde_json::from_slice(built.json())?;
    let catalog = proof::at(&envelope, "/catalog")?;
    let session = CatalogQuerySession::open(&archive, full_proof::NO_LANGUAGES, None)?;
    let sdk = session.read_facility_interior_placements(0, None)?;
    assert_eq!(
        serde_json::to_value(&sdk.rows)?,
        serde_json::to_value(interior_rows(catalog)?)?,
        "the default placement rows preserve the catalog values, states and encryption markers"
    );
    assert_eq!(
        serde_json::to_value(&sdk.coverage.rows)?,
        *proof::at(catalog, "/placements/facilityInteriorCoverage")?,
        "the default placement query preserves the coverage markers"
    );
    let inventory = &sdk.coverage.source_inventory;
    assert_eq!(inventory.provenance_mode(), ProvenanceMode::MarkersOnly);
    assert_eq!(inventory.iter_sources().len(), 0);
    assert_eq!(inventory.iter_uses().len(), 0);
    Ok(())
}
