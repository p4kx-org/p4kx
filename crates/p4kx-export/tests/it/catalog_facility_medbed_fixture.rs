// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestPort,
    TestRecord, TestStruct, TestType, TestValue,
};

pub(super) const OWNER: u128 = 0x1063_0300;
pub(super) const TIER_CONFIG: u128 = 0x1063_0301;
pub(super) const RESOURCE: u128 = 0x1063_0302;
pub(super) const SHIP: u128 = 0x1063_0303;
pub(super) const COMPONENT: &str = "MedBedComponentParams";
const CONFIG: &str = "MedicalItemTierConfig";
const TIER: &str = "MedBedTierParams";
const TIER_TYPE: &str = "MedBedTier";
const CONSUMPTION: &str = "MedBedResourceConsumptionParams";
const INTERACTION: &str = "SSharedInteractionParams";
const RANGE: &str = "MedBedRespawnRangeOverride";
const SURGERY_NAMES: &str = "MedBedSurgeryNames";
const INJURY_NAMES: &str = "MedBedSurgeryNamesInjurySeverity";
const STANDARD: &str = "SStandardCargoUnit";
const CENTI: &str = "SCentiCargoUnit";
const MICRO: &str = "SMicroCargoUnit";
const FUTURE: &str = "FixtureBedCargoUnit";
const CAPACITY: &str = "SBaseCargoUnit";
const STAT: &str = "ActorStatType";

pub(super) fn database() -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    let builder = units(config_schema(component_schema(builder)));
    builder
        .record(
            TestEntity::new(
                OWNER,
                "FixtureMedBed",
                "libs/foundry/records/entities/scitem/fixture/med_bed.xml",
            )
            .attach_type("Misc")
            .component(node(component()))
            .record(),
        )
        .record(configuration())
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "ResourceType.FixtureBedResource",
            "libs/foundry/records/resources/fixture_bed_resource.xml",
        ))
        .record(
            TestEntity::new(
                SHIP,
                "FixtureMedicalShip",
                "libs/foundry/records/entities/spaceships/fixture_medical_ship.xml",
            )
            .vehicle("", "")
            .port(TestPort::new("med_bay"))
            .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                "med_bay", OWNER,
            )]))
            .record(),
        )
}

fn component_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(TIER_TYPE, ["FixtureTierOne", "FixtureTierTwo"])
        .structure(TestStruct::new(INTERACTION).field("fixtureFlag", TestType::Bool))
        .structure(TestStruct::new(RANGE).field("respawnRange", TestType::Int64))
        .structure(TestStruct::new(SURGERY_NAMES).field("headInjuries", class(INJURY_NAMES)))
        .structure(TestStruct::new(INJURY_NAMES).field("injuryName", TestType::Locale))
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("fixtureInteraction", strong(INTERACTION))
                .field(
                    "healInteraction",
                    TestType::WeakPointer(INTERACTION.to_owned()),
                )
                .field(
                    "setRespawnInteraction",
                    TestType::WeakPointer(INTERACTION.to_owned()),
                )
                .field("respawnRangeOverride", strong(RANGE))
                .field("timeToHeal", TestType::Single)
                .field("delayBeforeHeal", TestType::Single)
                .field("medBedTier", TestType::Enum(TIER_TYPE.to_owned()))
                .field(
                    "medicalItemTierConfig",
                    TestType::ReferenceTo(CONFIG.to_owned()),
                )
                .field("canRespawnHere", TestType::Bool)
                .field("invulnerableUser", TestType::Bool)
                .field("invulnerableDuration", TestType::Single)
                .field("surgeryNames", class(SURGERY_NAMES))
                .field("resourceRegenerationPerMinute", strong(CAPACITY)),
        )
}

fn config_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(STAT, ["FixtureStatOne", "FixtureStatTwo"])
        .structure(TestStruct::new("ResourceType"))
        .structure(
            TestStruct::new(TIER)
                .field("medBedTier", TestType::Enum(TIER_TYPE.to_owned()))
                .field("defaultRespawnRange", TestType::Int64)
                .array("statTypesToHeal", TestType::Enum(STAT.to_owned())),
        )
        .structure(
            TestStruct::new(CONSUMPTION)
                .field(
                    "bedResource",
                    TestType::ReferenceTo("ResourceType".to_owned()),
                )
                .field("resourceAmountForSurgeryT1", strong(CAPACITY))
                .field("resourceAmountForSurgeryT2", strong(CAPACITY))
                .field("resourceAmountForSurgeryT3", strong(CAPACITY))
                .field("resourceAmountForCloning", strong(CAPACITY)),
        )
        .structure(
            TestStruct::new(CONFIG)
                .array("medBedConfigs", class(TIER))
                .field("resourceConsumption", class(CONSUMPTION)),
        )
}

fn units(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(CAPACITY))
        .structure(
            TestStruct::new(STANDARD)
                .parent(CAPACITY)
                .field("standardCargoUnits", TestType::Single),
        )
        .structure(
            TestStruct::new(CENTI)
                .parent(CAPACITY)
                .field("centiSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new(MICRO)
                .parent(CAPACITY)
                .field("microSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new(FUTURE)
                .parent(CAPACITY)
                .field("fixtureQuanta", TestType::UInt16),
        )
}

fn component() -> TestInstance {
    TestInstance::new(COMPONENT)
        .value(
            "fixtureInteraction",
            node(TestInstance::new(INTERACTION).named("fixture-heal")),
        )
        .value("healInteraction", TestValue::pointer("fixture-heal"))
        .value("setRespawnInteraction", TestValue::Null)
        .value(
            "respawnRangeOverride",
            node(TestInstance::new(RANGE).value("respawnRange", TestValue::Int64(0))),
        )
        .value("timeToHeal", TestValue::Single(-0.0))
        .value("delayBeforeHeal", TestValue::Single(2.5))
        .value("medBedTier", TestValue::text("FixtureTierTwo"))
        .value("medicalItemTierConfig", TestValue::Reference(TIER_CONFIG))
        .value("canRespawnHere", TestValue::Bool(false))
        .value("invulnerableUser", TestValue::Bool(true))
        .value("invulnerableDuration", TestValue::Single(0.0))
        .value(
            "surgeryNames",
            node(TestInstance::new(SURGERY_NAMES).value(
                "headInjuries",
                node(
                    TestInstance::new(INJURY_NAMES).value("injuryName", TestValue::text("@shared")),
                ),
            )),
        )
        .value(
            "resourceRegenerationPerMinute",
            node(TestInstance::new(MICRO).value("microSCU", TestValue::Int32(12))),
        )
}

fn configuration() -> TestRecord {
    TestRecord::new(
        TIER_CONFIG,
        CONFIG,
        "MedicalItemTierConfig.Fixture",
        "libs/foundry/records/medical/fixture_item_tiers.xml",
    )
    .value(
        "medBedConfigs",
        TestValue::Array(vec![
            node(tier("FixtureTierTwo", 0)),
            node(tier("FixtureTierOne", 73_001)),
            node(tier("FixtureTierTwo", 0)),
        ]),
    )
    .value(
        "resourceConsumption",
        node(
            TestInstance::new(CONSUMPTION)
                .value("bedResource", TestValue::Reference(RESOURCE))
                .value(
                    "resourceAmountForSurgeryT1",
                    node(
                        TestInstance::new(STANDARD)
                            .value("standardCargoUnits", TestValue::Single(0.75)),
                    ),
                )
                .value(
                    "resourceAmountForSurgeryT2",
                    node(TestInstance::new(CENTI).value("centiSCU", TestValue::Int32(9))),
                )
                .value(
                    "resourceAmountForSurgeryT3",
                    node(TestInstance::new(FUTURE).value("fixtureQuanta", TestValue::UInt16(3))),
                )
                .value("resourceAmountForCloning", TestValue::Null),
        ),
    )
}

fn tier(label: &str, range: i64) -> TestInstance {
    TestInstance::new(TIER)
        .value("medBedTier", TestValue::text(label))
        .value("defaultRespawnRange", TestValue::Int64(range))
        .value(
            "statTypesToHeal",
            TestValue::Array(
                ["FixtureStatTwo", "FixtureStatOne", "FixtureStatTwo"]
                    .into_iter()
                    .map(TestValue::text)
                    .collect(),
            ),
        )
}

fn strong(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
