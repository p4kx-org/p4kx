// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel, TestMtl,
    TestMtlMaterial, TestNode, TestPort, TestRecord, TestStruct, TestType, TestValue,
    entity_schema,
};

/// The material library the sample assembly's gun Entity chooses, replacing the one its model
/// names; green, where the ship's library is red and the library the gun's model names is blue.
pub const ASSEMBLY_GUN_PAINT: &str = "Objects/Test/gun_paint";

const SHIP_LIBRARY: &str = "objects/test/ship";

const IDENTITY: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

const fn at(x: f32, y: f32, z: f32) -> [[f32; 4]; 3] {
    [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]]
}

fn node(name: &str, parent: Option<u16>, local: [[f32; 4]; 3], kind: u16) -> TestNode {
    TestNode {
        name: name.to_owned(),
        parent,
        local,
        kind,
    }
}

fn ship_model() -> TestModel {
    TestModel::cube()
        .with_material_library(SHIP_LIBRARY)
        .with_nodes(vec![
            node("ship", None, IDENTITY, 2),
            node("ship_mesh", Some(0), IDENTITY, 0),
            node("hp_nose", Some(0), at(0.0, 4.0, 0.0), 2),
            node("hardpoint_relay", Some(0), at(3.0, 0.0, 0.0), 2),
        ])
        .with_lod(TestModel::cube().scaled(0.5))
}

fn mount_model() -> TestModel {
    TestModel::cube()
        .with_material_library(SHIP_LIBRARY)
        .scaled(0.25)
        .with_nodes(vec![
            node("mount", None, IDENTITY, 2),
            node("mount_mesh", Some(0), IDENTITY, 0),
            node("hardpoint_class_2", Some(0), at(0.0, 0.0, 0.5), 2),
        ])
}

fn library(colour: [f32; 3]) -> Vec<u8> {
    let material = |name: &str| TestMtlMaterial::new(name).diffuse(colour);
    TestMtl::multi(&[material("unused"), material("caps"), material("sides")]).cryxml()
}

fn model_entries(name: &str, model: &TestModel) -> Vec<TestEntry> {
    let path = format!("Data\\Objects\\Test\\{name}");
    let mut entries = vec![
        TestEntry::stored(format!("{path}.cgf"), model.cgf()),
        TestEntry::stored(format!("{path}.cgfm"), model.cgfm()),
    ];
    if let (Some(cgf), Some(cgfm)) = (model.lod_cgf(1), model.lod_cgfm(1)) {
        entries.push(TestEntry::stored(format!("{path}_lod1.cgf"), cgf));
        entries.push(TestEntry::stored(format!("{path}_lod1.cgfm"), cgfm));
    }
    entries
}

fn database() -> DataForgeBuilder {
    let ship = TestEntity::new(0x1, "TEST_Ship", "entities/spaceships/test_ship.xml")
        .geometry("Objects\\Test\\ship.cgf", None)
        .port(TestPort::new("hardpoint_weapon_nose").helper(
            "hp_nose",
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 90.0],
        ))
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("hardpoint_weapon_nose", "TEST_Mount").loadout(
                TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "hardpoint_class_2",
                    "TEST_Gun",
                )]),
            ),
            TestLoadoutEntry::class("hardpoint_relay", "TEST_Relay"),
            TestLoadoutEntry::class("hardpoint_seat", "TEST_Seat"),
            TestLoadoutEntry::class("hardpoint_door", "TEST_Door"),
            TestLoadoutEntry::reference("hardpoint_cargo", 0x00ff),
        ]));
    let mount = TestEntity::new(0x2, "TEST_Mount", "entities/scitem/test_mount.xml")
        .geometry("Objects/Test/mount.cgf", None)
        .port(TestPort::new("hardpoint_class_2").helper("hardpoint_class_2", [0.0; 3], [0.0; 3]));
    let gun = TestEntity::new(0x3, "TEST_Gun", "entities/scitem/test_gun.xml")
        .geometry("Objects/Test/gun.cgf", Some(ASSEMBLY_GUN_PAINT));
    let relay = TestEntity::new(0x4, "TEST_Relay", "entities/scitem/test_relay.xml")
        .geometry("Objects/Test/gun.cgf", Some(ASSEMBLY_GUN_PAINT));
    let seat = TestEntity::new(0x5, "TEST_Seat", "entities/scitem/test_seat.xml");
    let door = TestEntity::new(0x6, "TEST_Door", "entities/scitem/test_door.xml")
        .geometry("Objects/Test/door.cgf", None);
    [ship, mount, gun, relay, seat, door]
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
}

/// Returns the Entries of a synthetic Archive that holds a ship Entity with an `Assembly`.
///
/// `TEST_Ship` has a model with the helpers `hp_nose` at (0, 4, 0) and `hardpoint_relay` at
/// (3, 0, 0), in Z up, and a second level of detail at half the size. Its Loadout puts
/// `TEST_Mount` into `hardpoint_weapon_nose` (helper `hp_nose`, offset (0, 0, 1) turned 90
/// degrees about Z) with `TEST_Gun` in the mount's `hardpoint_class_2` (helper 0.5 m above the
/// mount's origin), `TEST_Relay` into `hardpoint_relay` (no port definition, so the node named
/// like the port), `TEST_Seat`, an Entity without geometry, `TEST_Door`, whose model the
/// Archive lacks, and a Reference to an Entity that does not exist into `hardpoint_cargo`.
///
/// The ship and the mount name the library `objects/test/ship`, red. The gun and the relay
/// share a model that names `objects/test/cube`, blue, and choose [`ASSEMBLY_GUN_PAINT`], green.
/// Every library holds the sub-materials 0 to 2 and the cubes use 1 and 2.
#[must_use]
pub fn assembly_entries() -> Vec<TestEntry> {
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", database().build())];
    entries.extend(model_entries("ship", &ship_model()));
    entries.extend(model_entries("mount", &mount_model()));
    entries.extend(model_entries("gun", &TestModel::cube().scaled(0.1)));
    entries.push(TestEntry::stored(
        "Data\\objects\\test\\ship.mtl",
        library([0.8, 0.1, 0.1]),
    ));
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\gun_paint.mtl",
        library([0.1, 0.8, 0.1]),
    ));
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\cube.mtl",
        library([0.1, 0.1, 0.8]),
    ));
    entries
}

/// Returns the Entries of [`assembly_entries`] with the camera of the vehicle hologram.
///
/// A `UIHoloVehicle_Config` Record sets the field of view to `camera[0]` degrees, the view angle
/// to `camera[1]` degrees and the distance scaler to `camera[2]`.
#[must_use]
pub fn hologram_entries(camera: [f32; 3]) -> Vec<TestEntry> {
    let config = TestStruct::new("UIHoloVehicle_Config")
        .field("cameraFOV", TestType::Single)
        .field("ownDefaultViewAngle", TestType::Single)
        .field("ownCameraDistanceScaler", TestType::Single);
    let record = TestRecord::new(
        0x7001,
        "UIHoloVehicle_Config",
        "UIHoloVehicle_Config.Vehicle",
        "libs/foundry/records/ui/holo/vehicle_config.xml",
    )
    .value("cameraFOV", TestValue::Single(camera[0]))
    .value("ownDefaultViewAngle", TestValue::Single(camera[1]))
    .value("ownCameraDistanceScaler", TestValue::Single(camera[2]));
    let mut entries = assembly_entries();
    if let Some(first) = entries.first_mut() {
        *first = TestEntry::zstd(
            "Data\\Game2.dcb",
            database().structure(config).record(record).build(),
        );
    }
    entries
}

/// Returns the synthetic assembly with whole-part and per-range material suppression.
#[must_use]
pub fn hologram_suppression_entries() -> Vec<TestEntry> {
    assembly_entries()
        .into_iter()
        .map(|entry| {
            let (first, second) = if entry.path().ends_with("ship.mtl") {
                ("NoDraw", "Decal")
            } else if entry.path().ends_with("gun_paint.mtl") {
                ("Decal", "Illum")
            } else {
                return entry;
            };
            let material = |name, shader| TestMtlMaterial::new(name).shader(shader);
            let bytes = TestMtl::multi(&[
                material("unused", "NoDraw"),
                material("caps", first),
                material("sides", second),
            ])
            .cryxml();
            TestEntry::stored(entry.path(), bytes)
        })
        .collect()
}
