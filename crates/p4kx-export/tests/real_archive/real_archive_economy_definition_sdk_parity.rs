// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::commodities::CommoditySnapshot;
use p4kx_export::catalog_queries::quality::QualityDefinitionSnapshot;
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogRecordSelection,
    GameplayPropertySnapshot, ManufacturerSnapshot, ParsedCatalogQueries,
};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{TestResult, with_bound_measurement_from_producer};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification::print_line;

#[path = "real_archive_economy_definition_evidence.rs"]
mod evidence;

use evidence::{Census, Evidence, RecordScope};

const EXPECTED: &str = "P4KX_ECONOMY_DEFINITION_SDK_EXPECTED_CENSUS";
const COLD_DATASET: &str = "P4KX_ECONOMY_DEFINITION_SDK_COLD_DATASET";
const PRODUCER: &str = "P4KX_ECONOMY_DEFINITION_CATALOG_PRODUCER_REVISION";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    Manufacturers,
    Commodities,
    CommodityPresentation,
    CargoManifests,
    GameplayProperties,
    QualityDistributions,
    QualityQuantizations,
}

const COUNT: usize = 7;
const FAMILIES: [Family; COUNT] = [
    Family::Manufacturers,
    Family::Commodities,
    Family::CommodityPresentation,
    Family::CargoManifests,
    Family::GameplayProperties,
    Family::QualityDistributions,
    Family::QualityQuantizations,
];
const QUERIES: [Family; 6] = [
    Family::Manufacturers,
    Family::Commodities,
    Family::CargoManifests,
    Family::GameplayProperties,
    Family::QualityDistributions,
    Family::QualityQuantizations,
];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Manufacturers => "/definitions/manufacturers",
            Self::Commodities => "/definitions/commodities",
            Self::CommodityPresentation => "/projections/commodityPresentation",
            Self::CargoManifests => "/definitions/cargoManifests",
            Self::GameplayProperties => "/definitions/gameplayProperties",
            Self::QualityDistributions => "/definitions/qualityDistributions",
            Self::QualityQuantizations => "/definitions/qualityQuantizations",
        }
    }

    const fn languages(self) -> CatalogLanguageSelection<'static> {
        match self {
            Self::Manufacturers
            | Self::Commodities
            | Self::CommodityPresentation
            | Self::GameplayProperties => CatalogLanguageSelection::AllAvailable,
            _ => CatalogLanguageSelection::Selected(&[]),
        }
    }
}

#[test]
#[ignore = "standalone cold query: configure the installed Build and one economy route"]
fn installed_economy_definitions_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact economy dataset path")?;
    let expected = Census::parse(&env::var(EXPECTED)?)?;
    equipment_input::with_archive(|archive| {
        let session = equipment_input::full_source_session(archive, family.languages())?;
        let parsed = session.parsed(None)?;
        let mut evidence = Evidence::cold(archive, expected);
        observe(&parsed, family, &mut evidence)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_economy_definitions_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = Census::parse(&env::var(EXPECTED)?)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let directory = tempfile::tempdir()?;
        let mut evidence = Evidence::retained(archive, expected, directory.path());
        {
            let session = equipment_input::full_source_session(
                archive,
                CatalogLanguageSelection::AllAvailable,
            )?;
            let parsed = session.parsed(None)?;
            for family in QUERIES {
                observe(&parsed, family, &mut evidence)?;
            }
        }
        let counts = evidence.compare(measurement)?;
        print_line(&format!(
            "complete economy definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
            counts.rows, counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

fn observe(
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    evidence: &mut Evidence<'_>,
) -> TestResult {
    let all = CatalogRecordSelection::All;
    match family {
        Family::Manufacturers => parsed.with_manufacturers(all, None, |snapshot| {
            observe_manufacturers(&snapshot, evidence)
        })?,
        Family::Commodities | Family::CommodityPresentation => {
            parsed.with_commodities(all, None, |snapshot| {
                observe_commodities(&snapshot, evidence)
            })?
        }
        Family::CargoManifests => parsed.with_cargo_manifests(all, None, |snapshot| {
            let scope = RecordScope {
                build: snapshot.build.as_ref(),
                availability: snapshot.availability,
                inventory: &snapshot.source_inventory,
                coverage: &snapshot.coverage,
            };
            evidence.records(family, &scope, &snapshot.rows, |row| {
                record_index(row.source())
            })
        })?,
        Family::GameplayProperties => {
            observe_properties(&parsed.read_gameplay_properties(all, None)?, evidence)
        }
        Family::QualityDistributions => {
            parsed.with_quality_distributions(all, None, |snapshot| {
                observe_quality(family, &snapshot, evidence, |row| {
                    record_index(row.source())
                })
            })?
        }
        Family::QualityQuantizations => {
            parsed.with_quality_quantizations(all, None, |snapshot| {
                observe_quality(family, &snapshot, evidence, |row| {
                    record_index(row.source())
                })
            })?
        }
    }
}

const fn record_index(header: &CanonicalRecordHeader<'_>) -> Option<u32> {
    header.record_index().value().copied()
}

fn observe_quality<Row: Serialize>(
    family: Family,
    snapshot: &QualityDefinitionSnapshot<Row>,
    evidence: &mut Evidence<'_>,
    select: impl Fn(&Row) -> Option<u32>,
) -> TestResult {
    let scope = RecordScope {
        build: snapshot.build(),
        availability: snapshot.availability(),
        inventory: snapshot.source_inventory(),
        coverage: snapshot.coverage(),
    };
    evidence.records(family, &scope, snapshot.rows(), select)
}

fn observe_manufacturers(
    snapshot: &ManufacturerSnapshot<'_>,
    evidence: &mut Evidence<'_>,
) -> TestResult {
    let scope = RecordScope {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        inventory: &snapshot.source_inventory,
        coverage: &snapshot.coverage,
    };
    evidence.records(Family::Manufacturers, &scope, &snapshot.rows, |row| {
        record_index(row.source())
    })?;
    evidence.texts(
        &snapshot.public_names,
        &snapshot.game_texts,
        &snapshot.source_inventory,
        &snapshot.requested_languages,
        &snapshot.available_languages,
    )
}

fn observe_properties(
    snapshot: &GameplayPropertySnapshot,
    evidence: &mut Evidence<'_>,
) -> TestResult {
    let scope = RecordScope {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        inventory: &snapshot.source_inventory,
        coverage: &snapshot.coverage,
    };
    evidence.records(Family::GameplayProperties, &scope, &snapshot.rows, |row| {
        row.source.record_index.value().copied()
    })?;
    evidence.texts(
        &snapshot.public_names,
        &[] as &[Value],
        &snapshot.source_inventory,
        &snapshot.requested_languages,
        &snapshot.available_languages,
    )
}

fn observe_commodities(
    snapshot: &CommoditySnapshot<'_>,
    evidence: &mut Evidence<'_>,
) -> TestResult {
    let scope = RecordScope {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        inventory: &snapshot.source_inventory,
        coverage: &snapshot.coverage,
    };
    evidence::require_readable("Commodity dependency", &snapshot.dependency_coverage)?;
    evidence.records(Family::Commodities, &scope, &snapshot.definitions, |row| {
        record_index(row.source())
    })?;
    evidence.presentation(&scope, snapshot)?;
    evidence.texts(
        &snapshot.public_names,
        &snapshot.game_texts,
        &snapshot.source_inventory,
        &snapshot.requested_languages,
        &snapshot.available_languages,
    )
}
