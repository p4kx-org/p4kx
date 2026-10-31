// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The vertex colours of a package mesh: the shares of the layers of a layered surface, which the
//! add-on reads, though a viewer of a plain GLB would multiply them into the colour.

use crate::package_materials::write;
use crate::support::TestResult;

#[test]
fn a_package_mesh_carries_the_layer_shares_of_a_layered_material_as_color_0() -> TestResult {
    let written = write()?;
    let mesh = written.manifest.meshes.first().ok_or("a mesh")?;
    let bytes = std::fs::read(written.dir.path().join("ship").join(&mesh.file))?;
    let length = u32::from_le_bytes(bytes[12..16].try_into()?) as usize;
    let document: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length])?;
    let materials = document["materials"].as_array().ok_or("materials")?;

    let mut layered = 0_usize;
    for primitive in document["meshes"][0]["primitives"]
        .as_array()
        .ok_or("primitives")?
    {
        let material = usize::try_from(primitive["material"].as_u64().ok_or("a material")?)?;
        let name = materials[material]["name"].as_str().ok_or("a name")?;
        let carries = primitive["attributes"].get("COLOR_0").is_some();
        let reads = written.material(name)?.reads_vertex_colours;
        assert_eq!(carries, reads, "{name}");
        layered += usize::from(carries);
    }
    assert!(layered > 0, "the hull is layered and reads the colours");
    Ok(())
}
