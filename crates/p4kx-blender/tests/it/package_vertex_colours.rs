// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The vertex colours of a package mesh: the shares of the layers of a layered surface, which the
//! add-on reads, though a viewer of a plain GLB would multiply them into the colour.

use p4kx_gltf::GlbOptions;

use crate::package_materials::{Written, write, write_with};
use crate::support::TestResult;

/// A primitive of a package mesh: its material, whether it carries `COLOR_0` and whether the
/// manifest states that the material reads the vertex colours.
struct Primitive {
    material: String,
    carries: bool,
    reads: bool,
}

fn primitives(written: &Written) -> Result<Vec<Primitive>, Box<dyn std::error::Error>> {
    let mesh = written.manifest.meshes.first().ok_or("a mesh")?;
    let bytes = std::fs::read(written.dir.path().join("ship").join(&mesh.file))?;
    let length = u32::from_le_bytes(bytes.get(12..16).ok_or("a header")?.try_into()?) as usize;
    let json = bytes.get(20..20 + length).ok_or("a JSON chunk")?;
    let document: serde_json::Value = serde_json::from_slice(json)?;
    let materials = document
        .get("materials")
        .and_then(serde_json::Value::as_array);
    let materials = materials.ok_or("materials")?;
    let list = document
        .pointer("/meshes/0/primitives")
        .and_then(serde_json::Value::as_array)
        .ok_or("primitives")?;
    let mut found = Vec::new();
    for primitive in list {
        let at = primitive
            .get("material")
            .and_then(serde_json::Value::as_u64);
        let at = usize::try_from(at.ok_or("a material")?)?;
        let name = materials
            .get(at)
            .and_then(|m| m.get("name"))
            .and_then(serde_json::Value::as_str)
            .ok_or("a name")?;
        found.push(Primitive {
            material: name.to_owned(),
            carries: primitive.pointer("/attributes/COLOR_0").is_some(),
            reads: written.material(name)?.reads_vertex_colours,
        });
    }
    Ok(found)
}

#[test]
fn switched_off_vertex_colours_stay_off_in_a_package_mesh() -> TestResult {
    let written = write_with(&glb_off())?;
    let found = primitives(&written)?;
    assert!(found.len() > 1, "the ship draws several materials");
    assert!(found.iter().all(|found| !found.carries));
    Ok(())
}

fn glb_off() -> GlbOptions {
    crate::support::glb_options(0).without_vertex_colours()
}

#[test]
fn a_package_mesh_carries_the_layer_shares_of_a_layered_material_as_color_0() -> TestResult {
    let found = primitives(&write()?)?;
    for found in &found {
        assert_eq!(found.carries, found.reads, "{}", found.material);
    }
    assert!(
        found.iter().any(|found| found.carries),
        "the hull is layered and reads the colours"
    );
    Ok(())
}
