// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::SourceValueState;
use serde_json::Value;

use super::{TestResult, build, fixture, proof};
use crate::sdk_full_proof as full_proof;

#[test]
fn sdk_room_coverage_matches_one_physical_ship_and_avoids_models() -> TestResult {
    for case in [
        fixture::Case::Direct,
        fixture::Case::ItemRooms,
        fixture::Case::HelperFrame,
        fixture::Case::AmbiguousClass,
    ] {
        let (_folder, archive) = fixture::archive(case)?;
        let canonical = build(&archive)?;
        let sdk = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
            .read_facility_interior_coverage(0, None)?;
        assert_eq!(
            sdk.root_state,
            SourceValueState::Present,
            "the selected Ship owns this scope"
        );
        assert_eq!(
            serde_json::to_value(&sdk.rows)?,
            *proof::at(&canonical, "/placements/facilityInteriorCoverage")?,
            "the coverage-only route matches canonical Normal positions and findings"
        );
        let known: BTreeSet<_> = sdk
            .source_inventory
            .iter_uses()
            .map(|used| used.id().as_str())
            .collect();
        let rows = serde_json::to_value(&sdk.rows)?;
        verify_refs(&rows, &known)?;
        let inventory = serde_json::to_string(&sdk.source_inventory)?;
        assert!(
            !inventory.to_ascii_lowercase().contains("room_hull.cg"),
            "the interior coverage result never opens model helpers"
        );
        if matches!(case, fixture::Case::AmbiguousClass) {
            assert!(
                !sdk.complete,
                "the physical class conflict is visible in the selected coverage"
            );
        }
    }
    Ok(())
}

fn verify_refs(value: &Value, known: &BTreeSet<&str>) -> TestResult {
    if let Some(object) = value.as_object() {
        if let Some(list) = object.get("sourceUsesRefs").and_then(Value::as_array) {
            for entry in list {
                let id = entry.as_str().ok_or("original source-use reference")?;
                assert!(
                    known.contains(id),
                    "the scoped source inventory retains every coverage reference"
                );
            }
        }
        for child in object.values() {
            verify_refs(child, known)?;
        }
    } else if let Some(values) = value.as_array() {
        for child in values {
            verify_refs(child, known)?;
        }
    }
    Ok(())
}
