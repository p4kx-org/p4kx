// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{Value, json};

use super::{
    ArenaRow, EncounterRow, HEALTH, MissionDisposition, MissionDispositionRows, OrderedRows,
    Projection, TestResult, fingerprint, retain, verify_health,
};

#[test]
fn mission_arena_digest_separates_alias_order_and_wire_changes() -> TestResult {
    use crate::catalog_capture::CatalogCapture;

    let first = sample_arena(11, "p1");
    let second = sample_arena(12, "p2");
    let mut ordered = OrderedRows::with_encounter(EncounterRow::MissionArena);
    for row in [&first, &second] {
        let (key, wire) = EncounterRow::MissionArena.project(row.clone())?;
        ordered.append_encounter_row(key, fingerprint(wire)?)?;
    }
    let mut capture = CatalogCapture::default();
    capture.project("/rows", Projection::StreamRows(ordered.finish()));
    let compare = |rows| -> TestResult {
        let bytes = serde_json::to_vec(&json!({"rows": rows}))?;
        capture.read(bytes.as_slice())?;
        Ok(())
    };
    compare([first.clone(), second.clone()])?;
    let Err(reordered) = compare([second, first.clone()]) else {
        return Err("same-GUID physical aliases must retain their order".into());
    };
    assert!(
        reordered.to_string().contains("encounter order differ"),
        "physical identity and order are diagnosed separately"
    );
    let Err(changed) = compare([first, sample_arena(12, "changed-use")]) else {
        return Err("an altered arena contribution must fail parity".into());
    };
    assert!(
        changed.to_string().contains("wire digests differ"),
        "a source change preserves the owner identity"
    );
    Ok(())
}

#[test]
fn mission_arena_digest_preserves_binary32_wire_projection() -> TestResult {
    use crate::catalog_capture::CatalogCapture;

    #[derive(Serialize)]
    struct Source {
        finite: f32,
    }
    let header = json!({
        "inputRef": "physical-input",
        "recordIndex": {"state": "present", "value": 11},
        "guid": {"state": "present", "value": "fixture-guid"},
    });
    let row = ArenaRow {
        header,
        source: &Source { finite: 0.1 },
    };
    let producer = format!(r#"{{"rows":[{}]}}"#, serde_json::to_string(&row)?);
    let mut ordered = OrderedRows::with_encounter(EncounterRow::MissionArena);
    retain(
        &mut ordered,
        EncounterRow::MissionArena,
        &row,
        &mut HashSet::new(),
    )?;
    let mut capture = CatalogCapture::default();
    capture.project("/rows", Projection::StreamRows(ordered.finish()));
    capture.read(producer.as_bytes())?;
    let widened_row = serde_json::to_value(&row)?;
    let widened = serde_json::to_vec(&json!({"rows": [widened_row]}))?;
    assert!(
        capture.read(widened.as_slice()).is_err(),
        "widening an original Single changes its serialized finite projection"
    );
    Ok(())
}

fn sample_arena(index: u32, use_id: &str) -> Value {
    json!({
        "inputRef": "physical-input",
        "recordIndex": {"state": "present", "value": index},
        "guid": {"state": "present", "value": "shared-guid"},
        "source": {"roots": [{"sourceUsesRefs": [use_id]}], "values": []},
        "unrelatedDefinitionField": true,
    })
}

#[test]
fn mission_dispositions_reject_a_changed_family_skip_count() -> TestResult {
    use crate::catalog_capture::CatalogCapture;

    let mut capture = CatalogCapture::default();
    capture.project(
        HEALTH,
        Projection::MissionDispositions(MissionDispositionRows),
    );
    let bytes = serde_json::to_vec(&json!({
        "exportHealth": {"kinds": [
            sample_health("pools", 2, 9),
            sample_health("contractTemplates", 3, 0),
            sample_health("missionBrokerEntries", 5, 2)
        ]}
    }))?;
    let catalog = capture.read(bytes.as_slice())?;
    let templates = MissionDisposition {
        owners: 3,
        skipped: 0,
    };
    verify_health(
        &catalog,
        [
            templates,
            MissionDisposition {
                owners: 5,
                skipped: 2,
            },
        ],
    )?;
    assert!(
        verify_health(
            &catalog,
            [
                templates,
                MissionDisposition {
                    owners: 5,
                    skipped: 1
                }
            ]
        )
        .is_err(),
        "the correct total requires the correct original Broker skip count"
    );
    Ok(())
}

fn sample_health(kind: &str, owners: usize, skipped: usize) -> Value {
    json!({
        "kind": kind,
        "sourceInventoryComplete": true,
        "writtenRows": owners,
        "intentionallySkippedRows": 0,
        "sourceRecords": {
            "selected": owners + skipped,
            "readSuccessful": owners,
            "projected": owners,
            "intentionallySkipped": skipped,
            "omittedAfterReadFailure": 0,
            "omittedAfterProjectionFailure": 0,
            "failedSelectionCandidates": 0,
        }
    })
}
