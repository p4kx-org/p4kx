// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic DataForge with multi-tier blueprints, selections, modifiers, pools, contracts,
//! quality distributions and a dismantle process.

mod records;
mod structures;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};

pub(crate) const TWO_TIERS: u128 = 0xa100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CHOOSE_ONE: u128 = 0xa200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const LEFTOVER: u128 = 0xa300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const BARE: u128 = 0xa400_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const IRON: u128 = 0xb100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const GOLD: u128 = 0xb200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const PRODUCT_A: u128 = 0xc100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PRODUCT_B: u128 = 0xc200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const BLACKLISTED: u128 = 0xc300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const PROPERTY: u128 = 0xd100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const POOL: u128 = 0xe100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const EMPTY_POOL: u128 = 0xe200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const BAD_POOL: u128 = 0xe500_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const CONTRACTS: u128 = 0xe300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const CONTRACT_ID: u128 = 0xe400_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const NAMELESS_CONTRACT_ID: u128 = 0xe600_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const LEFTOVER_POOL: u128 = 0xe700_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const DEFAULT_CURVE: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const OVERRIDE_CURVE: u128 = 0xf200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const PYRO: u128 = 0xf300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const DISMANTLE: u128 = 0x1100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const GLOBAL: u128 = 0x1200_0000_0000_4000_8000_0000_0000_0002;

pub(super) const CRAFTING: &str = "libs/foundry/records/crafting";

fn resource(guid: u128, scu: f32, context: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCost_Resource")
            .value("context", TestValue::Array(context))
            .value("minQuality", TestValue::Int32(300))
            .value("resource", TestValue::Reference(guid))
            .value(
                "quantity",
                TestValue::Struct(
                    TestInstance::new("SStandardCargoUnit")
                        .value("standardCargoUnits", TestValue::Single(scu)),
                ),
            ),
    )
}

fn item(guid: u128, units: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCost_Item")
            .value("quantity", TestValue::Int32(units))
            .value("entityClass", TestValue::Reference(guid)),
    )
}

fn select(count: i32, debug: &str, options: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCost_Select")
            .value("count", TestValue::Int32(count))
            .value(
                "nameInfo",
                TestValue::Struct(
                    TestInstance::new("CraftingNameInfo")
                        .value("debugName", TestValue::text(debug))
                        .value("displayName", TestValue::text("@crafting_slot")),
                ),
            )
            .value("options", TestValue::Array(options)),
    )
}

fn multiplier(factor: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCostContext_QuantityMultiplier")
            .value("multiplier", TestValue::Single(factor)),
    )
}

fn inclusion(option: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCostContext_ResultCompositionInclusion")
            .value("option", TestValue::text(option)),
    )
}

fn modifiers() -> TestValue {
    let linear = TestInstance::new("CraftingGameplayPropertyModifierValueRange_Linear")
        .value("startQuality", TestValue::Int32(0))
        .value("endQuality", TestValue::Int32(1000))
        .value("modifierAtStart", TestValue::Single(0.85))
        .value("modifierAtEnd", TestValue::Single(1.15));
    let additive =
        TestInstance::new("CraftingGameplayPropertyModifierValueRange_LinearIntegerAdditive")
            .value("startQuality", TestValue::Int32(0))
            .value("endQuality", TestValue::Int32(249))
            .value("additiveModifierAtStart", TestValue::Int32(-2))
            .value("additiveModifierAtEnd", TestValue::Int32(-1));
    let common = TestInstance::new("CraftingGameplayPropertyModifierCommon")
        .value("gameplayPropertyRecord", TestValue::Reference(PROPERTY))
        .value(
            "valueRanges",
            TestValue::Array(vec![TestValue::Struct(linear), TestValue::Struct(additive)]),
        );
    TestValue::Struct(
        TestInstance::new("CraftingCostContext_ResultGameplayPropertyModifiers").value(
            "gameplayPropertyModifiers",
            TestValue::Struct(
                TestInstance::new("CraftingGameplayPropertyModifiers_List").value(
                    "gameplayPropertyModifiers",
                    TestValue::Array(vec![TestValue::Struct(common)]),
                ),
            ),
        ),
    )
}

pub(super) fn time(minutes: i32, seconds: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("TimeValue_Partitioned")
            .value("days", TestValue::Int32(0))
            .value("hours", TestValue::Int32(0))
            .value("minutes", TestValue::Int32(minutes))
            .value("seconds", TestValue::Single(seconds)),
    )
}

fn tier(craft_time: TestValue, mandatory: TestValue, optional: Vec<TestValue>) -> TestValue {
    let costs = TestInstance::new("CraftingRecipeCosts")
        .value("craftTime", craft_time)
        .value("mandatoryCost", mandatory)
        .value("optionalCosts", TestValue::Array(optional));
    TestValue::Struct(TestInstance::new("CraftingBlueprintTier").value(
        "recipe",
        TestValue::Struct(
            TestInstance::new("CraftingRecipe").value("costs", TestValue::Struct(costs)),
        ),
    ))
}

fn blueprint(
    guid: u128,
    name: &str,
    path: &str,
    produces: u128,
    tiers: Vec<TestValue>,
) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingBlueprintRecord",
        format!("CraftingBlueprintRecord.{name}"),
        format!("{CRAFTING}/blueprints/crafting/{path}.xml"),
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new("CraftingBlueprint")
                .value(
                    "processSpecificData",
                    TestValue::Struct(
                        TestInstance::new("CraftingProcess_Creation")
                            .value("entityClass", TestValue::Reference(produces)),
                    ),
                )
                .value("tiers", TestValue::Array(tiers)),
        ),
    )
}

fn two_tiers() -> Vec<TestValue> {
    let first = tier(
        time(1, 30.0),
        select(
            2,
            "ASPECTS",
            vec![
                select(
                    1,
                    "FRAME",
                    vec![resource(IRON, 0.5, vec![multiplier(1.15)])],
                ),
                select(
                    1,
                    "CORE",
                    vec![resource(GOLD, 0.25, vec![inclusion("Include")])],
                ),
            ],
        ),
        vec![item(PRODUCT_B, 3)],
    );
    let second = tier(
        time(2, 0.0),
        select(1, "ASPECTS", vec![resource(IRON, 1.0, vec![modifiers()])]),
        Vec::new(),
    );
    vec![first, second]
}

fn blueprints(builder: DataForgeBuilder) -> DataForgeBuilder {
    let choose_one = tier(
        time(0, 10.0),
        select(
            1,
            "PICK",
            vec![
                resource(IRON, 0.1, Vec::new()),
                resource(GOLD, 0.1, Vec::new()),
                item(PRODUCT_B, 1),
            ],
        ),
        Vec::new(),
    );
    builder
        .record(blueprint(
            TWO_TIERS,
            "BP_CRAFT_two_tiers",
            "fps/bp_craft_two_tiers",
            PRODUCT_A,
            two_tiers(),
        ))
        .record(blueprint(
            CHOOSE_ONE,
            "BP_CRAFT_pick",
            "fps/bp_craft_pick",
            PRODUCT_B,
            vec![choose_one],
        ))
        .record(blueprint(
            LEFTOVER,
            "BP_CRAFT_dev_test",
            "fps/bp_craft_dev_test",
            PRODUCT_A,
            Vec::new(),
        ))
        .record(blueprint(
            BARE,
            "BP_CRAFT_bare",
            "fps/bp_craft_bare",
            BLACKLISTED,
            Vec::new(),
        ))
}

pub(crate) const UNKNOWN_RANGES: u128 = 0xa500_0000_0000_4000_8000_0000_0000_0005;

fn unknown_range(type_name: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(type_name)
            .value("startQuality", TestValue::Int32(0))
            .value("endQuality", TestValue::Int32(1000)),
    )
}

/// Returns the DataForge of the fixture with a blueprint whose modifiers hold three ranges of
/// two types the reader does not know.
pub(crate) fn unknown_ranges_database() -> DataForgeBuilder {
    let ranges = vec![
        unknown_range("CraftingGameplayPropertyModifierValueRange_Stepped"),
        unknown_range("CraftingGameplayPropertyModifierValueRange_Curve"),
        unknown_range("CraftingGameplayPropertyModifierValueRange_Stepped"),
    ];
    let common = TestInstance::new("CraftingGameplayPropertyModifierCommon")
        .value("gameplayPropertyRecord", TestValue::Reference(PROPERTY))
        .value("valueRanges", TestValue::Array(ranges));
    let context = TestValue::Struct(
        TestInstance::new("CraftingCostContext_ResultGameplayPropertyModifiers").value(
            "gameplayPropertyModifiers",
            TestValue::Struct(
                TestInstance::new("CraftingGameplayPropertyModifiers_List").value(
                    "gameplayPropertyModifiers",
                    TestValue::Array(vec![TestValue::Struct(common)]),
                ),
            ),
        ),
    );
    let only = tier(
        time(1, 0.0),
        select(1, "ASPECTS", vec![resource(IRON, 1.0, vec![context])]),
        Vec::new(),
    );
    database().record(blueprint(
        UNKNOWN_RANGES,
        "BP_CRAFT_unknown_ranges",
        "fps/bp_craft_unknown_ranges",
        PRODUCT_A,
        vec![only],
    ))
}

pub(crate) const TOO_DEEP: u128 = 0xa600_0000_0000_4000_8000_0000_0000_0006;

/// Returns the DataForge of the fixture with a blueprint whose selections nest `levels` deep.
pub(crate) fn nested_database(levels: usize) -> DataForgeBuilder {
    let nested = (0..levels).fold(resource(IRON, 1.0, Vec::new()), |inner, level| {
        select(1, &format!("LEVEL{level}"), vec![inner])
    });
    let only = tier(time(1, 0.0), nested, Vec::new());
    database().record(blueprint(
        TOO_DEEP,
        "BP_CRAFT_too_deep",
        "fps/bp_craft_too_deep",
        PRODUCT_A,
        vec![only],
    ))
}

pub(crate) const SECOND_DISMANTLE: u128 = 0x1300_0000_0000_4000_8000_0000_0000_0003;

/// Returns the DataForge of the fixture with a second dismantle process.
pub(crate) fn ambiguous_database() -> DataForgeBuilder {
    database().record(records::dismantle(SECOND_DISMANTLE, "SecondDismantle"))
}

/// Returns the DataForge of the fixture.
pub(crate) fn database() -> DataForgeBuilder {
    let builder = structures::structures(DataForgeBuilder::new().version(8));
    let builder = [IRON, GOLD, PRODUCT_A, PRODUCT_B, BLACKLISTED]
        .into_iter()
        .zip(1_u32..)
        .fold(builder, |builder, (guid, number)| {
            builder.record(TestRecord::new(
                guid,
                "Stub",
                format!("Stub.Stub{number}"),
                format!("libs/foundry/records/stubs/stub{number}.xml"),
            ))
        });
    let builder = builder.record(TestRecord::new(
        PYRO,
        "StarMapObject",
        "StarMapObject.FixtureLocation",
        "libs/foundry/records/stubs/fixture_location.xml",
    ));
    records::dismantle_and_global(records::quality(records::rewards(blueprints(builder))))
}
