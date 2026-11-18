// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestRecord,
    TestStruct, TestType, TestValue,
};

pub(super) const SHIP: u128 = 0x1064_0001;
pub(super) const RELAY: u128 = 0x1064_0002;
pub(super) const ACTOR: u128 = 0x1064_0003;
pub(super) const DOCKED: u128 = 0x1064_0004;
pub(super) const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";
pub(super) const HARDPOINT_LINK: &str = "SInternalHardpointLink";
pub(super) const RESOURCE_LINK: &str = "SInternalResourceLink";
pub(super) const VEHICLE: &str = "VehicleComponentParams";
pub(super) const ROOM: &str = "SVehicleObjectContainerParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const PORT: &str = "SItemPortDef";
const RESOURCE: &str = "ItemResourceComponentParams";
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const MANUAL_LOADOUT: &str = "SItemPortLoadoutManualParams";
const LOADOUT: &str = "SItemPortLoadoutBaseParams";
const ENTRY: &str = "SItemPortLoadoutEntryParams";
const SHIP_XML: &str = "Scripts/Entities/Vehicles/Implementations/Xml/fixture_topology.xml";
const DOCKED_XML: &str = "Scripts/Entities/Vehicles/Implementations/Xml/fixture_docked.xml";
const LOADOUT_XML: &str = "Scripts/Entities/Vehicles/Loadouts/fixture_topology_loadout.xml";
const PATCH_XML: &str = "Scripts/Entities/Vehicles/Implementations/Xml/fixture_topology_patch.xml";
#[path = "catalog_resource_topology_archive.rs"]
mod archive;

pub(super) use archive::{GraphSource, graph_archive, graph_archive_for};

pub(super) fn database() -> DataForgeBuilder {
    database_for(GraphSource::Database)
}

fn database_for(source: GraphSource) -> DataForgeBuilder {
    entity_schema()
        .record(ship(source))
        .record(item(RELAY, "SyntheticRelay", true, true))
        .record(item(ACTOR, "RELAY_2slot", false, false))
        .record(docked())
}

fn entity_schema() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new("SEntityComponentDefaultLoadoutParams")
                .parent(COMPONENT)
                .field("loadout", pointer(LOADOUT)),
        )
        .structure(TestStruct::new(LOADOUT))
        .structure(
            TestStruct::new("SItemPortLoadoutXMLParams")
                .parent(LOADOUT)
                .field("loadoutPath", TestType::String),
        )
        .structure(
            TestStruct::new(MANUAL_LOADOUT)
                .parent(LOADOUT)
                .array("entries", class(ENTRY)),
        )
        .structure(
            TestStruct::new(ENTRY)
                .field("itemPortName", TestType::String)
                .field("entityClassName", TestType::String)
                .field(
                    "entityClassReference",
                    TestType::ReferenceTo(ENTITY.to_owned()),
                )
                .field("loadout", pointer(LOADOUT)),
        )
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::String)
                .field("SubType", TestType::String)
                .field("Size", TestType::Int32)
                .field("Tags", TestType::String),
        )
        .structure(
            TestStruct::new(RESOURCE)
                .parent(COMPONENT)
                .field("isRelay", TestType::Bool)
                .field("isResourceNetworked", TestType::Bool),
        )
        .structure(port_container_schema());
    vehicle_schema(resource_schema(builder))
}

fn resource_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("EItemType", ["FixtureDevice"])
        .enumeration("EItemSubType", ["FixtureSubtype"])
        .structure(
            TestStruct::new("SItemPortDefTypes")
                .field("Type", TestType::Enum("EItemType".to_owned()))
                .array("SubTypes", TestType::Enum("EItemSubType".to_owned())),
        )
        .structure(
            TestStruct::new(PORT)
                .field("Name", TestType::String)
                .field("Flags", TestType::String)
                .field("resourceLinkToParent", TestType::Bool)
                .field("RequiredPortTags", TestType::String)
                .field("MinSize", TestType::Int32)
                .array("Types", class("SItemPortDefTypes")),
        )
        .structure(
            TestStruct::new(HARDPOINT_LINK)
                .field("port1", TestType::String)
                .field("port2", TestType::String),
        )
        .structure(
            TestStruct::new(RESOURCE_LINK)
                .field("port1", weak(PORT))
                .field("port2", weak(PORT)),
        )
}

fn vehicle_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(VEHICLE)
                .parent(COMPONENT)
                .field("vehicleDefinition", TestType::String)
                .field("modification", TestType::String)
                .array("objectContainers", class(ROOM)),
        )
        .structure(
            TestStruct::new("SAttachableObjectContainerData")
                .field("fileName", TestType::String)
                .field("guid", TestType::String),
        )
        .structure(
            TestStruct::new(ROOM)
                .parent("SAttachableObjectContainerData")
                .field("boneName", TestType::String)
                .field("resourceNetworkItemportName", TestType::String)
                .field("shouldRequestNavMeshRegenerationOnLoad", TestType::Bool),
        )
}

fn port_container_schema() -> TestStruct {
    TestStruct::new(PORT_CONTAINER)
        .parent(COMPONENT)
        .array("Ports", class(PORT))
        .field("PortFlags", TestType::String)
        .array("InternalHardpointLinks", class(HARDPOINT_LINK))
        .array("InternalResourceLinks", class(RESOURCE_LINK))
        .field("LinkAllItemsToResourceNetwork", TestType::Bool)
}

fn ship(source: GraphSource) -> TestRecord {
    let ports = [
        "mount_a", "mount_b", "deck", "loose", "hangar", "same", "SAME",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, name)| port(name, &format!("ship-port-{index}"), index == 0))
    .collect();
    let links = [
        ("mount_a", "deck"),
        ("deck", "mount_b"),
        ("mount_b", "mount_a"),
        ("mount_a", "deck"),
        ("same", "missing"),
        ("hangar", "deck"),
        ("vehicle_only", "deck"),
    ]
    .into_iter()
    .map(|(port1, port2)| hardpoint(port1, port2))
    .collect();
    let container = container(ports, links).value(
        "InternalResourceLinks",
        TestValue::Array(vec![node(
            TestInstance::new(RESOURCE_LINK)
                .value("port1", TestValue::pointer("ship-port-5"))
                .value("port2", TestValue::pointer("ship-port-2")),
        )]),
    );
    let loadout = TestLoadout::Manual(vec![
        TestLoadoutEntry::reference("mount_a", RELAY),
        TestLoadoutEntry::reference("mount_b", RELAY),
        TestLoadoutEntry::reference("deck", ACTOR),
        TestLoadoutEntry::reference("loose", ACTOR),
        TestLoadoutEntry::reference("hangar", DOCKED),
    ]);
    TestEntity::new(
        SHIP,
        "FixtureTopology",
        "libs/foundry/records/entities/spaceships/fixture_topology.xml",
    )
    .component(node(container))
    .component(node(vehicle(source)))
    .loadout(if matches!(source, GraphSource::Loadout) {
        TestLoadout::Xml(LOADOUT_XML.to_owned())
    } else {
        loadout
    })
    .record()
}

fn vehicle(source: GraphSource) -> TestInstance {
    let modification = if matches!(
        source,
        GraphSource::Patch | GraphSource::Elem | GraphSource::ElemWithoutDisplayValue
    ) {
        "variant"
    } else {
        ""
    };
    TestInstance::new(VEHICLE)
        .value("vehicleDefinition", TestValue::text(SHIP_XML))
        .value("modification", TestValue::text(modification))
        .value(
            "objectContainers",
            TestValue::Array(vec![room("mount_a", "bridge"), room("missing", "unused")]),
        )
}

fn item(guid: u128, name: &str, relay: bool, with_ports: bool) -> TestRecord {
    let mut item = TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/scitem/ships/fixture/{name}.xml"),
    )
    .attach_type("Misc")
    .component(node(
        TestInstance::new(RESOURCE)
            .value("isRelay", TestValue::Bool(relay))
            .value("isResourceNetworked", TestValue::Bool(true)),
    ));
    if with_ports {
        item = item.component(node(container(
            vec![
                port("leaf", "relay-leaf", false),
                port("aux", "relay-aux", false),
            ],
            vec![hardpoint("leaf", "aux")],
        )));
    }
    item.record()
}

fn docked() -> TestRecord {
    TestEntity::new(
        DOCKED,
        "FixtureDocked",
        "libs/foundry/records/entities/spaceships/fixture_docked.xml",
    )
    .attach_type("NOITEM_Vehicle")
    .component(node(
        TestInstance::new(VEHICLE)
            .value("vehicleDefinition", TestValue::text(DOCKED_XML))
            .value("modification", TestValue::text("")),
    ))
    .component(node(container(
        vec![port("docked_port", "docked-port", false)],
        vec![hardpoint("docked_port", "docked_port")],
    )))
    .record()
}

fn container(ports: Vec<TestValue>, links: Vec<TestValue>) -> TestInstance {
    TestInstance::new(PORT_CONTAINER)
        .value("Ports", TestValue::Array(ports))
        .value("InternalHardpointLinks", TestValue::Array(links))
}

fn port(name: &str, identity: &str, invisible: bool) -> TestValue {
    node(
        TestInstance::new(PORT)
            .named(identity)
            .value("Name", TestValue::text(name))
            .value("resourceLinkToParent", TestValue::Bool(invisible))
            .value("RequiredPortTags", TestValue::text("fixture-required"))
            .value("MinSize", TestValue::Int32(1))
            .value(
                "Types",
                TestValue::Array(vec![node(
                    TestInstance::new("SItemPortDefTypes")
                        .value("Type", TestValue::text("FixtureDevice"))
                        .value(
                            "SubTypes",
                            TestValue::Array(vec![TestValue::text("FixtureSubtype")]),
                        ),
                )]),
            )
            .value(
                "Flags",
                TestValue::text(if invisible { "invisible" } else { "" }),
            ),
    )
}

fn hardpoint(port1: &str, port2: &str) -> TestValue {
    node(
        TestInstance::new(HARDPOINT_LINK)
            .value("port1", TestValue::text(port1))
            .value("port2", TestValue::text(port2)),
    )
}

fn room(port: &str, bone: &str) -> TestValue {
    node(
        TestInstance::new(ROOM)
            .value("resourceNetworkItemportName", TestValue::text(port))
            .value("boneName", TestValue::text(bone))
            .value("fileName", TestValue::text("data/fixture/room.socpak"))
            .value("guid", TestValue::text("fixture-room-id"))
            .value(
                "shouldRequestNavMeshRegenerationOnLoad",
                TestValue::Bool(true),
            ),
    )
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn weak(name: &str) -> TestType {
    TestType::WeakPointer(name.to_owned())
}
