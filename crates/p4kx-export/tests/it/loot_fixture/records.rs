// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Records of the loot fixture.

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestValue};

use super::{array, instance, legacy, record};

const LOOT: &str = "libs/foundry/records/lootgeneration/";
const SLOTS: &str = "libs/foundry/records/harvestable/slotpresets/";

/// The GUIDs of the Records.
pub(super) struct Ids {
    pub(super) tag_rare: u128,
    pub(super) tag_common: u128,
    pub(super) entity: u128,
    pub(super) archetype: u128,
    pub(super) table: u128,
    pub(super) table_debug: u128,
    pub(super) legacy_table: u128,
    pub(super) legacy_archetype: u128,
    pub(super) filter_leaf: u128,
    pub(super) filter: u128,
    pub(super) config: u128,
    pub(super) planet: u128,
}

fn term(positive: &[u128], negative: &[u128]) -> TestValue {
    let references = |guids: &[u128]| {
        TestValue::Array(guids.iter().copied().map(TestValue::Reference).collect())
    };
    TestValue::Struct(
        instance("TagsDNFTerm")
            .value("positiveTags", references(positive))
            .value("negativeTags", references(negative)),
    )
}

fn linear(min: f32, max: f32) -> TestValue {
    TestValue::Struct(
        instance("QuantityRange_Linear")
            .value("min", TestValue::Single(min))
            .value("max", TestValue::Single(max)),
    )
}

fn tag_selector(ids: &Ids) -> TestValue {
    TestValue::Struct(instance("LootArchetypeV3Selector_Tags").value(
        "tags",
        term(&[ids.tag_common, ids.tag_rare], &[ids.tag_rare]),
    ))
}

fn ammo_entry(ids: &Ids) -> p4kx_testkit::TestInstance {
    let stack = instance("ArchetypeOptionalDataV3_StackSize").value("stackSize", linear(1.0, 3.0));
    let spawn = instance("ArchetypeOptionalDataV3_SpawnWith")
        .value("name", TestValue::text("Suitable ammo"))
        .value("chanceToSpawnWith", TestValue::Single(1.0))
        .value("mode", TestValue::text("MostSimilar"))
        .value("amountToSpawn", linear(2.0, 4.0))
        .value(
            "selector",
            TestValue::Struct(
                instance("SpawnWithV3Selector_Tags").value("tags", term(&[ids.tag_common], &[])),
            ),
        );
    instance("LootArchetypeV3Entry")
        .value("name", TestValue::text("Ammo"))
        .value("weight", TestValue::Single(3.0))
        .value("selector", tag_selector(ids))
        .value("optionalData", array(vec![stack, spawn]))
}

fn unique_entry(ids: &Ids) -> p4kx_testkit::TestInstance {
    let classes = instance("EntityClassList_Manual").value(
        "entityClasses",
        TestValue::Array(vec![TestValue::Reference(ids.entity)]),
    );
    instance("LootArchetypeV3Entry")
        .value("name", TestValue::text("Unique"))
        .value("weight", TestValue::Single(1.0))
        .value(
            "selector",
            TestValue::Struct(
                instance("LootArchetypeV3Selector_EntityClasses")
                    .value("entityClasses", TestValue::Struct(classes)),
            ),
        )
}

fn archetype_record(ids: &Ids) -> TestRecord {
    record(
        ids.archetype,
        "LootArchetypeV3Record",
        "V3LootArchetype_Cargo",
        &format!("{LOOT}lootarchetypes/v3lootarchetype_cargo"),
    )
    .value(
        "lootArchetype",
        TestValue::Struct(
            instance("LootArchetypeV3").value("entries", array(vec![unique_entry(ids)])),
        ),
    )
}

fn inline_table_entry(ids: &Ids) -> p4kx_testkit::TestInstance {
    let optional = |data: p4kx_testkit::TestInstance| TestValue::Struct(data);
    instance("LootTableV3Entry")
        .value("name", TestValue::text("Ammo"))
        .value("weight", TestValue::Single(10.0))
        .value(
            "archetype",
            TestValue::Struct(
                instance("LootArchetypeV3").value("entries", array(vec![ammo_entry(ids)])),
            ),
        )
        .value(
            "optionalData",
            TestValue::Array(vec![
                optional(
                    instance("LootTableOptionalDataV3_PriorityChoice")
                        .value("priorityCount", TestValue::Int32(1)),
                ),
                optional(
                    instance("LootTableOptionalDataV3_ChoiceLimit")
                        .value("choiceLimit", TestValue::Int32(2)),
                ),
                optional(
                    instance("LootTableOptionalDataV3_DupeLimit")
                        .value("dupeLimit", TestValue::Int32(3)),
                ),
                TestValue::Null,
                optional(
                    instance("LootTableOptionalDataV3_FixtureExtension")
                        .value("unknownField", TestValue::UInt16(12)),
                ),
                optional(
                    instance("LootTableOptionalDataV3_DupeLimit")
                        .value("dupeLimit", TestValue::Int32(7)),
                ),
                optional(
                    instance("LootTableOptionalDataV3_ChanceToExist")
                        .value("chanceToExist", TestValue::Single(0.75)),
                ),
            ]),
        )
}

pub(super) fn table_record(guid: u128, name: &str, ids: &Ids) -> TestRecord {
    let linked = instance("LootTableV3Entry")
        .value("name", TestValue::text("Linked"))
        .value("weight", TestValue::Single(1.0))
        .value(
            "archetype",
            TestValue::Struct(
                instance("LootArchetypeV3_RecordRef")
                    .value("lootArchetypeRecord", TestValue::Reference(ids.archetype)),
            ),
        );
    record(
        guid,
        "LootTableV3Record",
        name,
        &format!("{LOOT}loottables/{}", name.to_lowercase()),
    )
    .value(
        "lootTable",
        TestValue::Struct(instance("LootTableV3").value(
            "lootArchetypes",
            array(vec![inline_table_entry(ids), linked]),
        )),
    )
}

fn filter_records(ids: &Ids) -> [TestRecord; 2] {
    let leaf = record(
        ids.filter_leaf,
        "PoolFilterRecord",
        "V3PoolFilter_Leaf",
        &format!("{LOOT}filters/v3poolfilter_leaf"),
    )
    .value(
        "filter",
        TestValue::Struct(
            instance("PoolFilter_Tags").value("tagFilter", term(&[ids.tag_common], &[])),
        ),
    );
    let step = |name: &str, mode: &str, filter: Option<TestValue>| {
        let step = instance("PoolFilterInstance")
            .value("name", TestValue::text(name))
            .value("mode", TestValue::text(mode));
        match filter {
            Some(filter) => step.value("filter", filter),
            None => step,
        }
    };
    let sequence = instance("PoolFilter_Sequence").value(
        "filters",
        array(vec![
            step(
                "Base",
                "Cumulative",
                Some(TestValue::Struct(instance("PoolFilter_RecordRef").value(
                    "filterRecord",
                    TestValue::Reference(ids.filter_leaf),
                ))),
            ),
            step(
                "Event",
                "Additive",
                Some(TestValue::Struct(
                    instance("PoolFilter_Tags").value("tagFilter", term(&[], &[ids.tag_rare])),
                )),
            ),
            step("Empty", "Additive", None),
        ]),
    );
    let whole = record(
        ids.filter,
        "PoolFilterRecord",
        "V3PoolFilter_Cargo",
        &format!("{LOOT}filters/v3poolfilter_cargo"),
    )
    .value("filter", TestValue::Struct(sequence));
    [leaf, whole]
}

fn config_record(ids: &Ids) -> TestRecord {
    let constraints = instance("LootConstraints")
        .value("totalResultsLimit", TestValue::Int32(4))
        .value("chanceToGenerate", TestValue::Single(1.0))
        .value(
            "chanceToGenerateAdditionalAttachedInventories",
            TestValue::Single(0.5),
        )
        .value("poolFilter", TestValue::Reference(ids.filter))
        .value(
            "fullnessFactorRange",
            TestValue::Struct(
                instance("FloatFactorRange")
                    .value("min", TestValue::Single(0.2))
                    .value("max", TestValue::Single(0.8)),
            ),
        );
    let slot = |loot: p4kx_testkit::TestInstance| {
        instance("SubHarvestableEntry").value("lootConfig", TestValue::Struct(loot))
    };
    let loot = instance("LootConfig")
        .value("lootConstraints", TestValue::Struct(constraints))
        .value("lootTable", TestValue::Reference(ids.legacy_table))
        .value("lootTableV3", TestValue::Reference(ids.table));
    let tagged = instance("TaggedSubConfig")
        .value("name", TestValue::text("Rare"))
        .value(
            "tagList",
            TestValue::Struct(instance("HarvestableTagList").value(
                "tags",
                TestValue::Array(vec![TestValue::Reference(ids.tag_rare)]),
            )),
        )
        .value(
            "subConfig",
            TestValue::Struct(
                instance("SubHarvestableConfigSingleManual").value(
                    "subConfigManual",
                    TestValue::Struct(
                        instance("SubHarvestableManual")
                            .value("subHarvestables", array(vec![slot(loot)])),
                    ),
                ),
            ),
        );
    record(
        ids.config,
        "SubHarvestableMultiConfigRecord",
        "LootGeneration_MultiSlotPreset_Cargo",
        &format!("{SLOTS}lootgeneration_multislotpreset_cargo"),
    )
    .value(
        "multiConfig",
        TestValue::Struct(
            instance("SubHarvestableMultiConfig").value("taggedConfigs", array(vec![tagged])),
        ),
    )
}

/// Adds the Records of the fixture to `builder`.
pub(super) fn records(builder: DataForgeBuilder, ids: &Ids) -> DataForgeBuilder {
    let tag = |guid: u128, name: &str| {
        record(
            guid,
            "Tag",
            name,
            &format!("libs/foundry/records/tagdatabase/{}", name.to_lowercase()),
        )
    };
    let [leaf, whole] = filter_records(ids);
    let [legacy_table, legacy_archetype] = legacy::records(&legacy::Ids {
        table: ids.legacy_table,
        archetype: ids.legacy_archetype,
        tag_rare: ids.tag_rare,
        tag_common: ids.tag_common,
    });
    builder
        .record(tag(ids.tag_rare, "Rare"))
        .record(tag(ids.tag_common, "Common"))
        .record(tag(ids.entity, "Stub"))
        .record(archetype_record(ids))
        .record(table_record(ids.table, "V3LootTable_Cargo", ids))
        .record(table_record(ids.table_debug, "V3LootTable_Debug", ids))
        .record(legacy_table)
        .record(legacy_archetype)
        .record(leaf)
        .record(whole)
        .record(config_record(ids))
        .record(
            record(
                ids.planet,
                "StarMapObject",
                "Stanton1",
                "libs/foundry/records/starmap/pu/system/stanton/stanton1",
            )
            .value("name", TestValue::text("@starmap_Stanton1")),
        )
}
