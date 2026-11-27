// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_export::catalog_queries::DatasetAvailability;
use serde::de::DeserializeSeed;
use serde_json::{Value, json};

use super::{
    DigestedOrigins, DigestedRows, DigestedTable, PhysicalSlot, TestResult, fingerprint,
    require_digest_scope,
};

#[test]
fn digested_comparison_rejects_missing_and_conflicting_rows() -> TestResult {
    let mut table = DigestedTable::default();
    let mut references = HashSet::new();
    let row = json!({"number": 7, "origin": {"sourceUsesRefs": ["a"]}});
    let linked = json!({"number": 2, "origin": {"sourceUsesRefs": ["b"]}});
    let direct_slot = PhysicalSlot {
        record_index: 5,
        component_ordinal: 1,
    };
    let linked_slot = PhysicalSlot {
        record_index: 2,
        component_ordinal: 1,
    };
    table.retain_direct(direct_slot, &row, &mut references)?;
    table.retain_dependency(linked_slot, &linked, &mut references)?;
    table.retain_dependency(linked_slot, &linked, &mut references)?;
    assert!(
        table
            .retain_dependency(direct_slot, &row, &mut references)
            .is_err(),
        "a digest slot cannot be both a direct row and a dependency",
    );
    assert!(
        table
            .retain_dependency(linked_slot, &row, &mut references)
            .is_err(),
        "two different dependency rows cannot occupy the same physical slot",
    );
    let seed = DigestedRows("/table", &table);
    let reordered_keys = json!({"origin": {"sourceUsesRefs": ["a"]}, "number": 7});
    for (rows, accepted) in [
        (json!([linked, reordered_keys]), true),
        (json!([row]), false),
        (json!([row, linked]), false),
        (json!([linked, row, linked]), false),
    ] {
        assert_eq!(
            seed.deserialize(rows).is_ok(),
            accepted,
            "full ordered occurrence fingerprints"
        );
    }

    let origin = json!({"id":"a", "role":"payload"});
    let origins = HashMap::from([("a".to_owned(), fingerprint(origin.clone())?)]);
    let seed = DigestedOrigins(&origins);
    for (rows, accepted) in [
        (json!([{"id":"unused"}, origin]), true),
        (json!([origin, origin]), false),
        (json!([{"id":"a", "role":"selector"}]), false),
        (json!([]), false),
    ] {
        assert_eq!(
            seed.deserialize(rows).is_ok(),
            accepted,
            "exact contributing provenance"
        );
    }
    Ok(())
}

#[test]
fn digested_scope_accepts_only_complete_dispositions() -> TestResult {
    let mut table = DigestedTable::default();
    let seed = DigestedRows("/table", &table);
    assert_eq!(
        seed.deserialize(json!([]))?,
        Value::from(0),
        "an exactly empty Catalog array matches a zero-row digest",
    );
    assert!(
        seed.deserialize(json!([{"different": true}])).is_err(),
        "an unexpected Catalog row cannot match an empty digest",
    );
    for (availability, complete) in [
        (DatasetAvailability::Available, true),
        (DatasetAvailability::Absent, true),
        (DatasetAvailability::Unavailable, false),
    ] {
        assert_eq!(
            require_digest_scope("/table", &table, availability).is_ok(),
            complete,
            "empty digest for {availability:?}",
        );
    }
    let slot = PhysicalSlot {
        record_index: 2,
        component_ordinal: 9,
    };
    let row = json!({"origin": {"sourceUsesRefs": ["a"]}});
    table.retain_direct(slot, &row, &mut HashSet::new())?;
    assert!(
        require_digest_scope("/table", &table, DatasetAvailability::Available).is_ok(),
        "an Available table can contain physical rows",
    );
    assert!(
        require_digest_scope("/table", &table, DatasetAvailability::Absent).is_err(),
        "an absence cannot carry physical rows",
    );
    Ok(())
}
