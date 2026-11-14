// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Interior of an Entity in a package: containers, objects and lights as nodes of the
//! manifest, with the lights described and the models shared as meshes.

use std::path::PathBuf;

use p4kx_assembly::{Interior, InteriorOptions};
use p4kx_blender::{
    InteriorScene, LightKind, Manifest, Node, NodeKind, PackageInfo, PackageSummary,
    manifest_schema, write_interior_package, write_package,
};
use p4kx_extraction::{Archive, select_entries};
use p4kx_testkit::{ENTITY_LIT_ROOM_CLASS, LIT_ROOM, check_glb, entity_lit_room_entries};

use crate::support::{
    TestResult, assemble, assemble_with_interior, fixture, glb_options, names_in, never,
};

struct Written {
    _dir: tempfile::TempDir,
    out: PathBuf,
    manifest: Manifest,
    raw: serde_json::Value,
    summary: PackageSummary,
}

fn write(interior: bool) -> Result<Written, Box<dyn std::error::Error>> {
    let fixture = fixture(entity_lit_room_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = if interior {
        assemble_with_interior(&archive, ENTITY_LIT_ROOM_CLASS)?
    } else {
        assemble(&archive, ENTITY_LIT_ROOM_CLASS)?
    };
    let models = p4kx_assembly::ArchiveModels::new(&archive);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship");
    let summary = write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::new(None, None),
        &out,
        &never,
    )?;
    let text = std::fs::read_to_string(out.join("scene.json"))?;
    Ok(Written {
        manifest: serde_json::from_str(&text)?,
        raw: serde_json::from_str(&text)?,
        _dir: dir,
        out,
        summary,
    })
}

fn named<'a>(manifest: &'a Manifest, name: &str) -> Result<&'a Node, String> {
    manifest
        .nodes
        .iter()
        .find(|node| node.name == name)
        .ok_or_else(|| format!("no node {name}"))
}

fn minus_z(rotation: [f32; 4]) -> [f32; 3] {
    let [qx, qy, qz, qw] = rotation;
    [
        -2.0 * qx.mul_add(qz, qw * qy),
        -2.0 * qy.mul_add(qz, -(qw * qx)),
        2.0f32.mul_add(qx.mul_add(qx, qy * qy), -1.0),
    ]
}

fn has_identity_ancestors(manifest: &Manifest, node: &Node) -> bool {
    let mut parent = node.parent;
    while let Some(index) = parent
        && let Some(above) = manifest.nodes.get(index)
    {
        if (above.rotation.iter().map(|q| q.abs()).sum::<f32>() - 1.0).abs() > 1e-5 {
            return false;
        }
        parent = above.parent;
    }
    true
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn without_the_interior_the_package_has_the_hull_alone() -> TestResult {
    let written = write(false)?;

    assert_eq!(written.manifest.nodes.len(), 1);
    assert_eq!(written.summary.lights, 0);
    assert_eq!(written.summary.containers, 0);
    Ok(())
}

#[test]
fn the_interior_hangs_below_the_hull_in_containers_objects_and_lights() -> TestResult {
    let written = write(true)?;
    let nodes = &written.manifest.nodes;

    let group = named(&written.manifest, "Interior")?;
    assert_eq!(group.kind, NodeKind::Container);
    assert_eq!(group.parent, Some(0));
    assert_eq!(group.path, "Interior");
    assert!(group.entity.is_none() && group.mesh.is_none() && group.light.is_none());
    assert!(
        nodes
            .iter()
            .enumerate()
            .all(|(index, node)| node.parent.is_none_or(|parent| parent < index)),
        "a node comes after its parent"
    );
    let count = |kind: NodeKind| nodes.iter().filter(|node| node.kind == kind).count();
    assert_eq!(count(NodeKind::Entity), 1);
    assert_eq!(
        count(NodeKind::Light),
        5,
        "lamp, beam, sun and the group's two"
    );
    assert_eq!(count(NodeKind::Object), 2, "stool and ghost");
    assert_eq!(written.summary.lights, 5);
    assert_eq!(written.summary.objects, 2);
    assert_eq!(written.summary.containers, count(NodeKind::Container));
    let room = named(&written.manifest, "lit_room")?;
    assert_eq!(room.kind, NodeKind::Container);
    assert_eq!(room.path, "Interior/lit_room");
    assert_eq!(
        room.parent,
        nodes.iter().position(|node| node.name == "Interior")
    );
    let lamp = named(&written.manifest, "lamp")?;
    assert!(
        lamp.path.starts_with("Interior/lit_room/"),
        "the path names the chain: {}",
        lamp.path
    );
    assert!(
        room.translation.iter().any(|axis| axis.abs() > 1.0),
        "the room sits at the bone, not at the origin: {:?}",
        room.translation
    );
    Ok(())
}

#[test]
fn lights_carry_their_type_colour_intensity_and_range() -> TestResult {
    let written = write(true)?;

    let lamp = named(&written.manifest, "lamp")?
        .light
        .ok_or("the lamp has no light")?;
    assert_eq!(lamp.kind, LightKind::Point);
    assert!((lamp.intensity - 3.0).abs() < 1e-6, "raw engine value");
    assert_eq!(lamp.range, Some(6.0));
    assert!(lamp.inner_cone.is_none() && lamp.outer_cone.is_none());
    assert!(
        lamp.colour[0] >= lamp.colour[1] && lamp.colour[1] >= lamp.colour[2],
        "warm: {:?}",
        lamp.colour
    );
    assert!(lamp.colour.iter().all(|c| (0.0..=1.0).contains(c)));
    let beam = named(&written.manifest, "beam")?
        .light
        .ok_or("the beam has no light")?;
    assert_eq!(beam.kind, LightKind::Spot);
    let outer = beam.outer_cone.ok_or("no outer cone")?;
    assert!(
        (outer - 30.0_f32.to_radians()).abs() < 1e-5,
        "half of 60 degrees: {outer}"
    );
    assert_eq!(beam.inner_cone, Some(0.0));
    let sun = named(&written.manifest, "sun")?
        .light
        .ok_or("the sun has no light")?;
    assert_eq!(sun.kind, LightKind::Directional);
    let soft = named(&written.manifest, "Light-A")?
        .light
        .ok_or("no group light")?;
    assert_eq!(soft.kind, LightKind::Point);
    Ok(())
}

#[test]
fn unsupported_light_types_are_no_nodes() -> TestResult {
    let written = write(true)?;

    for name in ["haze", "panel_1", "panel_2"] {
        assert!(
            written.manifest.nodes.iter().all(|node| node.name != name),
            "{name} is left out"
        );
    }
    assert!(
        written
            .manifest
            .issues
            .iter()
            .any(|issue| issue.kind == "light_unsupported"),
        "and reported"
    );
    Ok(())
}

#[test]
fn a_light_shines_along_its_nodes_minus_z() -> TestResult {
    let written = write(true)?;

    let sun = named(&written.manifest, "sun")?;
    assert!(
        has_identity_ancestors(&written.manifest, sun),
        "the hull and the room are not turned"
    );
    let direction = minus_z(sun.rotation);
    assert!(
        close(direction, [1.0, 0.0, 0.0]),
        "the engine's +X, which glTF keeps as +X: {direction:?}"
    );
    Ok(())
}

#[test]
fn objects_draw_shared_meshes_and_lights_draw_nothing() -> TestResult {
    let written = write(true)?;
    let manifest = &written.manifest;

    let stool = named(manifest, "stool")?;
    let id = stool.mesh.as_deref().ok_or("the stool draws no mesh")?;
    assert_eq!(stool.kind, NodeKind::Object);
    assert!(stool.entity.is_none());
    let mesh = manifest
        .meshes
        .iter()
        .find(|mesh| mesh.id == id)
        .ok_or("no mesh")?;
    assert_eq!(mesh.instances, 1);
    assert!(
        mesh.geometry
            .as_deref()
            .is_some_and(|path| path.contains("cube"))
    );
    assert!(
        manifest
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Light)
            .all(|node| node.mesh.is_none())
    );
    assert!(written.out.join(&mesh.file).is_file());
    assert_eq!(manifest.meshes.len(), written.summary.meshes);
    let ghost = named(manifest, "ghost")?;
    assert_eq!(ghost.kind, NodeKind::Object);
    assert!(ghost.mesh.is_none(), "the missing model draws nothing");
    Ok(())
}

#[test]
fn the_folder_holds_only_the_manifest_and_the_files_it_lists() -> TestResult {
    let written = write(true)?;

    assert_eq!(
        names_in(&written.out)?,
        ["meshes", "receipt.json", "scene.json", "textures"]
    );
    let meshes = names_in(&written.out.join("meshes"))?;
    assert_eq!(meshes.len(), written.manifest.meshes.len());
    for mesh in &written.manifest.meshes {
        let bytes = std::fs::read(written.out.join(&mesh.file))?;
        check_glb(&bytes)?;
    }
    Ok(())
}

#[test]
fn the_manifest_with_an_interior_matches_its_schema() -> TestResult {
    let written = write(true)?;
    let validator = jsonschema::draft202012::new(&manifest_schema())?;

    let violations: Vec<String> = validator
        .iter_errors(&written.raw)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();

    assert!(violations.is_empty(), "{violations:?}");
    let lamp = written
        .raw
        .pointer("/nodes")
        .and_then(serde_json::Value::as_array)
        .and_then(|nodes| nodes.iter().find(|node| node["name"] == "lamp"))
        .ok_or("no lamp")?;
    assert_eq!(lamp["kind"], "light");
    assert_eq!(lamp["light"]["kind"], "point");
    assert!(lamp.get("entity").is_none(), "no nulls, no entity");
    Ok(())
}

fn write_alone() -> Result<Written, Box<dyn std::error::Error>> {
    let fixture = fixture(entity_lit_room_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let found = select_entries(&archive, &[LIT_ROOM])?;
    let entry = found.first().ok_or("no container")?;
    let opened = archive.open_container(entry)?;
    let interior = Interior::build(
        &[&archive, &opened],
        entry.path(),
        &InteriorOptions::default(),
    )?;
    let models = p4kx_assembly::ArchiveModels::new(&archive);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("room");
    let summary = write_interior_package(
        &InteriorScene::new("lit_room", &interior),
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::new(None, None),
        &out,
        &never,
    )?;
    let text = std::fs::read_to_string(out.join("scene.json"))?;
    Ok(Written {
        manifest: serde_json::from_str(&text)?,
        raw: serde_json::from_str(&text)?,
        _dir: dir,
        out,
        summary,
    })
}

fn below(manifest: &Manifest, root: &str) -> Vec<String> {
    let prefix = format!("{root}/");
    let mut found: Vec<String> = manifest
        .nodes
        .iter()
        .filter_map(|node| {
            let relative = node.path.strip_prefix(&prefix)?;
            let geometry = node.mesh.as_deref().and_then(|id| {
                manifest
                    .meshes
                    .iter()
                    .find(|mesh| mesh.id == id)
                    .map(|mesh| mesh.geometry.clone())
            });
            Some(format!(
                "{relative} {} {:?} {:?} {:?} {:?} {geometry:?} {:?}",
                node.name, node.kind, node.translation, node.rotation, node.scale, node.light
            ))
        })
        .collect();
    found.sort();
    found
}

#[test]
fn an_interior_on_its_own_has_the_subtree_of_the_entitys_container() -> TestResult {
    let entity = write(true)?;
    let alone = write_alone()?;

    let inside = below(&entity.manifest, "Interior/lit_room");
    let own = below(&alone.manifest, "lit_room");

    assert!(own.len() >= 8, "{own:?}");
    assert_eq!(own, inside);
    let root = alone.manifest.nodes.first().ok_or("no root")?;
    assert_eq!(root.kind, NodeKind::Container);
    assert_eq!(root.name, "lit_room");
    assert_eq!(root.path, "lit_room");
    assert!(root.parent.is_none());
    assert_eq!(root.translation, [0.0; 3]);
    assert!(
        alone
            .manifest
            .nodes
            .iter()
            .all(|node| node.kind != NodeKind::Entity)
    );
    assert_eq!(alone.manifest.source.entity.class_name, "lit_room");
    assert_eq!(alone.manifest.source.entity.guid, "");
    assert_eq!(alone.summary.lights, entity.summary.lights);
    assert_eq!(alone.summary.objects, entity.summary.objects);
    let validator = jsonschema::draft202012::new(&manifest_schema())?;
    assert!(validator.is_valid(&alone.raw));
    for mesh in &alone.manifest.meshes {
        check_glb(&std::fs::read(alone.out.join(&mesh.file))?)?;
    }
    Ok(())
}
