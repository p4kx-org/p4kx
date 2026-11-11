// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x8000_0000_0000_4000_8000_0000_0000_0011;
pub(super) const RESEARCH: &str = "CraftingResearch";
pub(super) const COSTS: &str = "CraftingRecipeCosts_Base";
pub(super) const UNLOCK: &str = "CraftingResearchUnlock_Base";
pub(super) const UNKNOWN: &str = "CraftingRecipeCosts_SyntheticFuture";
const TIER: &str = "CraftingBlueprintTier";

#[derive(Clone, Copy)]
pub(super) enum Case {
    Absent,
    Null,
    Empty,
    Known,
    Unsupported,
    Unreadable,
}

pub(super) fn database(case: Case) -> DataForgeBuilder {
    let mut tier = TestStruct::new(TIER);
    if !matches!(case, Case::Absent) {
        tier = tier.field("research", pointer(RESEARCH));
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("blueprint", class("CraftingBlueprint")),
        )
        .structure(TestStruct::new("CraftingBlueprint").array("tiers", class(TIER)))
        .structure(tier)
        .structure(
            TestStruct::new(RESEARCH)
                .field("researchCosts", pointer(COSTS))
                .field("unlockRequirements", pointer(UNLOCK)),
        )
        .structure(TestStruct::new(COSTS))
        .structure(TestStruct::new(UNLOCK))
        .structure(
            TestStruct::new(UNKNOWN)
                .parent(COSTS)
                .field("opaqueToken", TestType::UInt64),
        )
        .record(record(case))
}

fn record(case: Case) -> TestRecord {
    let mut tier = TestInstance::new(TIER);
    if !matches!(case, Case::Absent) {
        tier = tier.value("research", research(case));
    }
    TestRecord::new(
        OWNER,
        "CraftingBlueprintRecord",
        "CraftingBlueprintRecord.BP_CRAFT_research",
        "libs/foundry/records/crafting/blueprints/crafting/bp_craft_research.xml",
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new("CraftingBlueprint")
                .value("tiers", TestValue::Array(vec![TestValue::Struct(tier)])),
        ),
    )
}

fn research(case: Case) -> TestValue {
    let costs = match case {
        Case::Absent | Case::Null => return TestValue::Null,
        Case::Empty => TestValue::Null,
        Case::Known => TestValue::Struct(TestInstance::new(COSTS)),
        Case::Unsupported => TestValue::Struct(
            TestInstance::new(UNKNOWN).value("opaqueToken", TestValue::UInt64(u64::MAX)),
        ),
        Case::Unreadable => TestValue::Raw(
            [u32::MAX - 1, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        ),
    };
    TestValue::Struct(
        TestInstance::new(RESEARCH)
            .value("researchCosts", costs)
            .value("unlockRequirements", TestValue::Null),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
