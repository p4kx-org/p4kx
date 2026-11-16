// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Port constraints and static attachment transforms.

use std::error::Error;

use p4kx_assembly::{Assembly, EntityNode, LoadoutResolver, Placement, PortDefinition};
use p4kx_dataforge::DataForge;
use p4kx_extraction::Archive;
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestModel, TestNode, TestPort, entity_schema,
};

use crate::assembly_support::{assemble, fixture};

type TestResult = Result<(), Box<dyn Error>>;

fn resolve(ports: Vec<TestPort>) -> Result<EntityNode, Box<dyn Error>> {
    let ship = ports.into_iter().fold(
        TestEntity::new(0x1000, "TEST_Ship", "entities/test_ship.xml"),
        TestEntity::port,
    );
    let bytes = entity_schema(DataForgeBuilder::new())
        .record(ship.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find("TEST_Ship")?;
    Ok(resolver.resolve(&entity)?)
}

fn definition<'n>(node: &'n EntityNode, port: &str) -> Result<&'n PortDefinition, Box<dyn Error>> {
    node.ports()
        .iter()
        .find(|candidate| candidate.name() == port)
        .and_then(|candidate| candidate.definition())
        .ok_or_else(|| format!("no definition of {port}").into())
}

#[test]
fn the_entity_attachment_flag_is_read_as_stored() -> TestResult {
    let ship = resolve(vec![
        TestPort::new("gimbal").entity_attachment(true),
        TestPort::new("fixed").entity_attachment(false),
        TestPort::new("plain"),
        TestPort::new("bone").helper("hardpoint_bone", [0.0; 3], [0.0; 3]),
    ])?;
    assert!(definition(&ship, "gimbal")?.no_rotation(), "gimbal");
    assert!(!definition(&ship, "fixed")?.no_rotation(), "entity, unset");
    assert!(!definition(&ship, "plain")?.no_rotation(), "plain");
    assert!(!definition(&ship, "bone")?.no_rotation(), "bone");
    Ok(())
}

#[test]
fn the_detach_direction_is_read_as_stored() -> TestResult {
    let ship = resolve(vec![
        TestPort::new("pod").detach_direction([0.0, -1.0, 0.5]),
        TestPort::new("plain"),
    ])?;
    assert_eq!(
        definition(&ship, "pod")?.detach_direction(),
        [0.0, -1.0, 0.5]
    );
    assert_eq!(definition(&ship, "plain")?.detach_direction(), [0.0; 3]);
    Ok(())
}

#[test]
fn the_tags_of_a_port_are_read_as_stored() -> TestResult {
    let ship = resolve(vec![
        TestPort::new("rack").tags("Missile Rack", "$Paint_Base"),
    ])?;
    let rack = definition(&ship, "rack")?;
    assert_eq!(rack.port_tags(), "Missile Rack");
    assert_eq!(rack.required_tags(), "$Paint_Base");
    Ok(())
}

#[test]
fn a_static_attachment_with_no_rotation_takes_the_rotated_port_node() -> TestResult {
    let fuse_guid = 2;
    let relay = TestEntity::new(1, "TEST_Relay", "entities/test_relay.xml")
        .geometry("objects/test/relay.cgf", None)
        .port(
            TestPort::new("slot")
                .entity_attachment(true)
                .default_item(fuse_guid),
        );
    let fuse = TestEntity::new(fuse_guid, "TEST_Fuse", "entities/test_fuse.xml")
        .geometry("objects/test/fuse.cgf", None);
    let forge = entity_schema(DataForgeBuilder::new())
        .record(relay.record())
        .record(fuse.record())
        .build();
    let relay_model = relay_with_rotated_socket();
    let fixture = fixture(vec![
        TestEntry::stored("Data/Game2.dcb", forge),
        TestEntry::stored("Data/objects/test/relay.cgf", relay_model.cgf()),
        TestEntry::stored("Data/objects/test/fuse.cgf", TestModel::cube().cgf()),
        TestEntry::stored("Data/objects/test/fuse.cgfm", TestModel::cube().cgfm()),
    ])?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_Relay", 0)?;
    let attached = assembly
        .part_at("slot")
        .ok_or("attached fuse at static slot")?;
    assert_eq!(
        attached.placement(),
        &Placement::PortNode,
        "the model's slot node supplies the placement"
    );
    assert_eq!(
        attached.world().rows,
        [
            [0.0, -1.0, 0.0, 9.0],
            [0.0, 0.0, -1.0, 7.0],
            [1.0, 0.0, 0.0, 16.0],
        ],
        "the static fuse composes the frame's X rotation with the socket's Z rotation and offset"
    );
    verify_glb_socket(&assembly)
}

fn relay_with_rotated_socket() -> TestModel {
    TestModel::cube()
        .with_nodes(vec![
            TestNode {
                name: "socket frame".into(),
                parent: None,
                local: [
                    [1.0, 0.0, 0.0, 7.0],
                    [0.0, 0.0, -1.0, 11.0],
                    [0.0, 1.0, 0.0, 13.0],
                ],
                kind: 2,
            },
            TestNode {
                name: "slot".into(),
                parent: Some(0),
                local: [
                    [0.0, -1.0, 0.0, 2.0],
                    [1.0, 0.0, 0.0, 3.0],
                    [0.0, 0.0, 1.0, 4.0],
                ],
                kind: 2,
            },
        ])
        .helpers_only()
}

fn verify_glb_socket(assembly: &Assembly) -> TestResult {
    let mut bytes = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
    let glb = p4kx_testkit::check_glb(&bytes)?;
    let relay = glb
        .document
        .nodes()
        .find(|node| node.name() == Some("TEST_Relay"))
        .ok_or("static GLB relay node")?;
    let fuse = relay
        .children()
        .find(|node| node.name() == Some("slot/TEST_Fuse"))
        .ok_or("GLB fuse attached to its relay")?;
    let model_root = fuse
        .children()
        .find(|node| node.name() == Some("cube"))
        .ok_or("the authored fuse model root is below its attachment")?;
    assert!(
        model_root
            .children()
            .any(|node| node.name() == Some("cube_mesh") && node.mesh().is_some()),
        "the attached model retains its drawable geometry"
    );

    // glTF columns are the original Z-up basis and translation expressed in Y-up axes.
    let expected = [
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [9.0, 16.0, -7.0, 1.0],
    ];
    let matrix = fuse.transform().matrix();
    assert!(
        matrix
            .into_iter()
            .flatten()
            .zip(expected.into_iter().flatten())
            .all(|(actual, wanted)| (actual - wanted).abs() < 1e-5),
        "the full socket transform reaches GLB: {matrix:?}"
    );
    Ok(())
}
