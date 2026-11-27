// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use serde_json::{Value, json};

use super::ReducedError;
use super::jump_inventory::{self, JumpInventory};
use crate::StarmapTraversalBudget;
use crate::catalog_queries::starmap::{self, JumpInventorySnapshot};

#[path = "../../tests/catalog_schema/sdk_jump_inventory_fixture.rs"]
mod fixture;

use self::fixture::Fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn jump_inventory_retains_order_markers_and_physical_occurrences() -> TestResult {
    let (scratch, archive) = fixture::open_fixture(Fixture::Complete)?;
    let original = starmap::read_jump_inventory(&archive, StarmapTraversalBudget::new(), None)?;
    let reduced = stage(&original, scratch.path())?;
    let endpoints = serde_json::to_value(reduced.endpoints)?;
    let rows = endpoints
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("reduced endpoint rows")?;
    let [first, repeated, beta] = rows.as_slice() else {
        return Err("two alpha occurrences and one beta occurrence".into());
    };
    assert_eq!(
        first.pointer("/system/value"),
        Some(&json!("alpha")),
        "the first canonical system comes first"
    );
    assert_eq!(
        beta.pointer("/system/value"),
        Some(&json!("beta")),
        "the second system follows repeated alpha placements"
    );
    assert_eq!(
        first.pointer("/starmapRecordGuid/value"),
        repeated.pointer("/starmapRecordGuid/value"),
        "repeated occurrences refer to the same Starmap Record"
    );
    assert_ne!(
        first.pointer("/instanceId/value"),
        repeated.pointer("/instanceId/value"),
        "repeated occurrences retain distinct Child routes"
    );
    for (row, endpoint) in rows.iter().zip(&original.endpoints) {
        assert_matches_endpoint(row, &serde_json::to_value(endpoint)?)?;
    }
    assert_eq!(
        serde_json::to_value(reduced.connections)?,
        json!({"availability": "absent", "rows": []}),
        "a surveyed empty relationship table remains present"
    );
    Ok(())
}

fn assert_matches_endpoint(reduced: &Value, original: &Value) -> TestResult {
    for pointer in [
        "/instanceId",
        "/system",
        "/starmapRecordGuid",
        "/starmapRecordIndex",
        "/starmapType",
        "/worldPosition",
        "/targetPath",
        "/declaredPartner",
    ] {
        let picked = reduced.pointer(pointer).ok_or("selected endpoint cell")?;
        let source = original.pointer(pointer).ok_or("original endpoint cell")?;
        assert_cell(picked, source, pointer);
        assert_eq!(picked.get("value"), source.get("value"), "{pointer} value");
    }
    let reduced_candidates = reduced
        .get("targetCandidates")
        .ok_or("selected candidate cell")?;
    let original_candidates = original
        .get("targetCandidates")
        .ok_or("canonical candidate cell")?;
    assert_cell(reduced_candidates, original_candidates, "targetCandidates");
    assert_candidate_entries(
        reduced_candidates.get("value"),
        original_candidates.get("value"),
    )?;
    for field in ["gate", "container", "localPosition", "worldRotation"] {
        assert!(
            reduced.get(field).is_none(),
            "{field} is outside the selection"
        );
    }
    Ok(())
}

fn assert_candidate_entries(reduced: Option<&Value>, original: Option<&Value>) -> TestResult {
    let expected = original
        .and_then(Value::as_array)
        .ok_or("original ordered candidates")?;
    let actual = reduced
        .and_then(Value::as_array)
        .ok_or("reduced ordered candidates")?;
    assert_eq!(actual.len(), expected.len(), "candidate counts match");
    for (picked, source) in actual.iter().zip(expected) {
        assert_cell(picked, source, "target candidate");
        let exact_index = source
            .pointer("/value/entryIndex")
            .ok_or("physical candidate Entry")?;
        assert_eq!(
            picked.get("value"),
            Some(&json!({"entryIndex": exact_index})),
            "the reduced target identifies the same Entry without a provenance arena"
        );
    }
    Ok(())
}

fn assert_cell(reduced: &Value, original: &Value, label: &str) {
    for field in ["state", "containsEncryptedSource"] {
        assert_eq!(
            reduced.get(field),
            original.get(field),
            "{label} preserves {field}"
        );
    }
    assert_eq!(
        reduced.as_object().map(serde_json::Map::len),
        Some(3),
        "{label} keeps only the three selected cell keys"
    );
}

#[test]
fn jump_inventory_distinguishes_empty_and_incomplete_source_scopes() -> TestResult {
    for (layout, expected) in [
        (Fixture::NoRoots, "absent"),
        (Fixture::NoEndpoints, "available"),
    ] {
        let (scratch, archive) = fixture::open_fixture(layout)?;
        let original = starmap::read_jump_inventory(&archive, StarmapTraversalBudget::new(), None)?;
        let reduced = stage(&original, scratch.path())?;
        assert_eq!(
            serde_json::to_value(reduced.endpoints)?,
            json!({"availability": expected, "rows": []}),
            "the endpoint table preserves source availability at zero rows"
        );
    }

    let (scratch, archive) = fixture::open_fixture(Fixture::LocalEndpoint)?;
    let original = starmap::read_jump_inventory(&archive, StarmapTraversalBudget::new(), None)?;
    let reduced = stage(&original, scratch.path())?;
    let value = serde_json::to_value(reduced.endpoints)?;
    for field in ["targetPath", "targetCandidates", "declaredPartner"] {
        assert_eq!(
            value.pointer(&format!("/rows/2/{field}/state")),
            Some(&json!("absent")),
            "a local endpoint has no authored {field}"
        );
    }

    for layout in [
        Fixture::UnreadableBeta,
        Fixture::DuplicateTarget,
        Fixture::UnreadableRecordType,
        Fixture::ExternalNameMissing,
        Fixture::ExternalNameEmpty,
        Fixture::MissingMetadata,
    ] {
        let (scratch, archive) = fixture::open_fixture(layout)?;
        assert_unavailable(&archive, scratch.path())?;
    }
    let (scratch, archive) = fixture::open_recovered()?;
    assert_unavailable(&archive, scratch.path())
}

fn assert_unavailable(archive: &p4kx_extraction::Archive, scratch: &Path) -> TestResult {
    let original = starmap::read_jump_inventory(archive, StarmapTraversalBudget::new(), None)?;
    assert!(
        matches!(
            stage(&original, scratch),
            Err(ReducedError::IncompleteScope(
                jump_inventory::ENDPOINTS | jump_inventory::CONNECTIONS
            ))
        ),
        "a partial survey cannot certify an empty relationship table"
    );
    Ok(())
}

fn stage(snapshot: &JumpInventorySnapshot, scratch: &Path) -> Result<JumpInventory, ReducedError> {
    jump_inventory::stage(
        snapshot.endpoint_availability,
        &snapshot.endpoints,
        &snapshot.connections,
        scratch,
        None,
    )
}
