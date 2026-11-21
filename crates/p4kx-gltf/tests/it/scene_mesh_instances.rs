// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_gltf::{GlbOptions, Scene, SceneNode, ScenePart};
use p4kx_model::{MaterialStatus, Transform};
use p4kx_testkit::gltf::scene::Node;
use p4kx_testkit::{TestModel, TestMtl, TestMtlMaterial, TestNode, TestSubmesh};

use super::{LIBRARY, TestResult, companions, node_named, read, translation, write};

const NO_DRAW: u32 = 0x0400;

#[test]
fn repeated_multipart_mounts_keep_their_own_meshes_and_drop_empty_primitives() -> TestResult {
    let weapon = read(&multipart((3, 6)))?;
    let other = read(&multipart((6, 3)))?;
    let materials = materials();
    let mut scene = Scene::new();
    let hull = scene.push(SceneNode::new("hull", None, Transform::IDENTITY))?;
    for (name, x, model) in [
        ("left", -2.0, &weapon),
        ("centre", 0.0, &other),
        ("right", 2.0, &weapon),
    ] {
        scene.push(
            SceneNode::new(name, Some(hull), translation(x, 0.0, 0.0))
                .with_part(ScenePart::new(model, &materials)),
        )?;
    }
    let (glb, summary) = write(&scene, GlbOptions::default())?;
    assert_eq!(
        summary.meshes, 4,
        "each of two distinct models owns two meshes"
    );
    assert_eq!(summary.shared_meshes, 2, "only duplicates share each mesh");
    let left = mount_meshes(&node_named(&glb, "left")?, [3, 6])?;
    let centre = mount_meshes(&node_named(&glb, "centre")?, [6, 3])?;
    let right = node_named(&glb, "right")?;
    assert_eq!(
        right.transform().decomposed().0,
        [2.0, 0.0, 0.0],
        "mesh sharing keeps the second mount at its own translation"
    );
    assert_eq!(mount_meshes(&right, [3, 6])?, left, "reuse by model node");
    assert_ne!(left[0], left[1], "body and barrel have distinct meshes");
    assert!(
        centre.into_iter().all(|index| !left.contains(&index)),
        "the middle placement has identical node names and different geometry"
    );
    Ok(())
}

fn multipart((body_indices, barrel_indices): (u32, u32)) -> TestModel {
    let nodes = ["model", "body", "barrel", "hidden"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| TestNode {
            name: name.to_owned(),
            parent: (index != 0).then_some(0),
            local: Transform::IDENTITY.rows,
            kind: if index == 0 { 2 } else { 0 },
        })
        .collect();
    let row = |node, first_index, index_count| TestSubmesh {
        material_id: node,
        node,
        first_index,
        index_count,
        first_vertex: 0,
        vertex_count: 24,
    };
    TestModel::cube().with_nodes(nodes).with_submeshes(vec![
        row(2, body_indices, barrel_indices),
        row(3, body_indices + barrel_indices, 3),
        row(1, 0, body_indices),
    ])
}

fn materials() -> MaterialStatus {
    let file = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("body"),
        TestMtlMaterial::new("barrel"),
        TestMtlMaterial::new("hidden").flags(NO_DRAW),
    ])
    .cryxml();
    MaterialStatus::load(
        Some(LIBRARY),
        &mut companions(&TestModel::cube(), Some(file)),
    )
}

fn mount_meshes(
    mount: &Node<'_>,
    [body, barrel]: [usize; 2],
) -> Result<[usize; 2], Box<dyn Error>> {
    let model = mount.children().next().ok_or("mounted model root")?;
    let node = |name| {
        model
            .children()
            .find(|node| node.name() == Some(name))
            .ok_or("mounted model child")
    };
    assert!(node("hidden")?.mesh().is_none(), "retain the meshless node");
    let index = |name, count| -> Result<usize, Box<dyn Error>> {
        let mesh = node(name)?.mesh().ok_or("mesh for drawable node")?;
        let mut primitives = mesh.primitives();
        assert_eq!(primitives.len(), 1, "one primitive in {name}");
        let indices = primitives
            .next()
            .and_then(|primitive| primitive.indices())
            .ok_or("primitive indices")?;
        assert_eq!(indices.count(), count, "geometry bound to {name}");
        Ok(mesh.index())
    };
    Ok([index("body", body)?, index("barrel", barrel)?])
}
