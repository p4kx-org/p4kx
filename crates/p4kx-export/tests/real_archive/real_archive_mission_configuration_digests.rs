// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::mission_properties::{
    MissionFlowOwnerRef, MissionPropertyOwnerSnapshot,
};
use serde::de::{DeserializeSeed, Deserializer, Error as _};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::super::compare_rows::Projection;
use super::super::digest_parity::{capture_inventory, fingerprint};
use super::super::{BoundMeasurement, Counts, TestResult, collect_references, wire_row};
use super::digest_runs::{DigestRuns, Fingerprint};
use super::wire::{EncounterRow, OrderedRows};
use super::{StreamedComponentDigests, spill};

const KINDS: [&str; 2] = ["contractTemplates", "missionBrokerEntries"];
const HEALTH: &str = "/exportHealth/kinds";
const PATHS: [&str; 4] = [
    "/configurations/missionProperties",
    "/configurations/missionFlows",
    "/definitions/contractTemplates",
    "/definitions/missionBrokerEntries",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MissionDisposition {
    pub(crate) owners: usize,
    pub(crate) skipped: usize,
}

pub(crate) struct StreamedMissionDigests<'directory> {
    comparison: StreamedComponentDigests<'directory, 4>,
}

impl<'directory> StreamedMissionDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        let orders = [
            EncounterRow::MissionProperties,
            EncounterRow::MissionFlows,
            EncounterRow::MissionArena,
            EncounterRow::MissionArena,
        ];
        Self {
            comparison: StreamedComponentDigests {
                rows: orders.map(OrderedRows::with_encounter),
                uses: DigestRuns::new(directory),
                sources: DigestRuns::new(directory),
            },
        }
    }

    pub(crate) fn retain_owner(
        &mut self,
        snapshot: &MissionPropertyOwnerSnapshot<'_>,
    ) -> TestResult {
        let [properties, flows, templates, brokers] = &mut self.comparison.rows;
        let mut references = HashSet::new();
        for row in &snapshot.rows {
            retain(
                properties,
                EncounterRow::MissionProperties,
                row,
                &mut references,
            )?;
        }
        for row in &snapshot.flows {
            retain(flows, EncounterRow::MissionFlows, row, &mut references)?;
        }
        let arenas = match &snapshot.source.owner {
            MissionFlowOwnerRef::ContractTemplate(_) => templates,
            MissionFlowOwnerRef::MissionBrokerEntry(_) => brokers,
            _ => return Err("the mission arena has an unsupported physical owner family".into()),
        };
        let owner = snapshot.source.owner.physical();
        let row = ArenaRow {
            header: json!({
                "inputRef": owner.input_ref(),
                "recordIndex": {"state": "present", "value": owner.record_index()},
                "guid": {"state": "present", "value": owner.record_guid()},
            }),
            source: &snapshot.source.arena,
        };
        retain(arenas, EncounterRow::MissionArena, &row, &mut references)?;
        capture_inventory(
            &snapshot.source_inventory,
            references,
            |id, row| spill(&mut self.comparison.uses, &id, row),
            |id, row| spill(&mut self.comparison.sources, &id, row),
        )
    }
}

#[derive(Serialize)]
struct ArenaRow<'source, Source> {
    #[serde(flatten)]
    header: Value,
    source: &'source Source,
}

fn retain(
    ordered: &mut OrderedRows,
    shape: EncounterRow,
    row: &impl Serialize,
    references: &mut HashSet<String>,
) -> TestResult {
    let (key, wire) = shape.project(wire_row(row)?)?;
    let mut needed = HashSet::new();
    collect_references(&wire, &mut needed);
    for reference in needed {
        if !references.contains(reference) {
            references.insert(reference.to_owned());
        }
    }
    ordered.append_encounter_row(key, fingerprint(wire)?)
}

pub(super) fn project_mission_row(
    shape: EncounterRow,
    row: Value,
) -> TestResult<(Fingerprint, Value)> {
    match shape {
        EncounterRow::MissionProperties | EncounterRow::MissionFlows => {
            configuration_row(row, shape)
        }
        EncounterRow::MissionArena => arena_row(row),
        EncounterRow::Placement => Err("expected a mission comparison row".into()),
    }
}

fn configuration_row(row: Value, shape: EncounterRow) -> TestResult<(Fingerprint, Value)> {
    let owner = row
        .get("owner")
        .filter(|value| value.is_object())
        .ok_or("missing physical mission configuration owner")?;
    let declaration = row
        .get("declarationIndex")
        .ok_or("missing declaration position")?;
    let mut identity = vec![owner, declaration];
    if shape == EncounterRow::MissionProperties {
        identity.push(
            row.get("ownerObjective")
                .ok_or("missing property objective position")?,
        );
    }
    Ok((fingerprint(serde_json::to_value(identity)?)?, row))
}

fn arena_row(mut row: Value) -> TestResult<(Fingerprint, Value)> {
    let original = row
        .as_object_mut()
        .ok_or("a mission definition must be an object")?;
    let input = original
        .remove("inputRef")
        .filter(Value::is_string)
        .ok_or("missing physical input identity")?;
    let source = original
        .remove("source")
        .filter(Value::is_object)
        .ok_or("missing original mission source arena")?;
    let record_index = present_header(&row, "recordIndex")?
        .as_u64()
        .ok_or("missing original Record index")?;
    let guid = present_header(&row, "guid")?
        .as_str()
        .ok_or("missing original Record GUID")?;
    let owner = json!({"inputRef": input, "recordIndex": record_index, "guid": guid});
    let key = fingerprint(owner.clone())?;
    Ok((key, json!({"owner": owner, "source": source})))
}

fn present_header<'row>(row: &'row Value, field: &str) -> TestResult<&'row Value> {
    let cell = row
        .get(field)
        .ok_or("missing original mission header cell")?;
    if cell.get("state").and_then(Value::as_str) != Some("present") {
        return Err("unreadable original mission header cell".into());
    }
    Ok(cell
        .get("value")
        .ok_or("missing present mission header value")?)
}

impl BoundMeasurement {
    pub(crate) fn verify_streamed_mission_configuration(
        &self,
        availability: [DatasetAvailability; 4],
        dispositions: [MissionDisposition; 2],
        evidence: StreamedMissionDigests<'_>,
    ) -> TestResult<Counts<4>> {
        self.verify_streamed_tables(
            PATHS,
            availability,
            evidence.comparison,
            |capture| {
                capture.project(
                    HEALTH,
                    Projection::MissionDispositions(MissionDispositionRows),
                );
            },
            |catalog| verify_health(catalog, dispositions),
        )
    }
}

#[derive(Clone, Copy)]
pub(in super::super) struct MissionDispositionRows;

impl<'de> DeserializeSeed<'de> for MissionDispositionRows {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        let health = Vec::<KindHealth>::deserialize(deserializer)?;
        serde_json::to_value(
            health
                .into_iter()
                .filter(|kind| KINDS.contains(&kind.kind.as_str()))
                .collect::<Vec<_>>(),
        )
        .map_err(D::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct KindHealth {
    kind: String,
    source_inventory_complete: bool,
    source_records: RecordDispositions,
    written_rows: usize,
    intentionally_skipped_rows: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordDispositions {
    selected: usize,
    read_successful: usize,
    projected: usize,
    intentionally_skipped: usize,
    omitted_after_read_failure: usize,
    omitted_after_projection_failure: usize,
    failed_selection_candidates: usize,
}

fn verify_health(catalog: &Value, expected: [MissionDisposition; 2]) -> TestResult {
    let retained: Vec<KindHealth> = serde_json::from_value(
        catalog
            .pointer(HEALTH)
            .ok_or("missing retained mission dispositions")?
            .clone(),
    )?;
    if retained.len() != KINDS.len() {
        return Err("the retained kind ledger does not cover the two mission families".into());
    }
    for (kind, evidence) in KINDS.into_iter().zip(expected) {
        let entry = retained
            .iter()
            .find(|entry| entry.kind == kind)
            .ok_or_else(|| format!("missing retained disposition for {kind}"))?;
        let counts = &entry.source_records;
        let selected = evidence
            .owners
            .checked_add(evidence.skipped)
            .ok_or("mission disposition count overflow")?;
        if !entry.source_inventory_complete
            || entry.written_rows != evidence.owners
            || entry.intentionally_skipped_rows != 0
            || counts.selected != selected
            || counts.read_successful != evidence.owners
            || counts.projected != evidence.owners
            || counts.intentionally_skipped != evidence.skipped
            || counts.omitted_after_read_failure != 0
            || counts.omitted_after_projection_failure != 0
            || counts.failed_selection_candidates != 0
        {
            return Err(format!(
                "{kind}: retained selected {}, read {}, projected {}, skipped {}; original selected {selected}, projected {}, skipped {}",
                counts.selected, counts.read_successful, counts.projected, counts.intentionally_skipped, evidence.owners, evidence.skipped,
            ).into());
        }
    }
    Ok(())
}

#[path = "real_archive_mission_configuration_digest_tests.rs"]
mod tests;
