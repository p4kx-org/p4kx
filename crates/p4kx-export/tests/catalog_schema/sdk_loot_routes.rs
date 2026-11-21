// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::loot::LootDefinitionKind;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

use super::{Route, TestResult};

const TABLES: &str = "/lootTables";
const ARCHETYPES: &str = "/lootArchetypes";
const REMAINING: &str =
    "physical selection and synthetic catalog parity available; installed producer parity open";

struct Fixture {
    family: LootDefinitionKind,
    path: &'static str,
    source_type: &'static str,
    record_path: &'static str,
}

const FIXTURES: [Fixture; 10] = [
    Fixture {
        family: LootDefinitionKind::Table,
        path: "/definitions/lootTables",
        source_type: "LootTableV3Record",
        record_path: "libs/foundry/records/lootgeneration/loottables/fixture.xml",
    },
    Fixture {
        family: LootDefinitionKind::Archetype,
        path: "/definitions/lootArchetypes",
        source_type: "LootArchetypeV3Record",
        record_path: "libs/foundry/records/lootgeneration/lootarchetypes/fixture.xml",
    },
    Fixture {
        family: LootDefinitionKind::PoolFilter,
        path: "/definitions/lootPoolFilters",
        source_type: "PoolFilterRecord",
        record_path: "libs/foundry/records/lootgeneration/filters/fixture.xml",
    },
    fixture(
        LootDefinitionKind::SecondaryChoiceSingleLayer,
        "/definitions/lootSecondaryChoiceSingleLayers",
        "LootV3SecondaryChoicesSingleLayerRecord",
    ),
    fixture(
        LootDefinitionKind::SecondaryChoiceMultiLayer,
        "/definitions/lootSecondaryChoiceMultiLayers",
        "LootV3SecondaryChoicesMultiLayerRecord",
    ),
    fixture(
        LootDefinitionKind::Theme,
        "/definitions/lootThemes",
        "LootThemeRecord",
    ),
    fixture(
        LootDefinitionKind::HierarchicalContextualDataPreset,
        "/definitions/hierarchicalContextualDataPresets",
        "HierarchicalContextualDataPresetRecord",
    ),
    fixture(
        LootDefinitionKind::GlobalParams,
        "/definitions/lootGenerationGlobalParams",
        "LootGenerationGlobalParams",
    ),
    fixture(
        LootDefinitionKind::SubHarvestableConfig,
        "/definitions/subHarvestableConfigs",
        "SubHarvestableConfigRecord",
    ),
    fixture(
        LootDefinitionKind::SubHarvestableMultiConfig,
        "/definitions/subHarvestableMultiConfigs",
        "SubHarvestableMultiConfigRecord",
    ),
];

const fn fixture(
    family: LootDefinitionKind,
    path: &'static str,
    source_type: &'static str,
) -> Fixture {
    Fixture {
        family,
        path,
        source_type,
        record_path: "synthetic/loot/test/fixture.xml",
    }
}

pub(super) fn datasets() -> impl Iterator<Item = &'static str> {
    FIXTURES
        .iter()
        .map(|fixture| fixture.path)
        .chain([TABLES, ARCHETYPES])
}

pub(super) fn read() -> TestResult<Vec<Route>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let data = (1_u128..).zip(FIXTURES.iter()).fold(
        DataForgeBuilder::new().version(8),
        |database, (guid, row)| {
            database
                .structure(TestStruct::new(row.source_type))
                .record(TestRecord::new(
                    guid,
                    row.source_type,
                    format!("{}.Fixture", row.source_type),
                    row.record_path,
                ))
        },
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", data.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let mut routes = Vec::new();
    for row in FIXTURES {
        parsed.with_loot_definitions(
            row.family,
            CatalogRecordSelection::All,
            None,
            |snapshot| {
                routes.push(Route::queried_rows(row.path, &snapshot.rows, REMAINING)?);
                match row.family {
                    LootDefinitionKind::Table => {
                        routes.push(Route::queried_rows(TABLES, &snapshot.tables, REMAINING)?);
                    }
                    LootDefinitionKind::Archetype => routes.push(Route::queried_rows(
                        ARCHETYPES,
                        &snapshot.archetypes,
                        REMAINING,
                    )?),
                    _ => {}
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(routes)
}
