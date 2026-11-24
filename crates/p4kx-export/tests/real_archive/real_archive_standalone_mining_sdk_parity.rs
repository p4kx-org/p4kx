// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, MiningDefinitionSnapshot, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Build};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_rows, with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

const ELEMENTS: &str = "/mineableElements";
const COMPOSITIONS: &str = "/mineableCompositions";
const PROVIDERS: &str = "/harvestableProviderPresets";
const GLOBALS: &str = "/configurations/miningGlobal";
const REFINING: &str = "/definitions/refiningProcesses";
const RESOURCES: &str = "/definitions/resourceTypes";
const ENTITIES: &str = "/definitions/mineableEntities";

#[test]
#[ignore = "same Build: configure the Archive, completed Normal Catalog and exact Mining producer revision"]
fn installed_standalone_mining_sdk_matches_one_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_MINING_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Mining Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let mut origins = Origins::default();
        let sdk = read_sdk(archive, &mut origins)?;
        let [elements, compositions, providers] = &sdk.arrays;
        let arrays = [
            (ELEMENTS, elements.rows()),
            (COMPOSITIONS, compositions.rows()),
            (PROVIDERS, providers.rows()),
        ];
        let counts = measurement.verify_tables_and_arrays(
            [GLOBALS, REFINING, RESOURCES, ENTITIES],
            &sdk.tables,
            &arrays,
            &origins,
        )?;
        let [globals, refining, resources, entities] = counts.rows;
        print_line(&format!(
            "complete standalone Mining SDK parity: mineableElements={} mineableCompositions={} harvestableProviderPresets={} miningGlobal={globals} refiningProcesses={refining} resourceTypes={resources} mineableEntities={entities} sourceUses={} physicalSources={}",
            elements.rows().len(),
            compositions.rows().len(),
            providers.rows().len(),
            counts.source_uses,
            counts.physical_sources,
        ));
        Ok(())
    })
}

struct MiningRows {
    arrays: [Table; 3],
    tables: [Table; 4],
}

#[derive(Clone, Copy)]
pub(crate) enum MiningDataset {
    Elements,
    Compositions,
    Providers,
    Globals,
    Refining,
    Resources,
    Entities,
}

impl MiningDataset {
    pub(crate) const ALL: [Self; 7] = [
        Self::Elements,
        Self::Compositions,
        Self::Providers,
        Self::Globals,
        Self::Refining,
        Self::Resources,
        Self::Entities,
    ];

    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::Elements => ELEMENTS,
            Self::Compositions => COMPOSITIONS,
            Self::Providers => PROVIDERS,
            Self::Globals => GLOBALS,
            Self::Refining => REFINING,
            Self::Resources => RESOURCES,
            Self::Entities => ENTITIES,
        }
    }
}

fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<MiningRows> {
    let session = CatalogQuerySession::open(archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let mut read = |dataset| capture_dataset(archive, &parsed, dataset, origins);
    Ok(MiningRows {
        arrays: [
            read(MiningDataset::Elements)?,
            read(MiningDataset::Compositions)?,
            read(MiningDataset::Providers)?,
        ],
        tables: [
            read(MiningDataset::Globals)?,
            read(MiningDataset::Refining)?,
            read(MiningDataset::Resources)?,
            read(MiningDataset::Entities)?,
        ],
    })
}

pub(crate) fn capture_dataset(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    dataset: MiningDataset,
    origins: &mut Origins,
) -> TestResult<Table> {
    let all = CatalogRecordSelection::All;
    let path = dataset.path();
    match dataset {
        MiningDataset::Elements => parsed.with_mineable_elements(all, None, |snapshot| {
            capture_definition(path, archive, &snapshot, origins)
        })?,
        MiningDataset::Compositions => {
            parsed.with_mineable_compositions(all, None, |snapshot| {
                capture_definition(path, archive, &snapshot, origins)
            })?
        }
        MiningDataset::Providers => {
            let providers = parsed.read_harvestable_provider_presets(all, None)?;
            check_owner_scope(
                path,
                archive,
                providers.build.as_ref(),
                providers.availability,
                &providers.coverage,
            )?;
            if providers.rows.is_empty() || providers.unreadable_dependencies != 0 {
                return Err(
                    format!("{path}: expected provider rows with complete dependencies").into(),
                );
            }
            capture_rows(
                providers.availability,
                &providers.rows,
                &providers.source_inventory,
                origins,
                record_index,
            )
        }
        MiningDataset::Globals => parsed.with_mining_global_params(all, None, |snapshot| {
            capture_definition(path, archive, &snapshot, origins)
        })?,
        MiningDataset::Refining => parsed.with_refining_processes(all, None, |snapshot| {
            capture_definition(path, archive, &snapshot, origins)
        })?,
        MiningDataset::Resources => parsed.with_resource_types(all, None, |snapshot| {
            capture_definition(path, archive, &snapshot, origins)
        })?,
        MiningDataset::Entities => parsed.with_mineable_entities(all, None, |snapshot| {
            capture_definition(path, archive, &snapshot, origins)
        })?,
    }
}

fn capture_definition<Row: Serialize>(
    path: &str,
    archive: &Archive,
    snapshot: &MiningDefinitionSnapshot<'_, Row>,
    origins: &mut Origins,
) -> TestResult<Table> {
    check_owner_scope(
        path,
        archive,
        snapshot.build(),
        snapshot.availability(),
        snapshot.coverage(),
    )?;
    if snapshot.rows().is_empty() {
        return Err(format!("{path}: expected installed Mining Record rows").into());
    }
    if !complete(snapshot.dependency_coverage()) {
        return Err(format!("{path}: incomplete original dependency coverage").into());
    }
    capture_rows(
        snapshot.availability(),
        snapshot.rows(),
        snapshot.source_inventory(),
        origins,
        record_index,
    )
}

fn check_owner_scope(
    path: &str,
    archive: &Archive,
    build: Option<&Build>,
    availability: DatasetAvailability,
    coverage: &CatalogRecordCoverage,
) -> TestResult {
    if build.is_none()
        || build != archive.build()
        || !matches!(availability, DatasetAvailability::Available)
        || !complete(coverage)
    {
        return Err(
            format!("{path}: requires complete original coverage of the matching Build").into(),
        );
    }
    Ok(())
}

const fn complete(coverage: &CatalogRecordCoverage) -> bool {
    coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
}

fn record_index(row: &Value) -> TestResult<u64> {
    row.pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or_else(|| "expected an original Mining Record index".into())
}
