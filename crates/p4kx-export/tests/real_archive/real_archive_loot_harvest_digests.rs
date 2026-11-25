// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_extraction::SourceInventory;
use serde::Serialize;
use serde_json::Value;

use super::super::compare_rows::Projection;
use super::super::digest_parity::{capture_inventory, fingerprint};
use super::super::{BoundMeasurement, Counts, TestResult, collect_references, wire_row};
use super::digest_runs::{DigestRuns, Fingerprint};
use super::wire::{EncounterRow, OrderedRows, RowDigest};
use super::{StreamedComponentDigests, spill};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LootHarvestDataset {
    ContextPresets,
    HarvestConfigs,
    HarvestMultiConfigs,
    WeightedTables,
    WeightedArchetypes,
    LegacyTables,
    LegacyArchetypes,
    SlotPresets,
}

const TABLES: [LootHarvestDataset; 3] = [
    LootHarvestDataset::ContextPresets,
    LootHarvestDataset::HarvestConfigs,
    LootHarvestDataset::HarvestMultiConfigs,
];
const ARRAYS: [LootHarvestDataset; 5] = [
    LootHarvestDataset::WeightedTables,
    LootHarvestDataset::WeightedArchetypes,
    LootHarvestDataset::LegacyTables,
    LootHarvestDataset::LegacyArchetypes,
    LootHarvestDataset::SlotPresets,
];

impl LootHarvestDataset {
    pub(crate) const ALL: [Self; TABLES.len() + ARRAYS.len()] = [
        Self::ContextPresets,
        Self::HarvestConfigs,
        Self::HarvestMultiConfigs,
        Self::WeightedTables,
        Self::WeightedArchetypes,
        Self::LegacyTables,
        Self::LegacyArchetypes,
        Self::SlotPresets,
    ];

    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::ContextPresets => "/definitions/hierarchicalContextualDataPresets",
            Self::HarvestConfigs => "/definitions/subHarvestableConfigs",
            Self::HarvestMultiConfigs => "/definitions/subHarvestableMultiConfigs",
            Self::WeightedTables => "/lootTables",
            Self::WeightedArchetypes => "/lootArchetypes",
            Self::LegacyTables => "/legacyLootTables",
            Self::LegacyArchetypes => "/legacyLootArchetypes",
            Self::SlotPresets => "/subHarvestableMultiConfigs",
        }
    }

    const fn shape(self) -> EncounterRow {
        match self {
            Self::WeightedTables | Self::WeightedArchetypes => EncounterRow::LootWeighted,
            Self::SlotPresets => EncounterRow::HarvestSlotPreset,
            _ => EncounterRow::LootOriginal,
        }
    }
}

pub(crate) struct LootHarvestDigests<'directory> {
    original: StreamedComponentDigests<'directory, { TABLES.len() }>,
    arrays: [OrderedRows; ARRAYS.len()],
    available: [Option<DatasetAvailability>; TABLES.len()],
    visited: Vec<LootHarvestDataset>,
}

pub(crate) struct LootHarvestCounts {
    pub(crate) original: Counts<{ TABLES.len() }>,
    pub(crate) arrays: [usize; ARRAYS.len()],
}

impl<'directory> LootHarvestDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        Self {
            original: StreamedComponentDigests {
                rows: TABLES.map(|dataset| OrderedRows::with_encounter(dataset.shape())),
                uses: DigestRuns::new(directory),
                sources: DigestRuns::new(directory),
            },
            arrays: ARRAYS.map(|dataset| OrderedRows::with_encounter(dataset.shape())),
            available: [None; TABLES.len()],
            visited: Vec::with_capacity(LootHarvestDataset::ALL.len()),
        }
    }

    pub(crate) fn retain(
        &mut self,
        dataset: LootHarvestDataset,
        availability: DatasetAvailability,
        rows: &[impl Serialize],
        inventory: &SourceInventory,
    ) -> TestResult {
        if self.visited.contains(&dataset) {
            return Err(format!("duplicate SDK proof for {}", dataset.path()).into());
        }
        let ordered = if let Some(position) = TABLES.iter().position(|value| *value == dataset) {
            let state = self
                .available
                .get_mut(position)
                .ok_or("missing Loot table state")?;
            *state = Some(availability);
            self.original
                .rows
                .get_mut(position)
                .ok_or("missing Loot table rows")?
        } else {
            let position = ARRAYS
                .iter()
                .position(|value| *value == dataset)
                .ok_or("unknown Loot array")?;
            self.arrays
                .get_mut(position)
                .ok_or("missing Loot array rows")?
        };
        let referenced = retain_rows(dataset, rows, ordered)?;
        if referenced.is_empty() {
            if !rows.is_empty() {
                return Err(format!("{} has no contributing source uses", dataset.path()).into());
            }
        } else {
            capture_inventory(
                inventory,
                referenced,
                |id, row| spill(&mut self.original.uses, &id, row),
                |id, row| spill(&mut self.original.sources, &id, row),
            )
            .map_err(|error| format!("{}: {error}", dataset.path()))?;
        }
        self.visited.push(dataset);
        Ok(())
    }

    pub(crate) fn compare(self, measurement: &BoundMeasurement) -> TestResult<LootHarvestCounts> {
        if self.visited.len() != LootHarvestDataset::ALL.len() {
            return Err("an exact Loot or harvest route is missing from the SDK comparison".into());
        }
        let [Some(context), Some(single), Some(multi)] = self.available else {
            return Err("an original Loot or harvest table availability was not recorded".into());
        };
        let arrays = self.arrays.map(OrderedRows::finish);
        let original = measurement.verify_streamed_tables(
            TABLES.map(LootHarvestDataset::path),
            [context, single, multi],
            self.original,
            |capture| {
                for (dataset, rows) in ARRAYS.into_iter().zip(arrays) {
                    capture.project(dataset.path(), Projection::StreamRows(rows));
                }
            },
            |catalog| verify_array_sizes(catalog, &arrays),
        )?;
        Ok(LootHarvestCounts {
            original,
            arrays: arrays.map(|row| row.count),
        })
    }
}

fn retain_rows(
    dataset: LootHarvestDataset,
    rows: &[impl Serialize],
    ordered: &mut OrderedRows,
) -> TestResult<HashSet<String>> {
    let mut identities = HashSet::new();
    let mut referenced = HashSet::new();
    for row in rows {
        let wire = wire_row(row)?;
        let key = loot_harvest_key(dataset.shape(), &wire)?;
        if !identities.insert(key) {
            return Err(format!("{} repeats an original row identity", dataset.path()).into());
        }
        let mut row_refs = HashSet::new();
        collect_references(&wire, &mut row_refs);
        for reference in row_refs {
            if !referenced.contains(reference) {
                referenced.insert(reference.to_owned());
            }
        }
        ordered.append_encounter_row(key, fingerprint(wire)?)?;
    }
    Ok(referenced)
}

fn verify_array_sizes(catalog: &Value, expected: &[RowDigest; ARRAYS.len()]) -> TestResult {
    for (dataset, rows) in ARRAYS.into_iter().zip(expected) {
        if catalog.pointer(dataset.path()) != Some(&Value::from(rows.count)) {
            return Err(format!("{} is missing or incomplete", dataset.path()).into());
        }
    }
    Ok(())
}

pub(super) fn loot_harvest_key(shape: EncounterRow, row: &Value) -> TestResult<Fingerprint> {
    let value = match shape {
        EncounterRow::LootOriginal => {
            if row.pointer("/recordIndex/state").and_then(Value::as_str) != Some("present") {
                return Err("an original Loot row has no present physical Record index".into());
            }
            record_index(row.pointer("/recordIndex/value"))?
        }
        EncounterRow::LootWeighted => record_index(row.get("recordIndex"))?,
        EncounterRow::HarvestSlotPreset => {
            let guid = row
                .get("guid")
                .and_then(Value::as_str)
                .ok_or("missing harvest preset GUID")?;
            let path = row
                .get("recordPath")
                .and_then(Value::as_str)
                .ok_or("missing harvest preset Record path")?;
            serde_json::to_value((guid, path))?
        }
        _ => return Err("unknown Loot or harvest row layout".into()),
    };
    Ok(fingerprint(value)?)
}

fn record_index(value: Option<&Value>) -> TestResult<Value> {
    let index = value
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or("missing or invalid physical Loot Record index")?;
    Ok(Value::from(index))
}

#[test]
fn loot_harvest_comparison_retains_physical_and_projected_identity() -> TestResult {
    use serde_json::json;

    let sourced = json!({"recordIndex":{"state":"present","value":5}});
    let weighted = json!({"recordIndex":5});
    let aliases = [
        json!({"guid":"same","recordPath":"first"}),
        json!({"guid":"same","recordPath":"second"}),
    ];
    assert_eq!(
        loot_harvest_key(EncounterRow::LootOriginal, &sourced)?,
        loot_harvest_key(EncounterRow::LootWeighted, &weighted)?,
        "both published views identify the same original Record",
    );
    assert!(
        loot_harvest_key(EncounterRow::LootOriginal, &weighted).is_err(),
        "an original row must retain its sourced header cell",
    );
    let [first, second] = &aliases;
    assert_ne!(
        loot_harvest_key(EncounterRow::HarvestSlotPreset, first)?,
        loot_harvest_key(EncounterRow::HarvestSlotPreset, second)?,
        "projected GUID aliases retain their authored record paths",
    );
    Ok(())
}

#[test]
fn loot_harvest_comparison_accepts_empty_routes() -> TestResult {
    use p4kx_extraction::SourceUses;

    let directory = tempfile::tempdir()?;
    let mut proof = LootHarvestDigests::new(directory.path());
    let inventory = SourceUses::new().freeze()?;
    let no_rows: &[Value] = &[];
    proof.retain(
        LootHarvestDataset::ContextPresets,
        DatasetAvailability::Absent,
        no_rows,
        &inventory,
    )?;
    proof.retain(
        LootHarvestDataset::HarvestConfigs,
        DatasetAvailability::Available,
        no_rows,
        &inventory,
    )?;
    proof.retain(
        LootHarvestDataset::SlotPresets,
        DatasetAvailability::Available,
        no_rows,
        &inventory,
    )?;
    assert_eq!(
        proof.visited.len(),
        3,
        "empty source closures keep their exact route populations in the comparison"
    );
    Ok(())
}

#[test]
fn loot_harvest_comparison_requires_every_claimed_source() -> TestResult {
    use p4kx_extraction::SourceUses;
    use serde_json::json;

    let directory = tempfile::tempdir()?;
    let mut proof = LootHarvestDigests::new(directory.path());
    let inventory = SourceUses::new().freeze()?;
    let authored = [json!({"recordIndex":{"state":"present","value":5}})];
    let result = proof.retain(
        LootHarvestDataset::HarvestConfigs,
        DatasetAvailability::Available,
        &authored,
        &inventory,
    );
    let Err(error) = result else {
        return Err("an authored route accepted rows without contributors".into());
    };
    assert_eq!(
        error.to_string(),
        "/definitions/subHarvestableConfigs has no contributing source uses",
        "a nonempty original route needs row-level source uses",
    );

    let value_only_slot = [json!({"guid":"slot", "recordPath":"slot-presets/one"})];
    let result = proof.retain(
        LootHarvestDataset::SlotPresets,
        DatasetAvailability::Available,
        &value_only_slot,
        &inventory,
    );
    let Err(error) = result else {
        return Err("a slot preset accepted a value-only row".into());
    };
    assert_eq!(
        error.to_string(),
        "/subHarvestableMultiConfigs has no contributing source uses",
        "a nonempty projection needs field-level source uses",
    );

    let slot = [json!({
        "guid":"slot",
        "recordPath":"slot-presets/one",
        "fieldOrigins":{"/guid":{"sourceUsesRefs":["missing"]}}
    })];
    let result = proof.retain(
        LootHarvestDataset::SlotPresets,
        DatasetAvailability::Available,
        &slot,
        &inventory,
    );
    let Err(error) = result else {
        return Err("a projection accepted a dangling source-use reference".into());
    };
    assert_eq!(
        error.to_string(),
        "/subHarvestableMultiConfigs: a value lacks its referenced SDK source use",
        "every emitted projection source use is resolved when present",
    );
    Ok(())
}
