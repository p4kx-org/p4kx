// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::loot::{
    LegacyLootSnapshot, LootDefinitionKind, LootDefinitionSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Build, FieldPath, SourceInventory};
use serde::Serialize;

use crate::real_archive_catalog_table_parity::{
    LootHarvestDataset as Dataset, LootHarvestDigests, TestResult,
    with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification::print_line;

const EXPECTED: &str = "P4KX_LOOT_HARVEST_SDK_EXPECTED_CENSUS";
const COLD_DATASET: &str = "P4KX_LOOT_HARVEST_SDK_COLD_DATASET";
const PRODUCER: &str = "P4KX_LOOT_HARVEST_CATALOG_PRODUCER_REVISION";
const RETAINED_QUERIES: [Dataset; 7] = [
    Dataset::ContextPresets,
    Dataset::HarvestConfigs,
    Dataset::WeightedTables,
    Dataset::WeightedArchetypes,
    Dataset::LegacyTables,
    Dataset::LegacyArchetypes,
    Dataset::HarvestMultiConfigs,
];

#[test]
#[ignore = "same Build: configure the installed Archive and completed Normal producer"]
fn installed_loot_harvest_matches_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = Census::read()?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let directory = tempfile::tempdir()?;
        let mut proof = Proof {
            archive,
            expected,
            retained: Some(LootHarvestDigests::new(directory.path())),
        };
        {
            let session = equipment_input::full_source_session(
                archive,
                CatalogLanguageSelection::Selected(&[]),
            )?;
            let parsed = session.parsed(None)?;
            for dataset in RETAINED_QUERIES {
                observe(&parsed, dataset, &mut proof)?;
            }
        }
        let evidence = proof
            .retained
            .ok_or("missing retained Loot and harvest evidence")?;
        let count = evidence.compare(measurement)?;
        let rows = [count.original.rows.as_slice(), count.arrays.as_slice()].concat();
        print_line(&format!(
            "complete loot and harvest SDK parity: rows={rows:?} sourceUses={} physicalSources={} slotPresetEvidence=source",
            count.original.source_uses, count.original.physical_sources,
        ));
        Ok(())
    })
}

#[test]
#[ignore = "standalone cold query: configure the installed Build and one exact dataset"]
fn installed_loot_harvest_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let dataset = Dataset::ALL
        .into_iter()
        .find(|value| value.path() == path)
        .ok_or("configure one exact Loot or harvest dataset")?;
    let expected = Census::read()?;
    equipment_input::with_archive(|archive| {
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        let mut proof = Proof {
            archive,
            expected,
            retained: None,
        };
        observe(&parsed, dataset, &mut proof)
    })
}

fn observe(
    parsed: &ParsedCatalogQueries<'_>,
    dataset: Dataset,
    proof: &mut Proof<'_>,
) -> TestResult {
    let all = CatalogRecordSelection::All;
    let family = match dataset {
        Dataset::LegacyTables => {
            return parsed.with_legacy_loot_tables(all, None, |snapshot| {
                observe_legacy(dataset, &snapshot, proof)
            })?;
        }
        Dataset::LegacyArchetypes => {
            return parsed.with_legacy_loot_archetypes(all, None, |snapshot| {
                observe_legacy(dataset, &snapshot, proof)
            })?;
        }
        Dataset::ContextPresets => LootDefinitionKind::HierarchicalContextualDataPreset,
        Dataset::HarvestConfigs => LootDefinitionKind::SubHarvestableConfig,
        Dataset::HarvestMultiConfigs | Dataset::SlotPresets => {
            LootDefinitionKind::SubHarvestableMultiConfig
        }
        Dataset::WeightedTables => LootDefinitionKind::Table,
        Dataset::WeightedArchetypes => LootDefinitionKind::Archetype,
    };
    parsed.with_loot_definitions(family, all, None, |snapshot| {
        observe_loot(dataset, &snapshot, proof)
    })?
}

fn observe_legacy<Row: Serialize>(
    dataset: Dataset,
    snapshot: &LegacyLootSnapshot<Row>,
    proof: &mut Proof<'_>,
) -> TestResult {
    let scope = Scope {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        inventory: &snapshot.source_inventory,
        coverage: &snapshot.coverage,
    };
    proof.check(dataset, &scope, &snapshot.rows)
}

fn observe_loot(
    dataset: Dataset,
    snapshot: &LootDefinitionSnapshot<'_>,
    proof: &mut Proof<'_>,
) -> TestResult {
    let scope = Scope {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        inventory: &snapshot.source_inventory,
        coverage: &snapshot.coverage,
    };
    match dataset {
        Dataset::WeightedTables => {
            verify_weighted_owners(snapshot, snapshot.tables.iter().map(|row| row.record_index))?;
            proof.check(dataset, &scope, &snapshot.tables)
        }
        Dataset::WeightedArchetypes => {
            verify_weighted_owners(
                snapshot,
                snapshot.archetypes.iter().map(|row| row.record_index),
            )?;
            proof.check(dataset, &scope, &snapshot.archetypes)
        }
        Dataset::HarvestMultiConfigs | Dataset::SlotPresets => {
            verify_slot_preset_owners(snapshot)?;
            proof.check(Dataset::HarvestMultiConfigs, &scope, &snapshot.rows)?;
            proof.check(
                Dataset::SlotPresets,
                &scope,
                &snapshot.sub_harvestable_multi_configs,
            )
        }
        _ => proof.check(dataset, &scope, &snapshot.rows),
    }
}

fn verify_weighted_owners(
    snapshot: &LootDefinitionSnapshot<'_>,
    projected: impl Iterator<Item = u32>,
) -> TestResult {
    let mut owners = HashSet::new();
    for row in &snapshot.rows {
        let index = row
            .source()
            .record_index()
            .value()
            .copied()
            .ok_or("missing original Loot owner")?;
        if !owners.insert(index) {
            return Err("duplicate original Loot owner".into());
        }
    }
    for index in projected {
        if !owners.remove(&index) {
            return Err("a weighted Loot row has no unique original owner".into());
        }
    }
    if !owners.is_empty() {
        return Err("an original Loot owner lacks its weighted row".into());
    }
    Ok(())
}

fn verify_slot_preset_owners(snapshot: &LootDefinitionSnapshot<'_>) -> TestResult {
    if snapshot.harvest_dependency_warning.is_some() {
        return Err("a required slot-preset dependency read failed".into());
    }
    let mut owners = HashSet::new();
    for row in &snapshot.rows {
        let header = row.source();
        let guid = header
            .guid()
            .value()
            .copied()
            .ok_or("missing original harvest owner GUID")?;
        let path = header
            .record_path()
            .value()
            .copied()
            .ok_or("missing original harvest owner path")?;
        if !owners.insert((guid, path)) {
            return Err("ambiguous projected harvest owner identity".into());
        }
    }
    for row in &snapshot.sub_harvestable_multi_configs {
        if !owners.remove(&(row.guid.get(), row.record_path)) {
            return Err("a projected slot preset has no unique original GUID and path".into());
        }
    }
    verify_slot_preset_origins(snapshot)
}

fn verify_slot_preset_origins(snapshot: &LootDefinitionSnapshot<'_>) -> TestResult {
    for (pointer, is_header) in [
        ("/guid", true),
        ("/className", true),
        ("/recordPath", true),
        ("/defaultConfigGuid", false),
        ("/ignoreAttachableTags", false),
        ("/taggedConfigs", false),
    ] {
        let path = FieldPath::parse(pointer)?;
        for row in &snapshot.sub_harvestable_multi_configs {
            let origin = row.field_origins.get(&path).ok_or_else(|| {
                format!("{} has no slot preset origin at {pointer}", row.record_path)
            })?;
            if is_header && (!origin.source_set_complete() || origin.source_uses_refs().is_empty())
            {
                return Err(format!(
                    "{} has no complete slot preset header origin at {pointer}",
                    row.record_path,
                )
                .into());
            }
        }
    }
    Ok(())
}

struct Census([(usize, usize); Dataset::ALL.len()]);

impl Census {
    fn read() -> TestResult<Self> {
        let raw = env::var(EXPECTED)?;
        let counts = raw
            .split(',')
            .map(|item| {
                let (count, leftovers) = item.split_once(':').ok_or("expected rows:skipped")?;
                Ok((count.parse()?, leftovers.parse()?))
            })
            .collect::<TestResult<Vec<_>>>()?;
        Ok(Self(
            counts
                .try_into()
                .map_err(|_| "configure all eight exact route counts")?,
        ))
    }

    fn expected(&self, dataset: Dataset) -> TestResult<(usize, usize)> {
        let index = Dataset::ALL
            .iter()
            .position(|value| *value == dataset)
            .ok_or("unknown Loot route")?;
        self.0
            .get(index)
            .copied()
            .ok_or_else(|| "missing sealed route count".into())
    }
}

struct Scope<'read> {
    build: Option<&'read Build>,
    availability: DatasetAvailability,
    inventory: &'read SourceInventory,
    coverage: &'read CatalogRecordCoverage,
}

struct Proof<'archive> {
    archive: &'archive Archive,
    expected: Census,
    retained: Option<LootHarvestDigests<'archive>>,
}

impl Proof<'_> {
    fn check(
        &mut self,
        dataset: Dataset,
        scope: &Scope<'_>,
        rows: &[impl Serialize],
    ) -> TestResult {
        let path = dataset.path();
        let count = rows.len();
        let (expected, leftovers) = self.expected.expected(dataset)?;
        let uses = scope.inventory.use_count();
        let sources = scope.inventory.source_count();
        let coverage = scope.coverage;
        if scope.build.is_none()
            || scope.build != self.archive.build()
            || !matches!(
                (scope.availability, count),
                (DatasetAvailability::Available, _) | (DatasetAvailability::Absent, 0)
            )
            || count != expected
            || coverage.skipped_leftovers != leftovers
            || !coverage.unreadable_headers.is_empty()
            || !coverage.unreadable_bodies.is_empty()
            || !coverage.incomplete_fields.is_empty()
            || (count != 0 && (uses == 0 || sources == 0))
        {
            return Err(format!(
                "{path}: expected {expected} rows and {leftovers} exclusions, observed {count} {:?}, skipped={}, unreadableHeaders={} unreadableBodies={} incompleteFields={} inventoryUses={uses} inventorySources={sources}",
                scope.availability, coverage.skipped_leftovers, coverage.unreadable_headers.len(),
                coverage.unreadable_bodies.len(), coverage.incomplete_fields.len(),
            ).into());
        }
        if let Some(proof) = &mut self.retained {
            proof.retain(dataset, scope.availability, rows, scope.inventory)?;
        }

        print_line(&format!(
            "Loot and harvest SDK: dataset={path} availability={:?} rowEvidence=source rows={count} inventoryUses={uses} inventorySources={sources} skippedLeftovers={leftovers}",
            scope.availability,
        ));
        Ok(())
    }
}
