// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::loot::LootDefinitionKind;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

use super::{Route, TestResult};

#[path = "sdk_sub_harvestable_multi_config_route.rs"]
mod multi_configs;

const TABLES: &str = "/lootTables";
const ARCHETYPES: &str = "/lootArchetypes";
const LEGACY_TABLES: &str = "/legacyLootTables";
const LEGACY_ARCHETYPES: &str = "/legacyLootArchetypes";
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

const fn definition_remaining(family: LootDefinitionKind) -> Option<&'static str> {
    match family {
        LootDefinitionKind::Table
        | LootDefinitionKind::Archetype
        | LootDefinitionKind::PoolFilter
        | LootDefinitionKind::SecondaryChoiceSingleLayer
        | LootDefinitionKind::SecondaryChoiceMultiLayer
        | LootDefinitionKind::Theme
        | LootDefinitionKind::GlobalParams => None,
        _ => Some(REMAINING),
    }
}

pub(super) fn datasets() -> impl Iterator<Item = &'static str> {
    FIXTURES.iter().map(|fixture| fixture.path).chain([
        TABLES,
        ARCHETYPES,
        LEGACY_TABLES,
        LEGACY_ARCHETYPES,
        multi_configs::DATASET,
    ])
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
    let data = add_legacy_rows(data);
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
                let remaining = definition_remaining(row.family);
                routes.push(Route::queried_rows(row.path, &snapshot.rows, remaining)?);
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
    let selection = CatalogRecordSelection::All;
    routes.push(parsed.with_legacy_loot_tables(selection, None, |snapshot| {
        Route::queried_rows(LEGACY_TABLES, &snapshot.rows, REMAINING)
    })??);
    routes.push(
        parsed.with_legacy_loot_archetypes(selection, None, |snapshot| {
            Route::queried_rows(LEGACY_ARCHETYPES, &snapshot.rows, REMAINING)
        })??,
    );
    routes.push(multi_configs::read()?);
    Ok(routes)
}

fn add_legacy_rows(data: DataForgeBuilder) -> DataForgeBuilder {
    data.structure(TestStruct::new("LootTable"))
        .structure(TestStruct::new("LootArchetype"))
        .record(TestRecord::new(
            100,
            "LootTable",
            "LootTable.Legacy",
            "libs/foundry/records/lootgeneration/loottables/legacy.xml",
        ))
        .record(TestRecord::new(
            101,
            "LootArchetype",
            "LootArchetype.Legacy",
            "libs/foundry/records/lootgeneration/lootarchetypes/legacy.xml",
        ))
}
