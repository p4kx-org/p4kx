// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! An item port of a character definition (`.cdf`) is anchored to a bone of its skeleton.

use crate::assembly_support::{TestResult, assemble, fixture};

use p4kx_assembly::Placement;
use p4kx_extraction::Archive;
use p4kx_testkit::{
    DataForgeBuilder, TestBone, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestNode, TestPort, TestSkeleton, entity_schema,
};

const ARM: &str = "TEST_Arm";

const CHARACTER: &str = r#"<CharacterDefinition>
  <AttachmentList>
    <Attachment Type="CA_SKIN" AName="Arm" Binding="Objects\Test\arm.skin"/>
  </AttachmentList>
</CharacterDefinition>"#;

type PortSpec<'a> = (&'a str, &'a str, [f32; 3]);

fn entries(skin: &TestModel, ports: &[PortSpec<'_>]) -> Vec<TestEntry> {
    let arm =
        TestEntity::new(0x1, ARM, "entities/test_arm.xml").geometry("Objects/Test/arm.cdf", None);
    let arm = ports.iter().fold(arm, |arm, (name, helper, position)| {
        arm.port(TestPort::new(*name).helper(*helper, *position, [0.0; 3]))
    });
    let arm = arm.loadout(TestLoadout::Manual(
        ports
            .iter()
            .map(|(name, _, _)| TestLoadoutEntry::class(*name, "TEST_Laser"))
            .collect(),
    ));
    let laser = TestEntity::new(0x2, "TEST_Laser", "entities/test_laser.xml")
        .geometry("Objects/Test/laser.cgf", None);
    let database = [arm, laser]
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        });
    let laser = TestModel::cube();
    vec![
        TestEntry::zstd("Data\\Game2.dcb", database.build()),
        TestEntry::stored(
            "Data\\Objects\\Test\\arm.cdf",
            CHARACTER.as_bytes().to_vec(),
        ),
        TestEntry::stored("Data\\Objects\\Test\\arm.skin", skin.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\arm.skinm", skin.cgfm()),
        TestEntry::stored("Data\\Objects\\Test\\laser.cgf", laser.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\laser.cgfm", laser.cgfm()),
    ]
}

fn assert_near(actual: [f32; 3], expected: [f32; 3]) {
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-4, "{actual:?} against {expected:?}");
    }
}

#[test]
fn a_bone_of_the_skeleton_anchors_the_item_port() -> TestResult {
    let ports = [
        ("on_tip", "TIP", [0.0; 3]),
        ("nowhere", "missing", [0.0; 3]),
    ];
    let fixture = fixture(entries(&TestModel::two_bone_skin(), &ports))?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, ARM, 0)?;

    let laser = assembly.part_at("on_tip").ok_or("the part")?;
    assert_eq!(laser.placement(), &Placement::Helper("TIP".to_owned()));
    assert_near(laser.world().translation(), [0.0, 0.0, 2.0]);
    Ok(())
}

#[test]
fn the_offset_of_the_port_applies_in_the_space_of_a_rotated_bone() -> TestResult {
    let ports = [("on_tip", "tip", [1.0, 0.0, 0.0])];
    let fixture = fixture(entries(&TestModel::two_bone_skin(), &ports))?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, ARM, 0)?;

    let laser = assembly.part_at("on_tip").ok_or("the part")?;
    assert_near(laser.world().translation(), [0.0, 1.0, 2.0]);
    Ok(())
}

#[test]
fn a_bone_below_a_rotated_chain_composes_every_parent() -> TestResult {
    let skin = TestModel::cube().with_skeleton(&TestSkeleton::chain(3));
    let ports = [("on_last", "bone_2", [0.0; 3])];
    let fixture = fixture(entries(&skin, &ports))?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, ARM, 0)?;

    let laser = assembly.part_at("on_last").ok_or("the part")?;
    let (first_y, first_x) = 30.0_f32.to_radians().sin_cos();
    let (second_y, second_x) = 60.0_f32.to_radians().sin_cos();
    assert_near(
        laser.world().translation(),
        [first_x + second_x, first_y + second_y, 0.0],
    );
    Ok(())
}

#[test]
fn a_node_of_the_model_beats_a_bone_of_the_same_name() -> TestResult {
    let node = |name: &str, x: f32, kind: u16| TestNode {
        name: name.to_owned(),
        parent: None,
        local: [
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        kind,
    };
    let nodes = vec![
        node("root", 0.0, 0),
        node("mesh", 0.0, 0),
        node("TIP", 5.0, 2),
    ];
    let skin = TestModel::two_bone_skin().with_nodes(nodes);
    let fixture = fixture(entries(&skin, &[("on_tip", "tip", [0.0; 3])]))?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, ARM, 0)?;

    let laser = assembly.part_at("on_tip").ok_or("the part")?;
    assert_eq!(laser.placement(), &Placement::Helper("tip".to_owned()));
    assert_near(laser.world().translation(), [5.0, 0.0, 0.0]);
    Ok(())
}

#[test]
fn a_helper_that_is_no_bone_is_still_listed() -> TestResult {
    let ports = [
        ("on_tip", "TIP", [0.0; 3]),
        ("nowhere", "missing", [0.0; 3]),
    ];
    let fixture = fixture(entries(&TestModel::two_bone_skin(), &ports))?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, ARM, 0)?;

    let part = assembly.part_at("nowhere").ok_or("the part")?;
    assert_eq!(part.placement(), &Placement::Origin);
    let helpers: Vec<&str> = assembly
        .issues()
        .iter()
        .filter(|issue| issue.kind().code() == "helper_not_found")
        .map(p4kx_assembly::Issue::path)
        .collect();
    assert_eq!(helpers, ["nowhere"]);
    Ok(())
}

fn bone(name: &str, parent: Option<u16>, rotation: [f32; 4]) -> TestBone {
    TestBone {
        name: name.to_owned(),
        parent,
        rotation,
        translation: [0.0, 0.0, 1.0],
    }
}

#[test]
fn a_damaged_skeleton_never_panics() -> TestResult {
    let identity = [0.0, 0.0, 0.0, 1.0];
    let skeletons = [
        vec![bone("a", Some(1), identity), bone("b", Some(0), identity)],
        vec![bone("a", None, identity), bone("b", Some(9), identity)],
        vec![bone("a", None, identity), bone("b", Some(0), [f32::NAN; 4])],
    ];
    for bones in skeletons {
        let skin = TestModel::cube().with_skeleton(&TestSkeleton::new(bones));
        let ports = [("on_b", "b", [0.0; 3])];
        let fixture = fixture(entries(&skin, &ports))?;
        let archive = Archive::open(&fixture.path)?;

        let assembly = assemble(&archive, ARM, 0)?;

        assert!(assembly.part_at("on_b").is_some());
    }
    Ok(())
}
