// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1059_0040;
pub(super) const POWER: &str = "EntityComponentPowerConnection";
pub(super) const HEAT: &str = "EntityComponentHeatConnection";
pub(super) const KEY: &str = "@fixture_misfire_type";
pub(super) const POWER_SINGLES: [(&str, f32); 11] = [
    ("PowerBase", -0.0),
    ("PowerDraw", 2.0),
    ("TimeToReachDrawRequest", 3.0),
    ("OverclockThresholdMin", 4.0),
    ("OverclockThresholdMax", 5.0),
    ("OverpowerPerformance", 6.0),
    ("OverclockPerformance", 7.0),
    ("PowerToEM", 8.0),
    ("DecayRateOfEM", 9.0),
    ("WarningDelayTime", 10.0),
    ("WarningDisplayTime", f32::from_bits(0x7fc0_1059)),
];
pub(super) const HEAT_SINGLES: [(&str, f32); 19] = [
    ("TemperatureToIR", -0.0),
    ("StartIRTemperature", 2.0),
    ("OverpowerHeat", 3.0),
    ("OverclockThresholdMinHeat", 4.0),
    ("OverclockThresholdMaxHeat", 5.0),
    ("ThermalEnergyBase", 6.0),
    ("ThermalEnergyDraw", 7.0),
    ("ThermalConductivity", 8.0),
    ("SpecificHeatCapacity", 9.0),
    ("Mass", 10.0),
    ("SurfaceArea", 11.0),
    ("StartCoolingTemperature", 12.0),
    ("MaxCoolingRate", 13.0),
    ("MaxTemperature", 14.0),
    ("OverheatTemperature", 15.0),
    ("RecoveryTemperature", 16.0),
    ("MinTemperature", 17.0),
    ("MisfireMinTemperature", 18.0),
    ("MisfireMaxTemperature", 19.0),
];

const RESOURCE: &str = "ItemResourceComponentParams";
const PARAMETER: &str = "ItemResourceControlParameterBase";
const NETWORK: &str = "ItemResourceControlParameterRNParameter";
const EXTERNAL: &str = "ItemResourceControlRNParameterExternalPort";
const FUTURE: &str = "FixtureFutureResourceParameter";
const ACCESS: &str = "ResourceNetworkAcessParameter";
const MISFIRE_GENERATION: &str = "SMisfireGenerationParams";
const MISFIRE_EVENTS: &str = "SMisfireEvents";
const MISFIRE_EVENT: &str = "SMisfireEventParams";

pub(super) fn database() -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    connections_schema(parameter_schema(builder)).record(
        TestEntity::new(
            OWNER,
            "FixturePowerAndHeat",
            "libs/foundry/records/entities/scitem/ships/resource/fixture_connections.xml",
        )
        .attach_type("Misc")
        .component(node(power()))
        .component(node(heat()))
        .component(node(resource()))
        .record(),
    )
}

fn connections_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    let power = single_fields(POWER, &POWER_SINGLES)
        .field("SafeguardPriority", TestType::Int32)
        .field("DisplayedInPoweredItemList", TestType::Bool)
        .field("IsThrottleable", TestType::Bool)
        .field("IsOverclockable", TestType::Bool)
        .field("MisfireItemTypeLocID", TestType::Locale)
        .field("MisfireGenerationParams", pointer(MISFIRE_GENERATION))
        .field("MisfireEvents", class(MISFIRE_EVENTS));
    builder
        .structure(power)
        .structure(single_fields(HEAT, &HEAT_SINGLES).field("fixtureThermalBias", TestType::UInt16))
        .structure(TestStruct::new(MISFIRE_GENERATION).field("maxWindowLength", TestType::Single))
        .structure(TestStruct::new(MISFIRE_EVENTS).array("MajorMisfires", class(MISFIRE_EVENT)))
        .structure(TestStruct::new(MISFIRE_EVENT).field("duration", TestType::Single))
}

fn parameter_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(ACCESS, ["FixtureFirst", "FixtureSecond"])
        .structure(
            TestStruct::new(RESOURCE)
                .parent("DataForgeComponentParams")
                .array("controlParameters", pointer(PARAMETER)),
        )
        .structure(TestStruct::new(PARAMETER))
        .structure(
            TestStruct::new(NETWORK)
                .parent(PARAMETER)
                .field("parameter", TestType::Enum(ACCESS.to_owned())),
        )
        .structure(
            TestStruct::new(EXTERNAL)
                .parent(PARAMETER)
                .field("parameter", TestType::Enum(ACCESS.to_owned()))
                .field("name", TestType::String)
                .field("portName", TestType::String),
        )
        .structure(
            TestStruct::new(FUTURE)
                .parent(PARAMETER)
                .field("fixtureExternalFact", TestType::Bool),
        )
}

fn power() -> TestInstance {
    let generation =
        TestInstance::new(MISFIRE_GENERATION).value("maxWindowLength", TestValue::Single(0.0));
    let event = TestInstance::new(MISFIRE_EVENT).value("duration", TestValue::Single(-0.0));
    let events = TestInstance::new(MISFIRE_EVENTS)
        .value("MajorMisfires", TestValue::Array(vec![node(event)]));
    single_values(POWER, &POWER_SINGLES)
        .value("SafeguardPriority", TestValue::Int32(0))
        .value("DisplayedInPoweredItemList", TestValue::Bool(false))
        .value("IsThrottleable", TestValue::Bool(true))
        .value("IsOverclockable", TestValue::Bool(false))
        .value("MisfireItemTypeLocID", TestValue::text(KEY))
        .value("MisfireGenerationParams", node(generation))
        .value("MisfireEvents", node(events))
}

fn heat() -> TestInstance {
    single_values(HEAT, &HEAT_SINGLES).value("fixtureThermalBias", TestValue::UInt16(12))
}

fn resource() -> TestInstance {
    let external = |label| {
        node(
            TestInstance::new(EXTERNAL)
                .value("parameter", TestValue::text(label))
                .value("name", TestValue::text(""))
                .value("portName", TestValue::text("fixture_external")),
        )
    };
    TestInstance::new(RESOURCE).value(
        "controlParameters",
        TestValue::Array(vec![
            node(TestInstance::new(NETWORK).value("parameter", TestValue::text("FixtureFirst"))),
            TestValue::Null,
            external("FixtureFirst"),
            external("FixtureSecond"),
            node(TestInstance::new(FUTURE).value("fixtureExternalFact", TestValue::Bool(true))),
        ]),
    )
}

fn single_fields(name: &str, fields: &[(&str, f32)]) -> TestStruct {
    fields.iter().fold(
        TestStruct::new(name).parent("DataForgeComponentParams"),
        |structure, &(field, _)| structure.field(field, TestType::Single),
    )
}

fn single_values(name: &str, values: &[(&str, f32)]) -> TestInstance {
    values
        .iter()
        .fold(TestInstance::new(name), |instance, &(name, value)| {
            instance.value(name, TestValue::Single(value))
        })
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
