// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The bone attachments of a character definition (`.cdf`) are drawn at their bones.

#[path = "character_drawable_body.rs"]
mod drawable_body;

use crate::assembly_support::{TestResult, assemble, assemble_with, fixture};

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, IssueKind, ModelProvider};
use p4kx_extraction::Archive;
use p4kx_gltf::GlbOptions;
use p4kx_testkit::gltf::Node;
use p4kx_testkit::{
    BONE_THRUSTER as THRUSTER, bone_thruster_definition, bone_thruster_entries,
    bone_thruster_housing, check_glb,
};

fn built(definition: &str) -> Result<Assembly, Box<dyn std::error::Error>> {
    let fixture = fixture(bone_thruster_entries(definition))?;
    let archive = Archive::open(&fixture.path)?;
    assemble(&archive, THRUSTER, 0)
}

fn assert_near(actual: [f32; 3], expected: [f32; 3]) {
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-4, "{actual:?} against {expected:?}");
    }
}

fn draws(node: &Node<'_>) -> bool {
    node.mesh().is_some() || node.children().any(|child| draws(&child))
}

#[test]
fn a_bone_attachment_sits_at_its_bone_offset_by_the_attachment() -> TestResult {
    let assembly = built(&bone_thruster_definition(&bone_thruster_housing(
        "tip",
        r"Objects\Test\housing.cgf",
    )))?;

    let root = assembly.root().ok_or("a root")?;
    let [attached] = root.attachments() else {
        return Err(format!("one attachment: {:?}", assembly.issues()).into());
    };
    assert_eq!(attached.name(), "Housing");
    assert_eq!(attached.bone(), "tip");
    assert!(attached.is_bone_found());
    assert_near(attached.local().translation(), [0.0, 1.0, 2.0]);
    assert_eq!(
        attached
            .materials()
            .library()
            .map(|library| library.path.as_str()),
        Some("Objects/Test/paint"),
        "the attachment's own material library"
    );
    assert!(assembly.issues().is_empty(), "{:?}", assembly.issues());
    Ok(())
}

#[test]
fn a_bone_the_skeleton_lacks_leaves_the_attachment_at_the_origin_with_a_note() -> TestResult {
    let assembly = built(&bone_thruster_definition(&bone_thruster_housing(
        "nowhere",
        r"Objects\Test\housing.cgf",
    )))?;

    let root = assembly.root().ok_or("a root")?;
    let [attached] = root.attachments() else {
        return Err("one attachment".into());
    };
    assert!(!attached.is_bone_found());
    assert_near(attached.local().translation(), [1.0, 0.0, 0.0]);
    let [issue] = assembly.issues() else {
        return Err(format!("one issue: {:?}", assembly.issues()).into());
    };
    assert!(matches!(
        issue.kind(),
        IssueKind::HelperNotFound { helper } if helper == "nowhere"
    ));
    Ok(())
}

#[test]
fn a_file_that_is_not_in_the_archive_is_listed_as_left_out() -> TestResult {
    let assembly = built(&bone_thruster_definition(&bone_thruster_housing(
        "tip",
        r"Objects\Test\gone.cgf",
    )))?;

    let root = assembly.root().ok_or("a root")?;
    assert!(root.attachments().is_empty());
    let [issue] = assembly.issues() else {
        return Err(format!("one issue: {:?}", assembly.issues()).into());
    };
    assert!(matches!(
        issue.kind(),
        IssueKind::AttachmentsLeftOut { files } if files == &["Objects/Test/gone.cgf"]
    ));
    Ok(())
}

#[test]
fn the_vertices_of_the_attachments_count_against_the_budget() -> TestResult {
    let fixture = fixture(bone_thruster_entries(&bone_thruster_definition(
        &bone_thruster_housing("tip", r"Objects\Test\housing.cgf"),
    )))?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble_with(
        &archive,
        THRUSTER,
        &AssemblyOptions::default().with_limits(10, 30),
    )?;

    let root = assembly.root().ok_or("a root")?;
    assert!(root.model().is_none(), "24 + 24 vertices do not fit in 30");
    assert!(root.attachments().is_empty());
    assert!(
        assembly
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), IssueKind::TooManyVertices { limit: 30 })),
        "{:?}",
        assembly.issues()
    );
    Ok(())
}

#[test]
fn the_glb_holds_the_attachment_as_a_child_of_its_part() -> TestResult {
    let assembly = built(&bone_thruster_definition(&bone_thruster_housing(
        "tip",
        r"Objects\Test\housing.cgf",
    )))?;
    let mut bytes = Vec::new();

    assembly.write_glb(&GlbOptions::default(), &mut bytes)?;

    let glb = check_glb(&bytes)?;
    let nodes: Vec<_> = glb.document.nodes().collect();
    let root = nodes
        .iter()
        .find(|node| node.name() == Some(THRUSTER))
        .ok_or("the part node")?;
    let housing = root
        .children()
        .find(|child| child.name() == Some("Housing"))
        .ok_or("the attachment below the part")?;
    assert_near(housing.transform().decomposed().0, [0.0, 2.0, -1.0]);
    assert!(draws(&housing), "the attachment's model is below its node");
    assert!(draws(root), "the skin keeps its own mesh");
    Ok(())
}

#[test]
fn a_number_that_overflows_single_precision_leaves_the_attachment_out() -> TestResult {
    let text =
        bone_thruster_housing("tip", r"Objects\Test\housing.cgf").replace("1,0,0\"", "1e40,0,0\"");
    let assembly = built(&bone_thruster_definition(&text))?;

    let root = assembly.root().ok_or("a root")?;
    assert!(root.attachments().is_empty());
    assert!(assembly.issues().iter().any(|issue| matches!(
        issue.kind(),
        IssueKind::AttachmentsLeftOut { files } if files == &["Objects/Test/housing.cgf"]
    )));
    Ok(())
}

#[test]
fn attachments_beyond_the_vertex_cap_of_the_definition_are_not_read_further() -> TestResult {
    let many = (0..3)
        .map(|_| bone_thruster_housing("tip", r"Objects\Test\housing.cgf"))
        .collect::<String>();
    let fixture = fixture(bone_thruster_entries(&bone_thruster_definition(&many)))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive).with_max_attachment_vertices(60);

    let provided = models.model("Objects/Test/thruster.cdf", 0)?;

    assert_eq!(
        provided.attachments.len(),
        1,
        "24 + 24 vertices fit in 60, a third model of 24 does not"
    );
    assert_eq!(provided.left_out.len(), 2);
    Ok(())
}
