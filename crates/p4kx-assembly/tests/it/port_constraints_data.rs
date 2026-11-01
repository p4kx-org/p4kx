// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A port definition carries the data placement and fit need: the tags, the detach direction and
//! whether the item keeps its own orientation.

use std::error::Error;

use p4kx_assembly::{EntityNode, LoadoutResolver, PortDefinition};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestPort, entity_schema};

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
fn a_gimbal_port_keeps_its_orientation_and_a_plain_port_does_not() -> TestResult {
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
