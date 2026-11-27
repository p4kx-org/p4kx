// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::InstalledConfigurationSnapshot;
use p4kx_extraction::SourceValueState;
use serde_json::Value;

use super::super::compare_rows::{Projection, ship_row};
use super::super::digest_parity::{capture_inventory, fingerprint};
use super::super::{BoundMeasurement, Counts, TestResult, collect_references, wire_row};
use super::digest_runs::Fingerprint;
use super::interior_loadouts::{LOADOUTS, ShipLoadoutDigest};
use super::wire::{EncounterRow, OrderedRows};
use super::{StreamedComponentDigests, spill};

pub(crate) struct StreamedFlightDigests<'directory> {
    comparison: StreamedComponentDigests<'directory>,
    roots: ShipLoadoutDigest,
}

impl<'directory> StreamedFlightDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        let mut comparison = StreamedComponentDigests::new(directory);
        comparison.rows[0] = OrderedRows::with_encounter(EncounterRow::InstalledFlight);
        Self {
            comparison,
            roots: ShipLoadoutDigest::default(),
        }
    }

    pub(crate) fn retain(&mut self, snapshot: &InstalledConfigurationSnapshot) -> TestResult {
        if snapshot.loadout_state == SourceValueState::Present {
            self.roots.append(snapshot.ship_record_index)?;
        }
        if snapshot.rows.is_empty() {
            return Ok(());
        }
        let mut uses = HashSet::new();
        for row in &snapshot.rows {
            let wire = wire_row(row)?;
            let mut row_uses = HashSet::new();
            collect_references(&wire, &mut row_uses);
            if row_uses.is_empty() {
                return Err(
                    "an installed Flight occurrence lacks original contributing uses".into(),
                );
            }
            uses.extend(row_uses.into_iter().map(str::to_owned));
            let key = occurrence_key(&wire)?;
            self.comparison.rows[0].append_encounter_row(key, fingerprint(wire)?)?;
        }
        let inventory = &snapshot.source_inventory;
        capture_inventory(
            inventory,
            uses,
            |id, row| spill(&mut self.comparison.uses, id, row),
            |id, row| spill(&mut self.comparison.sources, id, row),
        )
    }

    pub(crate) fn compare(
        self,
        measurement: &BoundMeasurement,
        path: &str,
        availability: DatasetAvailability,
    ) -> TestResult<Counts<1>> {
        let roots = self.roots.finish();
        measurement.verify_streamed_tables(
            [path],
            [availability],
            self.comparison,
            |capture| capture.project(LOADOUTS, Projection::ShipLoadoutRows(roots)),
            |catalog| {
                if catalog.pointer(LOADOUTS) != Some(&Value::from(roots.count())) {
                    return Err(
                        "the retained catalog omits the physical Ship Loadout census".into(),
                    );
                }
                Ok(())
            },
        )
    }
}

pub(super) fn occurrence_key(row: &Value) -> TestResult<Fingerprint> {
    let physical_ship = ship_row::physical_ship_index::<serde_json::Error>(row)?;
    let location = row
        .pointer("/source/position")
        .filter(|cell| cell.get("state").and_then(Value::as_str) == Some("present"))
        .and_then(|cell| cell.get("value"))
        .ok_or("the Flight row lacks a present Loadout occurrence")?;
    if location.get("loadoutRecordIndex").and_then(Value::as_u64) != Some(u64::from(physical_ship))
    {
        return Err("the Flight occurrence belongs to another physical Ship".into());
    }
    let ordinals = location
        .get("portOrdinals")
        .and_then(Value::as_array)
        .filter(|path| {
            path.iter().all(|entry| {
                entry
                    .as_u64()
                    .is_some_and(|number| u32::try_from(number).is_ok())
            })
        })
        .ok_or("the Flight row lacks valid ordered port ordinals")?;
    Ok(fingerprint(serde_json::to_value((
        physical_ship,
        ordinals,
    ))?)?)
}

#[test]
fn flight_digest_requires_exact_occurrences_and_contributors() -> TestResult {
    use serde::de::DeserializeSeed as _;
    use serde_json::json;

    let row = |root, path: &[u32], reference| {
        json!({
            "shipRecordIndex": {"state": "present", "value": root},
            "shipGuid": {"state": "present", "value": "shared"},
            "source": {"position": {
                "state": "present",
                "value": {"loadoutRecordIndex": root, "portOrdinals": path},
                "sourceUsesRefs": [reference]
            }}
        })
    };
    let first = row(7, &[], "first");
    let second = row(7, &[1, 0], "second");
    let alias = row(4, &[], "alias");
    let mut rows = OrderedRows::with_encounter(EncounterRow::InstalledFlight);
    for original in [&first, &second, &alias] {
        rows.append_encounter_row(occurrence_key(original)?, fingerprint(original.clone())?)?;
    }
    let expected = rows.finish();
    assert_eq!(
        expected.deserialize(&json!([first, second, alias]))?,
        json!(3),
        "the physical root and Loadout encounter order define the exact fleet",
    );
    for changed in [
        json!([alias, second, first]),
        json!([first, second]),
        json!([first, row(7, &[0, 1], "second"), alias]),
        json!([first, row(7, &[1, 0], "different use"), alias]),
    ] {
        assert!(
            expected.deserialize(&changed).is_err(),
            "changed order, occurrences, or provenance must fail parity"
        );
    }
    let mut wrong_root = alias;
    wrong_root["source"]["position"]["value"]["loadoutRecordIndex"] = json!(7);
    assert!(
        occurrence_key(&wrong_root).is_err(),
        "a matching Ship GUID cannot replace physical Loadout root identity"
    );
    Ok(())
}
