// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};

use super::{
    BAD_POOL, BARE, BLACKLISTED, CHOOSE_ONE, CONTRACT_ID, CONTRACTS, CRAFTING, DEFAULT_CURVE,
    DISMANTLE, EMPTY_POOL, GLOBAL, GOLD, LEFTOVER_POOL, NAMELESS_CONTRACT_ID, OVERRIDE_CURVE, POOL,
    PROPERTY, PYRO, TWO_TIERS, time,
};

pub(super) fn rewards(builder: DataForgeBuilder) -> DataForgeBuilder {
    contracts(pools(builder))
}

fn pools(builder: DataForgeBuilder) -> DataForgeBuilder {
    let entry = |guid: u128, weight: f32| {
        TestValue::Struct(
            TestInstance::new("BlueprintReward")
                .value("weight", TestValue::Single(weight))
                .value("blueprintRecord", TestValue::Reference(guid)),
        )
    };
    builder
        .record(
            TestRecord::new(
                POOL,
                "BlueprintPoolRecord",
                "BlueprintPoolRecord.BP_REWARDS_Haul",
                format!("{CRAFTING}/blueprintrewards/bp_rewards_haul.xml"),
            )
            .value(
                "blueprintRewards",
                TestValue::Array(vec![entry(TWO_TIERS, 1.0), entry(CHOOSE_ONE, 3.0)]),
            ),
        )
        .record(
            TestRecord::new(
                BAD_POOL,
                "BlueprintPoolRecord",
                "BlueprintPoolRecord.BP_REWARDS_Bad",
                format!("{CRAFTING}/blueprintrewards/bp_rewards_bad.xml"),
            )
            .value(
                "blueprintRewards",
                TestValue::Array(vec![entry(BARE, -2.0), entry(TWO_TIERS, 2.0)]),
            ),
        )
        .record(TestRecord::new(
            EMPTY_POOL,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.BP_REWARDS_Empty",
            format!("{CRAFTING}/blueprintrewards/bp_rewards_empty.xml"),
        ))
        .record(TestRecord::new(
            LEFTOVER_POOL,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.BP_REWARDS_Dev",
            format!("{CRAFTING}/blueprintrewards/bp_rewards_dev.xml"),
        ))
}

fn contracts(builder: DataForgeBuilder) -> DataForgeBuilder {
    let reward = |chance: f32, pool: u128| {
        TestValue::Struct(
            TestInstance::new("BlueprintRewards")
                .value("chance", TestValue::Single(chance))
                .value("blueprintPool", TestValue::Reference(pool)),
        )
    };
    let contract = |id: u128, name: &str, results: Vec<TestValue>| {
        TestValue::Struct(
            TestInstance::new("ContractLegacy")
                .value("id", TestValue::Guid(id))
                .value("debugName", TestValue::text(name))
                .value("results", TestValue::Array(results)),
        )
    };
    builder.record(
        TestRecord::new(
            CONTRACTS,
            "ContractGenerator",
            "ContractGenerator.Haul",
            "libs/foundry/records/contracts/contractgenerator/haul.xml",
        )
        .value(
            "generators",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("ContractGeneratorHandler_Legacy")
                    .value("debugName", TestValue::text("Haul_Career"))
                    .value(
                        "legacyContracts",
                        TestValue::Array(vec![
                            contract(
                                CONTRACT_ID,
                                "Haul_Intro",
                                vec![reward(0.5, POOL), reward(1.0, EMPTY_POOL)],
                            ),
                            contract(
                                0,
                                "Haul_Odd",
                                vec![reward(1.5, BAD_POOL), reward(0.5, BAD_POOL)],
                            ),
                            contract(NAMELESS_CONTRACT_ID, "", vec![reward(0.25, EMPTY_POOL)]),
                        ]),
                    ),
            )]),
        ),
    )
}

fn normal(min: i32, mean: f32, stddev: f64) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingQualityDistributionNormal")
            .value("min", TestValue::Int32(min))
            .value("max", TestValue::Int32(1000))
            .value("mean", TestValue::Single(mean))
            .value("stddev", TestValue::Double(stddev)),
    )
}

pub(super) fn quality(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(
            TestRecord::new(
                DEFAULT_CURVE,
                "CraftingQualityDistributionRecord",
                "CraftingQualityDistributionRecord.Creature_Default",
                format!("{CRAFTING}/qualitydistribution/creatures/creature_default.xml"),
            )
            .value("qualityDistribution", normal(1, 269.0, 254.0)),
        )
        .record(
            TestRecord::new(
                OVERRIDE_CURVE,
                "CraftingQualityLocationOverrideRecord",
                "CraftingQualityLocationOverrideRecord.Creature_Pyro",
                format!("{CRAFTING}/qualitydistribution/creatures/creature_pyro.xml"),
            )
            .value(
                "locationOverride",
                TestValue::Struct(TestInstance::new("CraftingQualityLocationOverride").value(
                    "locationOverrideList",
                    TestValue::Array(vec![TestValue::Struct(
                            TestInstance::new("CraftingQualityLocationOverrideEntry")
                                .value("location", TestValue::Reference(PYRO))
                                .value("qualityDistribution", normal(201, 295.0, 1.0)),
                        )]),
                )),
            ),
        )
        .record(
            TestRecord::new(
                PROPERTY,
                "CraftingGameplayPropertyDef",
                "CraftingGameplayPropertyDef.GPP_Armor_DamageMitigation",
                format!("{CRAFTING}/craftedproperties/gpp_armor_damagemitigation.xml"),
            )
            .value("propertyName", TestValue::text("@StatName_GPP_Armor")),
        )
}

pub(super) fn dismantle(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingBlueprintRecord",
        format!("CraftingBlueprintRecord.{name}"),
        format!(
            "{CRAFTING}/blueprints/dismantle/{}.xml",
            name.to_lowercase()
        ),
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new("GenericCraftingBlueprint").value(
                "processSpecificData",
                TestValue::Struct(
                    TestInstance::new("GenericCraftingProcess_Dismantle")
                        .value("efficiency", TestValue::Single(0.5))
                        .value("dismantleTime", time(0, 15.0)),
                ),
            ),
        ),
    )
}

pub(super) fn dismantle_and_global(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(dismantle(DISMANTLE, "GlobalGenericDismantle"))
        .record(
            TestRecord::new(
                GLOBAL,
                "CraftingGlobalParams",
                "CraftingGlobalParams.CraftingGlobalParams",
                format!("{CRAFTING}/globalparams/craftingglobalparams.xml"),
            )
            .value(
                "dismantleBlacklistResources",
                TestValue::Array(vec![TestValue::Reference(GOLD)]),
            )
            .value(
                "dismantleBlacklistEntityClasses",
                TestValue::Array(vec![TestValue::Reference(BLACKLISTED)]),
            )
            .value(
                "defaultBlueprintSelection",
                TestValue::Struct(
                    TestInstance::new("DefaultBlueprintSelection_Whitelist").value(
                        "blueprintRecords",
                        TestValue::Array(vec![
                            TestValue::Reference(DISMANTLE),
                            TestValue::Reference(CHOOSE_ONE),
                        ]),
                    ),
                ),
            ),
        )
}
