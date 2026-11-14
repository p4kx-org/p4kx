// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestPort,
    TestRecord, TestStruct, TestType, TestValue,
};

pub(super) const SHIP: u128 = 0x1061;
pub(super) const HYDROGEN_TANK: u128 = 0x1062;
pub(super) const QUANTUM_TANK: u128 = 0x1063;
pub(super) const INTAKE: u128 = 0x1064;
pub(super) const DOCKED: u128 = 0x1065;
pub(super) const HYDROGEN: u128 = 0x1066;
pub(super) const QUANTUM: u128 = 0x1067;
pub(super) const MISSING: u128 = 0x1068;
pub(super) const FUEL_NAN_BITS: u32 = 0x7fc0_1061;

const TANK_TYPE: &str = "SCItemFuelTankParams";
const INTAKE_TYPE: &str = "SCItemFuelIntakeParams";
const CONTAINER: &str = "ResourceContainer";
const BASE: &str = "SBaseCargoUnit";
const COMPOSITION: &str = "ResourceContainerDefaultCompositionEntry";
const STATE: &str = "SInteractionState";
const RTPC: &str = "AudioRtpc";
const AUDIO: &str = "GlobalResourceAudio";
const MUTABILITY: &str = "EResourceContainerMutabilityLevel";

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Full,
    CapacityAbsent,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    resource_schema(fuel_schema(builder), layout)
        .record(ship(layout))
        .record(hydrogen_tank(layout))
        .record(quantum_tank(layout))
        .record(intake())
        .record(docked())
        .record(resource(HYDROGEN, "FixtureHydrogen"))
        .record(resource(QUANTUM, "FixtureQuantum"))
}

fn fuel_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(STATE).field("stateName", TestType::String))
        .structure(TestStruct::new(AUDIO).field("audioTrigger", TestType::String))
        .structure(TestStruct::new(RTPC).field("rtpc", TestType::String))
        .structure(
            component(TANK_TYPE)
                .array("fixtureStates", class(STATE))
                .field("hydrogenMaxFlowMultiplier", TestType::Single)
                .field("quantumMaxFlowMultiplier", TestType::Single)
                .field("openState", TestType::WeakPointer(STATE.to_owned()))
                .field("closedState", TestType::WeakPointer(STATE.to_owned()))
                .field("pumpingState", TestType::WeakPointer(STATE.to_owned()))
                .field("fuelFlowLoopStartAudioTrigger", class(AUDIO))
                .field("fuelFlowLoopStopAudioTrigger", class(AUDIO))
                .field("fuelFillDrainRateAudioRtpc", class(RTPC))
                .field("fuelFillLevelAudioRtpc", class(RTPC)),
        )
        .structure(
            component(INTAKE_TYPE)
                .field("fuelPushRate", TestType::Single)
                .field("minimumRate", TestType::Single),
        )
}

fn resource_schema(builder: DataForgeBuilder, layout: Layout) -> DataForgeBuilder {
    let mut container = component(CONTAINER);
    if matches!(layout, Layout::Full) {
        container = container.field("capacity", TestType::StrongPointer(BASE.to_owned()));
    }
    let resource = TestType::ReferenceTo("ResourceType".to_owned());
    let group = TestType::ReferenceTo("ResourceTypeGroup".to_owned());
    builder
        .enumeration(MUTABILITY, ["FixtureMutable"])
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new("SCentiCargoUnit")
                .parent(BASE)
                .field("centiSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new("SMicroCargoUnit")
                .parent(BASE)
                .field("microSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new("SStandardCargoUnit")
                .parent(BASE)
                .field("standardCargoUnits", TestType::Single),
        )
        .structure(
            TestStruct::new("FixtureCargoUnit")
                .parent(BASE)
                .field("fixtureUnitAmount", TestType::Single),
        )
        .structure(TestStruct::new("ResourceType"))
        .structure(TestStruct::new("ResourceTypeGroup"))
        .structure(
            TestStruct::new(COMPOSITION)
                .field("entry", resource.clone())
                .field("weight", TestType::Single)
                .field("fixtureQuality", TestType::UInt32),
        )
        .structure(
            container
                .field("immutable", TestType::Bool)
                .field("mutabilityLevel", TestType::Enum(MUTABILITY.to_owned()))
                .field("generateRandomQuality", TestType::Bool)
                .array("inclusiveResources", resource.clone())
                .array("exclusiveResources", resource)
                .array("inclusiveGroups", group.clone())
                .array("exclusiveGroups", group)
                .array("defaultComposition", class(COMPOSITION))
                .field("defaultCompositionFillFactor", TestType::Single)
                .field("hideFromUI", TestType::Bool),
        )
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}

fn ship(layout: Layout) -> TestRecord {
    ship_entity(SHIP, "FixtureFuelShip", "fuel_ship")
        .port(TestPort::new("fuel_a").default_item(QUANTUM_TANK))
        .port(TestPort::new("fuel_b").default_item(HYDROGEN_TANK))
        .port(TestPort::new("intake").default_item(INTAKE))
        .port(TestPort::new("hangar").default_item(DOCKED))
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
            "fuel_a",
            HYDROGEN_TANK,
        )]))
        .component(container(
            layout,
            TestValue::Null,
            &[],
            f32::from_bits(FUEL_NAN_BITS),
        ))
        .record()
}

fn docked() -> TestRecord {
    ship_entity(DOCKED, "FixtureFuelDocked", "docked")
        .attach_type("NOITEM_Vehicle")
        .port(TestPort::new("contained_tank").default_item(QUANTUM_TANK))
        .record()
}

fn ship_entity(guid: u128, name: &str, filename: &str) -> TestEntity {
    TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/spaceships/{filename}.xml"),
    )
}

fn item(guid: u128, name: &str, filename: &str) -> TestEntity {
    TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/scitem/fixture/{filename}.xml"),
    )
    .attach_type("PowerPlant")
}

fn hydrogen_tank(layout: Layout) -> TestRecord {
    let tank = TestInstance::new(TANK_TYPE)
        .value(
            "fixtureStates",
            TestValue::Array(vec![node(
                TestInstance::new(STATE)
                    .named("fixture-open")
                    .value("stateName", TestValue::text("FixtureOpen")),
            )]),
        )
        .value("hydrogenMaxFlowMultiplier", TestValue::Single(1.5))
        .value("quantumMaxFlowMultiplier", TestValue::Single(0.25))
        .value("openState", TestValue::pointer("fixture-open"))
        .value(
            "fuelFlowLoopStartAudioTrigger",
            audio(AUDIO, "audioTrigger", "FixtureStart"),
        )
        .value(
            "fuelFlowLoopStopAudioTrigger",
            audio(AUDIO, "audioTrigger", "FixtureStop"),
        )
        .value(
            "fuelFillDrainRateAudioRtpc",
            audio(RTPC, "rtpc", "FixtureRate"),
        )
        .value(
            "fuelFillLevelAudioRtpc",
            audio(RTPC, "rtpc", "FixtureLevel"),
        );
    let capacity = unit("SCentiCargoUnit", "centiSCU", TestValue::Int32(125));
    item(HYDROGEN_TANK, "FixtureHydrogenTank", "hydrogen_tank")
        .component(node(tank))
        .component(container(
            layout,
            capacity,
            &[(HYDROGEN, 0.25), (MISSING, 0.5), (QUANTUM, 0.25)],
            0.75,
        ))
        .record()
}

fn quantum_tank(layout: Layout) -> TestRecord {
    let tank = TestInstance::new(TANK_TYPE)
        .value("hydrogenMaxFlowMultiplier", TestValue::Single(0.0))
        .value("quantumMaxFlowMultiplier", TestValue::Single(2.0));
    let micro = unit("SMicroCargoUnit", "microSCU", TestValue::Int32(0));
    let standard = unit(
        "SStandardCargoUnit",
        "standardCargoUnits",
        TestValue::Single(2.25),
    );
    let future = unit(
        "FixtureCargoUnit",
        "fixtureUnitAmount",
        TestValue::Single(3.5),
    );
    item(QUANTUM_TANK, "FixtureQuantumTank", "quantum_tank")
        .component(node(tank))
        .component(container(layout, micro, &[(QUANTUM, 1.0)], 0.0))
        .component(container(layout, standard, &[], 0.0))
        .component(container(layout, future, &[], 0.0))
        .record()
}

fn intake() -> TestRecord {
    item(INTAKE, "FixtureFuelIntake", "intake")
        .component(node(
            TestInstance::new(INTAKE_TYPE)
                .value("fuelPushRate", TestValue::Single(1.75))
                .value("minimumRate", TestValue::Single(-0.0)),
        ))
        .record()
}

fn audio(kind: &str, field: &str, value: &str) -> TestValue {
    node(TestInstance::new(kind).value(field, TestValue::text(value)))
}

fn unit(kind: &str, field: &str, amount: TestValue) -> TestValue {
    node(TestInstance::new(kind).value(field, amount))
}

fn container(layout: Layout, capacity: TestValue, entries: &[(u128, f32)], fill: f32) -> TestValue {
    let mut source = TestInstance::new(CONTAINER)
        .value("immutable", TestValue::Bool(false))
        .value("mutabilityLevel", TestValue::text("FixtureMutable"))
        .value("generateRandomQuality", TestValue::Bool(true))
        .value(
            "inclusiveResources",
            TestValue::Array(vec![
                TestValue::Reference(HYDROGEN),
                TestValue::Reference(QUANTUM),
            ]),
        )
        .value(
            "exclusiveResources",
            TestValue::Array(vec![TestValue::Reference(MISSING)]),
        )
        .value(
            "defaultComposition",
            TestValue::Array(
                entries
                    .iter()
                    .map(|&(guid, weight)| {
                        node(
                            TestInstance::new(COMPOSITION)
                                .value("entry", TestValue::Reference(guid))
                                .value("weight", TestValue::Single(weight))
                                .value("fixtureQuality", TestValue::UInt32(12)),
                        )
                    })
                    .collect(),
            ),
        )
        .value("defaultCompositionFillFactor", TestValue::Single(fill))
        .value("hideFromUI", TestValue::Bool(false));
    if matches!(layout, Layout::Full) {
        source = source.value("capacity", capacity);
    }
    node(source)
}

fn resource(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypes/{name}.xml"),
    )
}
