// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Scenes of placed models written as one GLB.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{GlbOptions, GltfError, Scene, SceneNode, ScenePart, write_scene_glb};
use p4kx_model::{
    CompanionError, CompanionPath, LodSelection, MaterialReading, MaterialStatus, Model,
    PositionBox, Transform,
};
use p4kx_testkit::gltf::scene as gltf_node;
use p4kx_testkit::{CheckedGlb, TestModel, TestMtl, TestMtlMaterial, check_glb};

#[path = "scene_mesh_instances.rs"]
mod mesh_instances;

type TestResult = Result<(), Box<dyn Error>>;

const LIBRARY: &str = "objects/test/cube";

fn companions(
    model: &TestModel,
    mtl: Option<Vec<u8>>,
) -> impl FnMut(CompanionPath<'_>) -> Result<Option<Vec<u8>>, CompanionError> + use<> {
    let cgfm = model.cgfm();
    let lod1 = model.lod_cgfm(1);
    move |path| {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            CompanionPath::Sibling("cube_lod1.cgfm") => lod1.clone(),
            CompanionPath::Material(LIBRARY) => mtl.clone(),
            _ => None,
        })
    }
}

fn read(model: &TestModel) -> Result<Model, Box<dyn Error>> {
    Ok(Model::read_with_materials(
        "cube.cgf",
        &ChunkFile::parse(model.cgf())?,
        &mut companions(model, None),
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?)
}

fn library(colour: [f32; 3]) -> MaterialStatus {
    let caps = TestMtlMaterial::new("caps").diffuse(colour);
    let sides = TestMtlMaterial::new("sides").diffuse(colour);
    let file = TestMtl::multi(&[TestMtlMaterial::new("unused"), caps, sides]).cryxml();
    MaterialStatus::load(
        Some(LIBRARY),
        &mut companions(&TestModel::cube(), Some(file)),
    )
}

const fn translation(x: f32, y: f32, z: f32) -> Transform {
    Transform {
        rows: [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]],
    }
}

fn write(
    scene: &Scene<'_>,
    options: GlbOptions,
) -> Result<(CheckedGlb, p4kx_gltf::GlbSummary), Box<dyn Error>> {
    let mut bytes = Vec::new();
    let summary = write_scene_glb(scene, &options, &mut bytes)?;
    Ok((check_glb(&bytes)?, summary))
}

fn node_named<'a>(glb: &'a CheckedGlb, name: &str) -> Result<gltf_node::Node<'a>, Box<dyn Error>> {
    glb.document
        .nodes()
        .find(|node| node.name() == Some(name))
        .ok_or_else(|| format!("no node {name}").into())
}

#[test]
fn nodes_keep_their_names_hierarchy_and_transforms() -> TestResult {
    let model = read(&TestModel::cube())?;
    let green = library([0.1, 0.8, 0.1]);
    let mut scene = Scene::new();
    let root = scene.push(SceneNode::new("TEST_Ship", None, Transform::IDENTITY))?;
    scene.push(
        SceneNode::new(
            "hardpoint_left/TEST_Gun",
            Some(root),
            translation(1.0, 2.0, 3.0),
        )
        .with_part(ScenePart::new(&model, &green)),
    )?;

    let (glb, summary) = write(&scene, GlbOptions::default())?;

    let gun = node_named(&glb, "hardpoint_left/TEST_Gun")?;
    assert_eq!(
        gun.transform().decomposed().0,
        [1.0, 3.0, -2.0],
        "Y-up translation"
    );
    let ship = node_named(&glb, "TEST_Ship")?;
    assert_eq!(ship.children().count(), 1, "the gun hangs below the ship");
    assert_eq!(
        gun.children().count(),
        1,
        "the model's root hangs below the gun"
    );
    assert_eq!(summary.parts, 1);
    assert_eq!(summary.nodes, 4, "ship, gun and the two nodes of the cube");
    Ok(())
}

fn json_of(bytes: &[u8]) -> Result<serde_json::Value, Box<dyn Error>> {
    let length = u32::from_le_bytes(bytes.get(12..16).ok_or("header")?.try_into()?) as usize;
    let json = bytes.get(20..20 + length).ok_or("JSON chunk")?;
    Ok(serde_json::from_slice(json)?)
}

#[test]
fn material_indices_add_only_the_extras() -> TestResult {
    let model = read(&TestModel::cube())?;
    let green = library([0.1, 0.8, 0.1]);
    let mut scene = Scene::new();
    scene.push(
        SceneNode::new("a", None, Transform::IDENTITY).with_part(ScenePart::new(&model, &green)),
    )?;
    let mut plain = Vec::new();
    write_scene_glb(&scene, &GlbOptions::default(), &mut plain)?;
    let mut indexed = Vec::new();
    write_scene_glb(
        &scene,
        &GlbOptions::default().with_material_indices(),
        &mut indexed,
    )?;
    check_glb(&indexed)?;

    let mut with = json_of(&indexed)?;
    let without = json_of(&plain)?;
    let materials = with["materials"].as_array_mut().ok_or("materials")?;
    assert_ne!(materials.len(), 0);
    for (index, material) in materials.iter_mut().enumerate() {
        let object = material.as_object_mut().ok_or("material")?;
        let extras = object.remove("extras").ok_or("extras")?;
        assert_eq!(extras["p4kx_index"], index);
    }
    assert_eq!(with, without, "nothing but the extras differs");
    assert!(
        !without.to_string().contains("\"extras\""),
        "the plain file has no extras anywhere"
    );
    Ok(())
}

#[test]
fn parts_with_equal_materials_share_them() -> TestResult {
    let model = read(&TestModel::cube())?;
    let first = library([0.1, 0.8, 0.1]);
    let second = library([0.1, 0.8, 0.1]);
    let other = library([0.9, 0.1, 0.1]);
    let mut scene = Scene::new();
    for (name, status) in [("a", &first), ("b", &second), ("c", &other)] {
        scene.push(
            SceneNode::new(name, None, Transform::IDENTITY)
                .with_part(ScenePart::new(&model, status)),
        )?;
    }

    let (glb, summary) = write(&scene, GlbOptions::default())?;

    assert_eq!(summary.parts, 3);
    assert_eq!(
        summary.materials, 4,
        "two colours of two sub-materials, not six"
    );
    assert_eq!(glb.document.materials().len(), 4);
    Ok(())
}

#[test]
fn repeated_parts_share_one_mesh_and_keep_their_nodes() -> TestResult {
    let model = read(&TestModel::cube())?;
    let green = library([0.1, 0.8, 0.1]);
    let mut scene = Scene::new();
    for (name, x) in [("a", 0.0), ("b", 2.0), ("c", 4.0)] {
        scene.push(
            SceneNode::new(name, None, translation(x, 0.0, 0.0))
                .with_part(ScenePart::new(&model, &green)),
        )?;
    }
    let mut single = Scene::new();
    single.push(
        SceneNode::new("a", None, Transform::IDENTITY).with_part(ScenePart::new(&model, &green)),
    )?;

    let (glb, summary) = write(&scene, GlbOptions::default())?;
    let (_, one) = write(&single, GlbOptions::default())?;

    assert_eq!(summary.parts, 3);
    assert_eq!(
        summary.nodes, 9,
        "three wrappers and the cube's two nodes each"
    );
    assert_eq!(summary.meshes, one.meshes, "one mesh however many parts");
    assert_eq!(summary.shared_meshes, 2 * one.meshes);
    assert_eq!(summary.vertices, one.vertices);
    assert_eq!(summary.materials, one.materials);
    assert!(summary.bytes < one.bytes + 2 * 1024, "{summary:?}");
    let meshed: Vec<_> = glb
        .document
        .nodes()
        .filter(|node| node.mesh().is_some())
        .collect();
    assert_eq!(meshed.len(), 3 * one.meshes);
    let first = meshed
        .first()
        .and_then(p4kx_testkit::gltf::Node::mesh)
        .map(|mesh| mesh.index());
    assert!(
        meshed
            .iter()
            .all(|node| node.mesh().map(|mesh| mesh.index()) == first),
        "every node uses the same mesh"
    );
    for (name, x) in [("a", 0.0), ("b", 2.0), ("c", 4.0)] {
        let wrapper = node_named(&glb, name)?;
        assert_eq!(wrapper.transform().decomposed().0, [x, 0.0, 0.0], "{name}");
    }
    Ok(())
}

#[test]
fn the_same_model_with_another_library_gets_its_own_mesh() -> TestResult {
    let model = read(&TestModel::cube())?;
    let green = library([0.1, 0.8, 0.1]);
    let red = library([0.9, 0.1, 0.1]);
    let mut scene = Scene::new();
    for (name, status) in [("a", &green), ("b", &red)] {
        scene.push(
            SceneNode::new(name, None, Transform::IDENTITY)
                .with_part(ScenePart::new(&model, status)),
        )?;
    }

    let (_, summary) = write(&scene, GlbOptions::default())?;

    assert_eq!(summary.shared_meshes, 0);
    assert_eq!(summary.shared_materials, 0);
    Ok(())
}

#[test]
fn the_library_of_the_scene_replaces_the_one_of_the_model() -> TestResult {
    let model = read(&TestModel::cube())?;
    assert!(
        matches!(model.materials, MaterialStatus::Skipped),
        "the model was read without its library"
    );
    let green = library([0.1, 0.8, 0.1]);
    let mut scene = Scene::new();
    scene.push(
        SceneNode::new("a", None, Transform::IDENTITY).with_part(ScenePart::new(&model, &green)),
    )?;

    let (glb, _) = write(&scene, GlbOptions::default())?;

    let colours: Vec<[f32; 4]> = glb
        .document
        .materials()
        .map(|material| material.pbr_metallic_roughness().base_color_factor())
        .collect();
    assert_eq!(colours.len(), 2);
    assert!(
        colours.iter().all(|colour| (colour[1] - 0.8).abs() < 1e-5),
        "{colours:?}"
    );
    Ok(())
}

#[test]
fn a_model_without_the_level_is_drawn_at_the_nearest_lower_one() -> TestResult {
    let with_lod = TestModel::cube().with_lod(TestModel::cube().scaled(0.5));
    let detailed = read(&with_lod)?;
    let plain = read(&TestModel::cube())?;
    let status = MaterialStatus::Unnamed;
    let mut scene = Scene::new();
    for (name, model) in [("a", &detailed), ("b", &plain)] {
        scene.push(
            SceneNode::new(name, None, Transform::IDENTITY)
                .with_part(ScenePart::new(model, &status)),
        )?;
    }

    let (_, summary) = write(&scene, GlbOptions::default().with_lod(1))?;

    assert_eq!(summary.parts, 2);
    assert_eq!(summary.lod_fallbacks, 1, "the model without LOD1 uses LOD0");
    Ok(())
}

#[test]
fn a_node_may_not_name_a_later_parent() {
    let mut scene = Scene::new();
    let error = scene
        .push(SceneNode::new("a", Some(0), Transform::IDENTITY))
        .err();
    assert!(
        matches!(
            error,
            Some(GltfError::InvalidSceneNode {
                index: 0,
                parent: 0,
                ..
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_scene_without_models_is_a_valid_file_of_empty_nodes() -> TestResult {
    let mut scene = Scene::new();
    let root = scene.push(SceneNode::new("empty", None, Transform::IDENTITY))?;
    scene.push(SceneNode::new(
        "child",
        Some(root),
        translation(0.0, 1.0, 0.0),
    ))?;

    let (glb, summary) = write(&scene, GlbOptions::default())?;

    assert_eq!(glb.document.nodes().len(), 2);
    assert_eq!((summary.parts, summary.meshes), (0, 0));
    Ok(())
}
