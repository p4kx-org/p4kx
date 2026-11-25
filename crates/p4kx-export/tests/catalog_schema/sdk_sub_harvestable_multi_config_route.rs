// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::loot::{LootDefinitionKind, LootDefinitionSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_export::{CatalogOptions, CraftingWarning, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourceInventory};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

use super::{Route, TestResult};

#[path = "sdk_harvest_projection_origins.rs"]
mod origins;

pub(super) const DATASET: &str = "/subHarvestableMultiConfigs";
const FAMILY: LootDefinitionKind = LootDefinitionKind::SubHarvestableMultiConfig;
const MULTI: &str = "SubHarvestableMultiConfigRecord";
const CONFIG: &str = "SubHarvestableMultiConfig";
const PRESET: &str = "HarvestablePreset";
const TAGGED: &str = "TaggedSubConfig";
const SINGLE: &str = "SubHarvestableConfigSingleManual";
const MANUAL: &str = "SubHarvestableManual";
const ENTRY: &str = "SubHarvestableEntry";
const SLOT_PATH: &str = "libs/foundry/records/harvestable/slotpresets";
const PRESET_GUID: u128 = 50;
const LATER: u32 = 0;
const OFF_PATH: u32 = 1;
const EARLIER: u32 = 2;
const DEV: u32 = 3;
const DEPENDENCY: u32 = 4;

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = open(false)?;
    let entry = DataForgeEntry::read(&archive)?;
    let producer = build_catalog_from(
        &archive,
        &entry,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(producer.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let parsed = session.parsed(None)?;
    let route =
        parsed.with_loot_definitions(FAMILY, CatalogRecordSelection::All, None, |snapshot| {
            assert_producer_parity(&snapshot, &catalog, producer.summary().source_inventory())
        })??;
    assert_physical_selection(&parsed)?;
    assert_failed_dependency()?;
    origins::assert_schema()?;
    Ok(route)
}

fn assert_producer_parity(
    snapshot: &LootDefinitionSnapshot<'_>,
    catalog: &Value,
    sources: &SourceInventory,
) -> TestResult<Route> {
    let rows = &snapshot.sub_harvestable_multi_configs;
    let projected = serde_json::to_value(rows)?;
    assert_eq!(
        Some(&projected),
        catalog.pointer(DATASET),
        "the scoped query returns the complete normal Catalog projection"
    );
    let [earlier, later] = rows.as_slice() else {
        return Err("only two admitted slot presets".into());
    };
    assert_eq!(
        (earlier.class_name, later.class_name),
        ("Earlier", "Later"),
        "the slot presets follow GUID order despite reverse physical order"
    );
    assert_eq!(
        snapshot.rows.len(),
        4,
        "all four physical owner definitions"
    );
    assert!(
        snapshot.harvest_dependency_warning.is_none(),
        "all dependencies of the admitted owners are readable"
    );
    assert_preset(snapshot, Some("Selected"))?;
    origins::assert_local_origins(snapshot)?;
    origins::assert_dependency_origins(snapshot)?;
    origins::assert_normal_source_closure(snapshot, sources)?;
    Route::queried_rows(
        DATASET,
        rows,
        "synthetic physical selection and field origins witnessed, with installed parity on the final candidate open",
    )
}

fn assert_physical_selection(parsed: &ParsedCatalogQueries<'_>) -> TestResult {
    for (index, expected) in [
        (LATER, Some("Later")),
        (EARLIER, Some("Earlier")),
        (OFF_PATH, None),
        (DEV, None),
    ] {
        parsed.with_loot_definitions(
            FAMILY,
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| assert_record(&snapshot, index, expected),
        )??;
    }
    Ok(())
}

fn assert_record(
    snapshot: &LootDefinitionSnapshot<'_>,
    index: u32,
    expected: Option<&str>,
) -> TestResult {
    let [source] = snapshot.rows.as_slice() else {
        return Err("one selected physical multi-config definition".into());
    };
    assert_eq!(
        source.source().record_index().value(),
        Some(&index),
        "the raw definition belongs to the requested physical Record"
    );
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "the definition family is available for every selected owner"
    );
    assert_eq!(
        snapshot.sub_harvestable_multi_configs.len(),
        usize::from(expected.is_some()),
        "only an admitted slot preset produces a convenience row at Record {index}"
    );
    assert_eq!(
        snapshot
            .sub_harvestable_multi_configs
            .first()
            .map(|row| row.class_name),
        expected,
        "the slot-preset row belongs to the same physical owner at Record {index}"
    );
    assert!(
        snapshot.harvest_dependency_warning.is_none(),
        "the selected scope at Record {index} has no failed dependency read"
    );
    Ok(())
}

fn assert_failed_dependency() -> TestResult {
    let (_folder, archive) = open(true)?;
    let entry = DataForgeEntry::read(&archive)?;
    {
        let forge = entry.open()?;
        let preset = forge.record(DEPENDENCY).ok_or("physical dependency")?;
        assert!(
            preset.record_path().is_ok(),
            "the selected dependency still has a readable physical header"
        );
        assert!(
            preset.tree().is_err(),
            "the selected dependency has a physically unreadable body"
        );
    }
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let parsed = session.parsed(None)?;
    for index in [LATER, EARLIER] {
        parsed.with_loot_definitions(
            FAMILY,
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                origins::assert_preset_failure(&snapshot, index == LATER)?;
                assert_unreadable_preset(snapshot)
            },
        )??;
    }
    parsed.with_loot_definitions(
        FAMILY,
        CatalogRecordSelection::RecordIndex(OFF_PATH),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows.len(),
                1,
                "the off-path definition remains readable"
            );
            assert!(
                snapshot.sub_harvestable_multi_configs.is_empty(),
                "the off-path owner emits no convenience row"
            );
            assert!(
                snapshot.harvest_dependency_warning.is_none(),
                "the off-path owner does not cause a convenience dependency read"
            );
        },
    )?;
    Ok(())
}

fn assert_unreadable_preset(snapshot: LootDefinitionSnapshot<'_>) -> TestResult {
    assert_preset(&snapshot, None)?;
    let Some(CraftingWarning::UnreadableHarvestables { count: 1, details }) =
        snapshot.harvest_dependency_warning
    else {
        return Err("one failed selected harvestable dependency".into());
    };
    let [detail] = details.as_slice() else {
        return Err("the warning must retain one failed dependency".into());
    };
    assert!(
        detail.starts_with("HarvestablePreset "),
        "the detail identifies the failed dependency as a preset"
    );
    assert!(
        detail.ends_with("cannot be read"),
        "the detail records a failed body read"
    );
    assert!(
        snapshot.coverage.unreadable_bodies.is_empty(),
        "the multi-config owner was readable independently of its preset"
    );
    Ok(())
}

fn assert_preset(snapshot: &LootDefinitionSnapshot<'_>, expected: Option<&str>) -> TestResult {
    let config = snapshot
        .sub_harvestable_multi_configs
        .first()
        .ok_or("selected slot preset")?;
    let [tagged] = config.tagged_configs.as_slice() else {
        return Err("one tagged authored config".into());
    };
    let [slot] = tagged.sub_harvestables.as_slice() else {
        return Err("one authored slot".into());
    };
    assert_eq!(
        tagged.name,
        Some("Fixture"),
        "the authored tag-group name is preserved"
    );
    assert_eq!(
        slot.relative_probability.map(f32::to_bits),
        Some(2.5_f32.to_bits()),
        "the slot retains its original Single probability"
    );
    let harvestable = slot
        .harvestable
        .as_ref()
        .ok_or("original preset reference")?;
    assert!(
        harvestable.preset_guid.is_some(),
        "the original preset GUID remains present when the referenced body is unreadable"
    );
    assert_eq!(
        harvestable.preset_class_name, expected,
        "the preset class name requires a successfully read dependency"
    );
    Ok(())
}

fn open(unreadable_preset: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let preset = TestRecord::new(
        PRESET_GUID,
        PRESET,
        "HarvestablePreset.Selected",
        "synthetic/preset.xml",
    )
    .value(
        "entityClass",
        TestValue::Reference(origins::FALLBACK_ENTITY),
    );
    let preset = if unreadable_preset {
        preset.with_instance_index(u16::MAX)
    } else {
        preset
    };
    let class = |kind: &str| TestType::Class(kind.to_owned());
    let data = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(MULTI).field("multiConfig", class(CONFIG)))
        .structure(TestStruct::new(CONFIG).array("taggedConfigs", class(TAGGED)))
        .structure(
            TestStruct::new(TAGGED)
                .field("name", TestType::String)
                .field("subConfig", class(SINGLE)),
        )
        .structure(TestStruct::new(SINGLE).field("subConfigManual", class(MANUAL)))
        .structure(
            TestStruct::new(MANUAL)
                .array("subHarvestables", TestType::StrongPointer(ENTRY.to_owned())),
        )
        .structure(
            TestStruct::new(ENTRY)
                .field("harvestable", TestType::Reference)
                .field("relativeProbability", TestType::Single)
                .field("harvestableSetup", TestType::Reference)
                .field("harvestableEntityClass", TestType::Reference),
        )
        .structure(
            TestStruct::new(PRESET)
                .field("fixture", TestType::Bool)
                .field("entityClass", TestType::Reference),
        )
        .record(record(3, "Later", &format!("{SLOT_PATH}/later.xml")))
        .record(record(1, "Outside", "synthetic/harvest/outside.xml"))
        .record(record(2, "Earlier", &format!("{SLOT_PATH}/earlier.xml")))
        .record(record(4, "Dev", &format!("{SLOT_PATH}/dev/leftover.xml")))
        .record(preset);
    let data = origins::add_dependencies(data).build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", data))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn record(guid: u128, name: &str, path: &str) -> TestRecord {
    let slot = TestValue::Struct(
        TestInstance::new(ENTRY)
            .value("harvestable", TestValue::Reference(PRESET_GUID))
            .value("relativeProbability", TestValue::Single(2.5))
            .value("harvestableSetup", TestValue::Null)
            .value(
                "harvestableEntityClass",
                if guid == 3 {
                    TestValue::Reference(origins::DIRECT_ENTITY)
                } else {
                    TestValue::Null
                },
            ),
    );
    let manual = TestInstance::new(MANUAL).value(
        "subHarvestables",
        TestValue::Array(vec![TestValue::Null, slot]),
    );
    let config = TestInstance::new(SINGLE).value("subConfigManual", TestValue::Struct(manual));
    let tagged = TestValue::Struct(
        TestInstance::new(TAGGED)
            .value("name", TestValue::text("Fixture"))
            .value("subConfig", TestValue::Struct(config)),
    );
    TestRecord::new(guid, MULTI, format!("{MULTI}.{name}"), path).value(
        "multiConfig",
        TestValue::Struct(
            TestInstance::new(CONFIG).value("taggedConfigs", TestValue::Array(vec![tagged])),
        ),
    )
}
