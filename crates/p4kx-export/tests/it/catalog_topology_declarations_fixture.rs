// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";
const HARDPOINT: &str = "SInternalHardpointLink";
const RESOURCE_LINK: &str = "SInternalResourceLink";
const PORT: &str = "SItemPortDef";
const VEHICLE: &str = "VehicleComponentParams";
const ROOM: &str = "SVehicleObjectContainerParams";
const ITEM_ROOM: &str = "SObjectContainerComponentParams";
pub(super) const ROOM_FILE: &str = "data/fixture/ship-room.socpak";
pub(super) const ITEM_FILE: &str = "data/fixture/item-room.socpak";
const PORT_IDENTITY: &str = "fixture-port";

pub(super) fn database() -> DataForgeBuilder {
    database_with_room_extension(TestValue::text("Fixture extension"))
}

pub(super) fn database_with_room_extension(extension: TestValue) -> DataForgeBuilder {
    database_with_link_endpoint(extension, TestValue::text("unbound"))
}

pub(super) fn database_with_link_endpoint(
    extension: TestValue,
    endpoint: TestValue,
) -> DataForgeBuilder {
    schema()
        .record(
            TestRecord::new(
                0x1166,
                ENTITY,
                "EntityClassDefinition.FixtureTopology",
                "libs/foundry/records/entities/spaceships/fixture_topology_sdk.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![ports(endpoint), vehicle()]),
            ),
        )
        .record(
            TestRecord::new(
                0x1167,
                ENTITY,
                "EntityClassDefinition.FixtureRoomItem",
                "libs/foundry/records/entities/scitem/fixture_room_sdk.xml",
            )
            .value(
                "Components",
                slots(
                    TestInstance::new(ITEM_ROOM)
                        .value("objectContainer", TestValue::text(ITEM_FILE))
                        .value("fixtureExtension", extension),
                ),
            ),
        )
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(PORT_CONTAINER)
                .parent(COMPONENT)
                .array("Ports", class(PORT))
                .field("PortFlags", TestType::String)
                .array("InternalHardpointLinks", class(HARDPOINT))
                .array("InternalResourceLinks", class(RESOURCE_LINK))
                .field("LinkAllItemsToResourceNetwork", TestType::Bool),
        )
        .structure(TestStruct::new(PORT).field("Name", TestType::String))
        .structure(
            TestStruct::new(HARDPOINT)
                .field("port1", TestType::String)
                .field("port2", TestType::String),
        )
        .structure(
            TestStruct::new(RESOURCE_LINK)
                .field("port1", TestType::WeakPointer(PORT.to_owned()))
                .field("port2", TestType::WeakPointer(PORT.to_owned())),
        )
        .structure(
            TestStruct::new(VEHICLE)
                .parent(COMPONENT)
                .field("vehicleDefinition", TestType::String)
                .array("objectContainers", class(ROOM)),
        )
        .structure(
            TestStruct::new(ROOM)
                .field("fileName", TestType::String)
                .field("boneName", TestType::String)
                .field("resourceNetworkItemportName", TestType::String),
        )
        .structure(
            TestStruct::new(ITEM_ROOM)
                .parent(COMPONENT)
                .field("objectContainer", TestType::String)
                .field("fixtureExtension", TestType::String),
        )
}

fn ports(endpoint: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PORT_CONTAINER)
            .value(
                "Ports",
                slots(
                    TestInstance::new(PORT)
                        .named(PORT_IDENTITY)
                        .value("Name", TestValue::text("mount")),
                ),
            )
            .value("PortFlags", bad_string())
            .value(
                "InternalHardpointLinks",
                slots(
                    TestInstance::new(HARDPOINT)
                        .value("port1", TestValue::text("mount"))
                        .value("port2", endpoint),
                ),
            )
            .value(
                "InternalResourceLinks",
                slots(
                    TestInstance::new(RESOURCE_LINK)
                        .value("port1", TestValue::pointer(PORT_IDENTITY)),
                ),
            )
            .value("LinkAllItemsToResourceNetwork", TestValue::Bool(true)),
    )
}

fn vehicle() -> TestValue {
    TestValue::Struct(
        TestInstance::new(VEHICLE)
            .value("vehicleDefinition", bad_string())
            .value(
                "objectContainers",
                slots(
                    TestInstance::new(ROOM)
                        .value("fileName", TestValue::text(ROOM_FILE))
                        .value("boneName", TestValue::text("bridge"))
                        .value("resourceNetworkItemportName", TestValue::text("mount")),
                ),
            ),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn slots(instance: TestInstance) -> TestValue {
    TestValue::Array(vec![TestValue::Struct(instance)])
}

pub(super) fn bad_string() -> TestValue {
    TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
}
