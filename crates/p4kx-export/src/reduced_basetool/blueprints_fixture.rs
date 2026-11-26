// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const BLUEPRINT: u128 = 0x1177_7331;
pub(super) const LOCALE_KEY: &str = "fixture_blueprint_name";

const CATEGORY: u128 = 0x1177_7332;
const PRODUCT: u128 = 0x1177_7333;
const DISMANTLE: u128 = 0x1177_7334;
const MATERIAL: u128 = 0x1177_7335;
const PROPERTY: u128 = 0x1177_7336;
const POOL: u128 = 0x1177_7337;
const ROOT: &str = "libs/foundry/records/crafting";
const POOL_TYPE: &str = "BlueprintPoolRecord";
const REWARD: &str = "FixtureBlueprintReward";
const RECORD: &str = "CraftingBlueprintRecord";
const SOURCE: &str = "FixtureCraftingBlueprint";
const TIER: &str = "FixtureCraftingTier";
const RECIPE: &str = "FixtureRecipe";
const COSTS: &str = "FixtureRecipeCosts";
const COST: &str = "FixtureCost";
const SELECTION: &str = "CraftingCost_Select";
const ITEM: &str = "CraftingCost_Item";
const INFO: &str = "FixtureNameInfo";
const PROCESS: &str = "FixtureProcess";
const CREATION_TYPE: &str = "FixtureCreation";
const DISMANTLE_TYPE: &str = "GenericCraftingProcess_Dismantle";
const RESOURCE: &str = "CraftingCost_Resource";
const SCU: &str = "FixtureStandardCargoUnit";
const DURATION: &str = "TimeValue_Partitioned";
const CONTEXT: &str = "FixtureCostContext";
const MULTIPLIER: &str = "CraftingCostContext_QuantityMultiplier";
const INCLUSION: &str = "CraftingCostContext_ResultCompositionInclusion";
const MODIFIERS: &str = "CraftingCostContext_ResultGameplayPropertyModifiers";
const MODIFIER_LIST: &str = "FixtureModifierList";
const MODIFIER: &str = "FixtureModifier";
const RANGE_BASE: &str = "FixtureRangeBase";
const LINEAR: &str = "CraftingGameplayPropertyModifierValueRange_Linear";
const UNKNOWN_RANGE: &str = "CraftingGameplayPropertyModifierValueRange_Curve";

#[derive(Clone, Copy)]
pub(super) enum RangeType {
    Linear,
    Unsupported,
}

impl RangeType {
    const fn type_name(self) -> &'static str {
        match self {
            Self::Linear => LINEAR,
            Self::Unsupported => UNKNOWN_RANGE,
        }
    }
}

pub(super) fn database(range: RangeType) -> DataForgeBuilder {
    schema()
        .record(blueprint("FixturePrimary", range))
        .record(blueprint("FixtureAlias", range))
        .record(TestRecord::new(
            CATEGORY,
            "BlueprintCategoryRecord",
            "BlueprintCategoryRecord.FixtureCategory",
            format!("{ROOT}/blueprintcategories/fixture.xml"),
        ))
        .record(TestRecord::new(
            PRODUCT,
            "EntityClassDefinition",
            "EntityClassDefinition.FixtureProduct",
            "libs/foundry/records/entities/items/fixture.xml",
        ))
        .record(
            TestRecord::new(
                DISMANTLE,
                RECORD,
                "CraftingBlueprintRecord.FixtureDismantle",
                format!("{ROOT}/blueprints/dismantle/fixture.xml"),
            )
            .value(
                "blueprint",
                TestValue::Struct(TestInstance::new(SOURCE).value(
                    "processSpecificData",
                    TestValue::Struct(TestInstance::new(DISMANTLE_TYPE)),
                )),
            ),
        )
        .record(TestRecord::new(
            MATERIAL,
            "ResourceType",
            "ResourceType.FixtureOre",
            "libs/foundry/records/resourcetypedatabase/fixture.xml",
        ))
        .record(TestRecord::new(
            PROPERTY,
            "CraftingGameplayPropertyDef",
            "CraftingGameplayPropertyDef.FixtureProperty",
            format!("{ROOT}/craftedproperties/fixture.xml"),
        ))
        .record(
            TestRecord::new(
                POOL,
                POOL_TYPE,
                "BlueprintPoolRecord.FixturePool",
                format!("{ROOT}/blueprintrewards/fixture.xml"),
            )
            .value(
                "blueprintRewards",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(REWARD)
                        .value("blueprintRecord", TestValue::Reference(BLUEPRINT))
                        .value("weight", TestValue::Single(2.5)),
                )]),
            ),
        )
}

fn schema() -> DataForgeBuilder {
    recipe_context(declarations())
}

fn declarations() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(RECORD).field("blueprint", pointer(SOURCE)))
        .structure(
            TestStruct::new(SOURCE)
                .field("blueprintName", TestType::Locale)
                .field("category", TestType::Reference)
                .field("processSpecificData", pointer(PROCESS))
                .array("tiers", pointer(TIER)),
        )
        .structure(TestStruct::new(TIER).field("recipe", pointer(RECIPE)))
        .structure(TestStruct::new(RECIPE).field("costs", pointer(COSTS)))
        .structure(
            TestStruct::new(COSTS)
                .field("craftTime", TestType::Class(DURATION.to_owned()))
                .field("mandatoryCost", pointer(COST))
                .array("optionalCosts", pointer(COST)),
        )
        .structure(TestStruct::new(COST).array("context", pointer(CONTEXT)))
        .structure(
            TestStruct::new(DURATION)
                .field("days", TestType::Int32)
                .field("hours", TestType::Int32)
                .field("minutes", TestType::Int32)
                .field("seconds", TestType::Single),
        )
        .structure(
            TestStruct::new(SELECTION)
                .parent(COST)
                .field("nameInfo", pointer(INFO))
                .array("options", pointer(COST))
                .field("count", TestType::Int32),
        )
        .structure(TestStruct::new(INFO).field("debugName", TestType::String))
        .structure(
            TestStruct::new(ITEM)
                .parent(COST)
                .field("entityClass", TestType::Reference)
                .field("quantity", TestType::Int32)
                .field("minQuality", TestType::Int32),
        )
        .structure(TestStruct::new(PROCESS))
        .structure(
            TestStruct::new(CREATION_TYPE)
                .parent(PROCESS)
                .field("entityClass", TestType::Reference),
        )
        .structure(TestStruct::new(DISMANTLE_TYPE).parent(PROCESS))
        .structure(TestStruct::new("BlueprintCategoryRecord"))
        .structure(TestStruct::new(POOL_TYPE).array("blueprintRewards", pointer(REWARD)))
        .structure(
            TestStruct::new(REWARD)
                .field("blueprintRecord", TestType::Reference)
                .field("weight", TestType::Single),
        )
        .structure(TestStruct::new("EntityClassDefinition"))
}

fn blueprint(name: &str, range: RangeType) -> TestRecord {
    let process = TestValue::Struct(
        TestInstance::new(CREATION_TYPE).value("entityClass", TestValue::Reference(PRODUCT)),
    );
    let source = TestInstance::new(SOURCE)
        .value("blueprintName", TestValue::text(format!("@{LOCALE_KEY}")))
        .value("category", TestValue::Reference(CATEGORY))
        .value("processSpecificData", process)
        .value(
            "tiers",
            TestValue::Array(vec![
                TestValue::Null,
                tier(resource(range), vec![TestValue::Null, named_selection()]),
                TestValue::Null,
                tier(item(4), Vec::new()),
            ]),
        );
    TestRecord::new(
        BLUEPRINT,
        RECORD,
        format!("{RECORD}.{name}"),
        format!("{ROOT}/blueprints/crafting/{name}.xml"),
    )
    .value("blueprint", TestValue::Struct(source))
}

fn tier(mandatory: TestValue, options: Vec<TestValue>) -> TestValue {
    let duration = TestInstance::new(DURATION)
        .value("days", TestValue::Int32(0))
        .value("hours", TestValue::Int32(0))
        .value("minutes", TestValue::Int32(1))
        .value("seconds", TestValue::Single(15.0));
    let costs = TestInstance::new(COSTS)
        .value("craftTime", TestValue::Struct(duration))
        .value("mandatoryCost", mandatory)
        .value("optionalCosts", TestValue::Array(options));
    let recipe = TestInstance::new(RECIPE).value("costs", TestValue::Struct(costs));
    TestValue::Struct(TestInstance::new(TIER).value("recipe", TestValue::Struct(recipe)))
}

fn named_selection() -> TestValue {
    let inner = selection(vec![TestValue::Null, item(2)]);
    let options = vec![TestValue::Null, TestValue::Struct(inner), item(3)];
    let name = TestInstance::new(INFO).value("debugName", TestValue::text("FixtureChoice"));
    TestValue::Struct(selection(options).value("nameInfo", TestValue::Struct(name)))
}

fn selection(options: Vec<TestValue>) -> TestInstance {
    TestInstance::new(SELECTION)
        .value("count", TestValue::Int32(1))
        .value("options", TestValue::Array(options))
}

fn item(quantity: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(ITEM)
            .value("entityClass", TestValue::Reference(PRODUCT))
            .value("quantity", TestValue::Int32(quantity))
            .value("minQuality", TestValue::Int32(200)),
    )
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn recipe_context(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("ResourceType"))
        .structure(TestStruct::new("CraftingGameplayPropertyDef"))
        .structure(
            TestStruct::new(RESOURCE)
                .parent(COST)
                .field("resource", TestType::Reference)
                .field("minQuality", TestType::Int32)
                .field("quantity", TestType::Class(SCU.to_owned())),
        )
        .structure(TestStruct::new(SCU).field("standardCargoUnits", TestType::Single))
        .structure(TestStruct::new(CONTEXT))
        .structure(
            TestStruct::new(MULTIPLIER)
                .parent(CONTEXT)
                .field("multiplier", TestType::Single),
        )
        .structure(
            TestStruct::new(INCLUSION)
                .parent(CONTEXT)
                .field("option", TestType::String),
        )
        .structure(
            TestStruct::new(MODIFIERS)
                .parent(CONTEXT)
                .field("gameplayPropertyModifiers", pointer(MODIFIER_LIST)),
        )
        .structure(
            TestStruct::new(MODIFIER_LIST).array("gameplayPropertyModifiers", pointer(MODIFIER)),
        )
        .structure(
            TestStruct::new(MODIFIER)
                .field("gameplayPropertyRecord", TestType::Reference)
                .array("valueRanges", pointer(RANGE_BASE)),
        )
        .structure(
            TestStruct::new(RANGE_BASE)
                .field("startQuality", TestType::Int32)
                .field("endQuality", TestType::Int32)
                .field("modifierAtStart", TestType::Single)
                .field("modifierAtEnd", TestType::Single),
        )
        .structure(TestStruct::new(LINEAR).parent(RANGE_BASE))
        .structure(TestStruct::new(UNKNOWN_RANGE).parent(RANGE_BASE))
}

fn resource(range: RangeType) -> TestValue {
    let context = vec![
        TestValue::Struct(
            TestInstance::new(MULTIPLIER).value("multiplier", TestValue::Single(1.25)),
        ),
        TestValue::Struct(TestInstance::new(INCLUSION).value("option", TestValue::text("Include"))),
        modifiers(range),
    ];
    let quantity = TestInstance::new(SCU).value("standardCargoUnits", TestValue::Single(0.5));
    TestValue::Struct(
        TestInstance::new(RESOURCE)
            .value("resource", TestValue::Reference(MATERIAL))
            .value("minQuality", TestValue::Int32(150))
            .value("quantity", TestValue::Struct(quantity))
            .value("context", TestValue::Array(context)),
    )
}

fn modifiers(range: RangeType) -> TestValue {
    let range = TestInstance::new(range.type_name())
        .value("startQuality", TestValue::Int32(0))
        .value("endQuality", TestValue::Int32(1000))
        .value("modifierAtStart", TestValue::Single(0.75))
        .value("modifierAtEnd", TestValue::Single(1.25));
    let modifier = TestInstance::new(MODIFIER)
        .value("gameplayPropertyRecord", TestValue::Reference(PROPERTY))
        .value(
            "valueRanges",
            TestValue::Array(vec![TestValue::Struct(range)]),
        );
    let list = TestInstance::new(MODIFIER_LIST).value(
        "gameplayPropertyModifiers",
        TestValue::Array(vec![TestValue::Struct(modifier)]),
    );
    TestValue::Struct(
        TestInstance::new(MODIFIERS).value("gameplayPropertyModifiers", TestValue::Struct(list)),
    )
}
