// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const LIST: u128 = 0x0010_7301;
pub(super) const EMPTY: u128 = 0x0010_7302;
pub(super) const SCOPE: u128 = 0x0010_7303;
pub(super) const STANDING: u128 = 0x0010_7304;
pub(super) const ITEM: u128 = 0x0010_7305;
pub(super) const FRANCHISE: u128 = 0x0010_7306;
pub(super) const MISSING: u128 = 0x0010_7307;
pub(super) const CATEGORY_ID: u128 = 0x0010_7308;
pub(super) const LITERAL: &str = "@fixture_literal_description";
pub(super) const ICON: &str = "textures/fixture/perk.dds";
pub(super) const DISCOUNT_BITS: u32 = 0x4148_0000;
pub(super) const RAW_BITS: u32 = 0x7fc0_1073;
pub(super) const AUTHORED: &str = "FixtureNewPerk";

const LIST_TYPE: &str = "SPerkReputationListParams";
const ENTRY: &str = "SPerkReputationParams";
const BASE: &str = "SPerkParamsBase";
const ITEM_TYPE: &str = "SPerkParamsItem";
const CATEGORY: &str = "SPerkShopItemCategoryBase";
const ENABLED: &str = "SPerkEnabledItemCategory";
const DISCOUNT: &str = "SPerkDiscountItemCategory";
const SCOPE_TYPE: &str = "SReputationScopeParams";
const STANDING_TYPE: &str = "SReputationStandingParams";

pub(super) fn complete() -> DataForgeBuilder {
    declarations()
        .record(TestRecord::new(
            EMPTY,
            LIST_TYPE,
            "SPerkReputationListParams.Empty",
            "libs/foundry/records/synthetic/template_empty_perks.xml",
        ))
        .record(
            TestRecord::new(
                LIST,
                LIST_TYPE,
                "SPerkReputationListParams.Shared",
                "libs/foundry/records/reputation/perks/fixture_shared.xml",
            )
            .value("perks", TestValue::Array(entries())),
        )
        .record(identity(SCOPE, SCOPE_TYPE, "reputation/scopes/fixture"))
        .record(identity(
            STANDING,
            STANDING_TYPE,
            "reputation/standings/fixture",
        ))
        .record(identity(
            ITEM,
            "EntityClassDefinition",
            "synthetic/fixture_reward",
        ))
        .record(identity(
            FRANCHISE,
            "ShopFranchise",
            "synthetic/fixture_franchise",
        ))
        .record(faction(0x0010_7309, "first"))
        .record(faction(0x0010_730a, "second"))
}

fn declarations() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("EItemType", ["FixtureCategory"])
        .enumeration("EItemSubType", ["FixtureUnknownSubtype"])
        .structure(TestStruct::new(LIST_TYPE).array("perks", TestType::Class(ENTRY.to_owned())))
        .structure(
            TestStruct::new(ENTRY)
                .field("scope", reference(SCOPE_TYPE))
                .field("standing", reference(STANDING_TYPE))
                .field("perk", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(base())
        .structure(
            TestStruct::new(ITEM_TYPE)
                .parent(BASE)
                .field("rewardItem", reference("EntityClassDefinition")),
        )
        .structure(category())
        .structure(TestStruct::new(ENABLED).parent(CATEGORY))
        .structure(
            TestStruct::new(DISCOUNT)
                .parent(CATEGORY)
                .field("discountPercentage", TestType::Single),
        )
        .structure(
            TestStruct::new(AUTHORED)
                .parent(BASE)
                .field("fixtureOriginalBits", TestType::Single),
        )
        .structure(TestStruct::new(SCOPE_TYPE))
        .structure(TestStruct::new(STANDING_TYPE))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("ShopFranchise"))
        .structure(TestStruct::new("Tag"))
        .structure(
            TestStruct::new("FactionReputation")
                .array("perkReputationRewardList", reference(LIST_TYPE)),
        )
}

fn base() -> TestStruct {
    TestStruct::new(BASE)
        .field("name", TestType::String)
        .field("description", TestType::String)
        .field("displayName", TestType::Locale)
        .field("displayDescription", TestType::Locale)
        .field("icon", TestType::String)
}

fn category() -> TestStruct {
    TestStruct::new(CATEGORY)
        .parent(BASE)
        .field("id", TestType::Guid)
        .field("franchise", reference("ShopFranchise"))
        .field("type", TestType::Enum("EItemType".to_owned()))
        .field("subType", TestType::Enum("EItemSubType".to_owned()))
        .field("requiredTag", reference("Tag"))
}

fn entries() -> Vec<TestValue> {
    vec![
        entry(
            STANDING,
            TestValue::Struct(perk(ITEM_TYPE).value("rewardItem", TestValue::Reference(ITEM))),
        ),
        entry(
            STANDING,
            TestValue::Struct(category_perk(DISCOUNT).value(
                "discountPercentage",
                TestValue::Single(f32::from_bits(DISCOUNT_BITS)),
            )),
        ),
        entry(MISSING, TestValue::Struct(category_perk(ENABLED))),
        entry(
            STANDING,
            TestValue::Struct(perk(AUTHORED).value(
                "fixtureOriginalBits",
                TestValue::Single(f32::from_bits(RAW_BITS)),
            )),
        ),
        entry(STANDING, TestValue::Null),
    ]
}

fn entry(standing: u128, perk: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(ENTRY)
            .value("scope", TestValue::Reference(SCOPE))
            .value("standing", TestValue::Reference(standing))
            .value("perk", perk),
    )
}

fn perk(source_type: &str) -> TestInstance {
    TestInstance::new(source_type)
        .value("name", TestValue::text("fixture_internal_perk"))
        .value("description", TestValue::text(LITERAL))
        .value("displayName", TestValue::text("@shared"))
        .value("displayDescription", TestValue::text("@own_description"))
        .value("icon", TestValue::text(ICON))
}

fn category_perk(source_type: &str) -> TestInstance {
    perk(source_type)
        .value("id", TestValue::Guid(CATEGORY_ID))
        .value("franchise", TestValue::Reference(FRANCHISE))
        .value("type", TestValue::text("FixtureCategory"))
        .value("subType", TestValue::text("FixtureUnknownSubtype"))
        .value("requiredTag", TestValue::Reference(MISSING))
}

fn identity(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Fixture"),
        format!("libs/foundry/records/{path}.xml"),
    )
}

fn faction(guid: u128, name: &str) -> TestRecord {
    identity(
        guid,
        "FactionReputation",
        &format!("factions/factionreputation/fixture_{name}"),
    )
    .value(
        "perkReputationRewardList",
        TestValue::Array(vec![TestValue::Reference(LIST)]),
    )
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}
