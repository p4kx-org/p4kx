// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Normal entity exports of a fitted paint from either original variant list.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_blender::{Manifest, Mesh};
use p4kx_testkit::{
    check_entity_package_sources, check_glb, fitted_paint_vehicle_entries, test_key,
};
use serde_json::Value;

use crate::entity_package::{manifest, package, write_archive};

#[path = "entity_vehicle_paint_origins.rs"]
mod paint_origins;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct PaintFixture {
    entity: &'static str,
    material: &'static str,
    library: &'static str,
    palette: &'static str,
    tint: [f32; 3],
    selector: &'static str,
    variant_tags: &'static str,
    hull_record: u64,
    paint_record: u64,
    palette_record: u64,
}

const CASES: [PaintFixture; 2] = [
    PaintFixture {
        entity: "TEST_Hull_Red",
        material: "FixtureRedHull",
        library: "Data/Objects/Test/red.mtl",
        palette: "synthetic_red",
        tint: [1.0, 0.0, 0.0],
        selector: "Components[2].Material.materialVariants[0].Palette.RootRecord",
        variant_tags: "Components[2].Material.materialVariants[0].Tags",
        hull_record: 0,
        paint_record: 4,
        palette_record: 6,
    },
    PaintFixture {
        entity: "TEST_Hull_Blue",
        material: "FixtureBlueHull",
        library: "Data/Objects/Test/blue.mtl",
        palette: "synthetic_blue",
        tint: [0.0, 0.0, 1.0],
        selector: "Components[1].Geometry.SubGeometry[0].Geometry.Palette.RootRecord",
        variant_tags: "Components[1].Geometry.SubGeometry[0].Tags",
        hull_record: 1,
        paint_record: 5,
        palette_record: 7,
    },
];

#[test]
fn fitted_paints_reach_normal_glb_and_blender_package() -> TestResult {
    let folder = tempfile::tempdir()?;
    let entries = fitted_paint_vehicle_entries()
        .into_iter()
        .map(|entry| {
            if entry.path() == "Data\\Game2.dcb" {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let archive = write_archive(folder.path(), entries)?;
    let key = test_key().to_hex();
    for fixture in CASES {
        let glb = folder.path().join(format!("{}.glb", fixture.entity));
        check_standalone(&fixture, &archive, &glb, &key)?;
        let output = folder.path().join(fixture.entity);
        let exported = package(&archive, fixture.entity, &output, &["--key", &key], false)?;
        assert_eq!(exported.code, Some(0), "{}", exported.stderr);
        check_package(&fixture, &output)?;
        paint_origins::check_export(&fixture, &glb, &output)?;
    }
    Ok(())
}

fn check_standalone(fixture: &PaintFixture, archive: &Path, out: &Path, key: &str) -> TestResult {
    let exported = p4kx_testkit::p4kx()?
        .args(["entity", "glb"])
        .arg(archive)
        .arg(fixture.entity)
        .arg("--out")
        .arg(out)
        .args(["--key", key])
        .output()?;
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let glb = check_glb(&fs::read(out)?)?;
    let mut primitives = glb
        .document
        .nodes()
        .filter_map(|node| node.mesh())
        .flat_map(|mesh| mesh.primitives())
        .peekable();
    assert!(primitives.peek().is_some(), "the hull has drawn primitives");
    for primitive in primitives {
        let material = primitive.material();
        assert_eq!(
            material.name(),
            Some(fixture.material),
            "drawn hull material"
        );
        let [r, g, b, _] = material.pbr_metallic_roughness().base_color_factor();
        check_tint([r, g, b], fixture.tint);
    }
    Ok(())
}

fn check_package(fixture: &PaintFixture, out: &Path) -> TestResult {
    let manifest = manifest(out)?;
    check_entity_package_sources(out, &manifest, &[fixture.library])?;
    let package: Manifest = serde_json::from_value(manifest)?;
    let (node_index, root) = package
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.path.is_empty())
        .ok_or("package hull")?;
    let [mesh] = package.meshes.as_slice() else {
        return Err("one drawn hull mesh".into());
    };
    assert_eq!(
        root.mesh.as_ref(),
        Some(&mesh.id),
        "the hull uses this mesh"
    );
    check_package_material(fixture, out, &package, mesh)?;
    let palette = package
        .palettes
        .iter()
        .find(|palette| Some(&palette.id) == mesh.palette_id.as_ref())
        .ok_or("the hull mesh has a fitted palette")?;
    assert_eq!(palette.name, fixture.palette, "authored variant palette");
    check_tint(palette.primary.tint, fixture.tint);
    let origins = package
        .palette_provenance
        .as_ref()
        .ok_or("palette origins")?;
    check_palette_origins(fixture, node_index, origins)
}

fn check_package_material(
    fixture: &PaintFixture,
    out: &Path,
    package: &Manifest,
    mesh: &Mesh,
) -> TestResult {
    let glb = check_glb(&fs::read(out.join(&mesh.file))?)?;
    let mut primitives = glb
        .document
        .nodes()
        .filter_map(|node| node.mesh())
        .flat_map(|mesh| mesh.primitives())
        .peekable();
    assert!(primitives.peek().is_some(), "the hull has drawn primitives");
    for primitive in primitives {
        let index = primitive.material().index().ok_or("hull material index")?;
        let assignment = mesh
            .materials
            .iter()
            .find(|assignment| assignment.index == index)
            .ok_or("manifest material assignment")?;
        let material = package
            .materials
            .iter()
            .find(|material| Some(&material.id) == assignment.material.as_ref())
            .ok_or("manifest material definition")?;
        assert_eq!(
            material.name, fixture.material,
            "drawn material in the package"
        );
        assert_eq!(
            material.source_library,
            Some(0),
            "the material selects the verified original library companion"
        );
    }
    Ok(())
}

fn check_palette_origins(fixture: &PaintFixture, node: usize, origins: &Value) -> TestResult {
    let binding = origins
        .get("bindings")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("node").and_then(Value::as_u64) == Some(node as u64))
        })
        .ok_or("palette provenance bound to the hull")?;
    let selection = provenance_row(origins, binding, "selections", "selection")?;
    let payload = provenance_row(origins, binding, "payloads", "payload")?;
    assert_eq!(
        selection.get("kind").and_then(Value::as_str),
        Some("named"),
        "the paint's named palette selection survives"
    );
    let selector = selection
        .get("selector")
        .ok_or("original palette selector")?;
    check_origin(
        origins,
        selector,
        "selector",
        fixture.hull_record,
        fixture.selector,
    )?;
    let fields = payload.get("fieldOrigins").ok_or("palette field origins")?;
    for (channel, component) in ["r", "g", "b"].into_iter().enumerate() {
        let origin = fields
            .get(format!("/primary/tint/{channel}"))
            .ok_or("original palette channel")?;
        check_origin(
            origins,
            origin,
            "conversion",
            fixture.palette_record,
            &format!("root.entryA.tintColor.{component}"),
        )?;
    }
    Ok(())
}

fn provenance_row<'a>(
    origins: &'a Value,
    binding: &Value,
    table: &str,
    key: &str,
) -> TestResult<&'a Value> {
    let index = binding
        .get(key)
        .and_then(Value::as_u64)
        .ok_or("table index")?;
    origins
        .pointer(&format!("/{table}/{index}"))
        .ok_or_else(|| format!("missing palette provenance row {table}/{index}").into())
}

fn check_origin(
    origins: &Value,
    origin: &Value,
    role: &str,
    record_index: u64,
    field: &str,
) -> TestResult {
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the original selector or colour is in the encrypted DCB"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "its source set is complete"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("palette field references")?;
    let [reference] = references.as_slice() else {
        return Err("one exact field use".into());
    };
    let physical = origins
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .and_then(|uses| uses.iter().find(|usage| usage.get("id") == Some(reference)))
        .ok_or("the field's physical source use")?;
    assert_eq!(
        physical.get("role").and_then(Value::as_str),
        Some(role),
        "use role"
    );
    assert_eq!(
        physical
            .pointer("/location/recordIndex")
            .and_then(Value::as_u64),
        Some(record_index),
        "the physical Record"
    );
    assert_eq!(
        physical.pointer("/location/field").and_then(Value::as_str),
        Some(field),
        "the exact original field"
    );
    Ok(())
}

fn check_tint(actual: [f32; 3], expected: [f32; 3]) {
    const TOLERANCE: f32 = 1e-5;
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() < TOLERANCE),
        "the fitted palette tint is {actual:?}, expected {expected:?}"
    );
}
