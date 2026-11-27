// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_assembly::BodyVariant;
use p4kx_testkit::{
    TestEntry, TestModel, TestZoneChunk, ZONE_JACKET, ZONE_SHIRT, armour_entries,
    armour_male_skeleton, jacketed_entries,
};
use serde_json::{Value, json};

mod package;
use package::Package;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const GLB: &str = "meshes/m0001.glb";
const MAP: &str = "/meshes/0/rig/body_zones";
const TORSO: &str = "torso01_zone";
const FOOT: &str = "l_foot_zone";
const CONDITIONAL: &str = "r_foot_zone";

#[test]
fn separately_exported_pieces_reproduce_the_full_outfit_body_mask() -> TestResult {
    let dir = tempfile::tempdir()?;
    let [male, female] = BodyVariant::ALL.map(zone_positions);
    let shirt = [
        TestZoneChunk::new(TORSO, 1),
        TestZoneChunk::new(CONDITIONAL, 1).conditional(),
    ];
    let jacket = [
        TestZoneChunk::new(FOOT, 2),
        TestZoneChunk::new("r_hand_zone", 2).showing(&[0]),
    ];
    let archive = crate::chf_glb::archive(
        dir.path(),
        jacketed_entries([&male, &female], (&shirt, &[]), &jacket, &[]),
    )?;
    for variant in BodyVariant::ALL {
        let destination = dir.path().join(variant.code());
        fs::create_dir(&destination)?;
        compare_outfit(&archive, &destination, variant)?;
    }
    Ok(())
}

#[test]
fn unrecognized_body_layout_keeps_the_geometry_and_reports_unavailable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = crate::chf_glb::archive(dir.path(), armour_entries())?;
    let ordinary = dir.path().join("ordinary");
    let modular = dir.path().join("modular");
    export(&archive, &ordinary, BodyVariant::Male, &[], false, 0)?;
    export(&archive, &modular, BodyVariant::Male, &[], true, 0)?;
    assert_eq!(
        fs::read(ordinary.join(GLB))?,
        fs::read(modular.join(GLB))?,
        "observing zones leaves the ordinary geometry byte-identical"
    );
    let manifest = read_manifest(&modular)?;
    assert_eq!(
        body_map(&manifest)?.pointer("/body/state"),
        Some(&json!("unavailable")),
        "the generic cube fixture has no authoritative human body zones"
    );
    assert!(
        read_manifest(&ordinary)?.pointer(MAP).is_none(),
        "the standard package keeps its existing manifest shape"
    );
    Ok(())
}

fn compare_outfit(archive: &Path, destination: &Path, variant: BodyVariant) -> TestResult {
    let base = build_package(archive, &destination.join("base"), variant, &[])?;
    let shirt = build_package(archive, &destination.join("shirt"), variant, &[ZONE_SHIRT])?;
    let jacket = build_package(
        archive,
        &destination.join("jacket"),
        variant,
        &[ZONE_JACKET],
    )?;
    let outfit = build_package(
        archive,
        &destination.join("outfit"),
        variant,
        &[ZONE_SHIRT, ZONE_JACKET],
    )?;
    let mut expected = base.body_primitives()?;
    assert_eq!(
        expected.len(),
        variant.zone_names().len(),
        "the intact base exposes every original body zone"
    );
    let shirt_map = body_map(&shirt.manifest)?;
    let jacket_map = body_map(&jacket.manifest)?;
    let outfit_map = body_map(&outfit.manifest)?;
    assert_mask_states(shirt_map, outfit_map);
    for (package, map, zone, item) in [
        (&shirt, shirt_map, TORSO, ZONE_SHIRT),
        (&jacket, jacket_map, FOOT, ZONE_JACKET),
    ] {
        assert_eq!(
            map.pointer("/parts/0/coverage/covers"),
            Some(&json!([zone])),
            "the single item exposes its own definite mask"
        );
        assert!(
            expected.remove(zone).is_some(),
            "item covers a mapped primitive of the common body: {zone}"
        );
        assert_eq!(
            package.part_primitives(item)?,
            outfit.part_primitives(item)?,
            "the item's actual GLB triangles agree in isolation and in the outfit"
        );
        assert_eq!(
            package.bind_pose()?,
            outfit.bind_pose()?,
            "one bind pose for {item}"
        );
    }
    assert_eq!(
        expected,
        outfit.body_primitives()?,
        "the union of the two item masks reproduces the complete outfit's body triangles"
    );
    assert_eq!(
        base.bind_pose()?,
        outfit.bind_pose()?,
        "same skeleton and bind data"
    );
    assert_zone_evidence(shirt_map)?;
    Ok(())
}

fn assert_mask_states(shirt_map: &Value, outfit_map: &Value) {
    assert_eq!(
        shirt_map.pointer("/parts/0/coverage/conditional"),
        Some(&json!([CONDITIONAL])),
        "unresolved conditions stay visible in the coverage information"
    );
    assert_eq!(
        outfit_map.pointer("/body/omitted"),
        Some(&json!([FOOT, TORSO])),
        "the ordinary assembly hides exactly the two definite zones"
    );
}

fn assert_zone_evidence(map: &Value) -> TestResult {
    let inventory = map
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("zone uses")?;
    for (field, authored) in [("covers", 0), ("conditional", 1)] {
        let path = format!("/parts/0/coverage/{field}/0");
        let origin = map
            .get("field_origins")
            .and_then(|fields| fields.get(&path))
            .ok_or("per-zone clothing origin")?;
        let [id] = origin
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("zone use reference")?
            .as_slice()
        else {
            return Err("expected exactly one observed clothing chunk".into());
        };
        let source = inventory
            .iter()
            .find(|entry| entry.get("id") == Some(id))
            .ok_or("matching source use")?;
        let coordinate = source
            .pointer("/location/field")
            .and_then(Value::as_str)
            .ok_or("chunk source position")?;
        assert!(
            coordinate.ends_with(&format!("].Chunks[{authored}]")),
            "one exact original chunk for {path}"
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(false)),
            "coverage retains partial evidence explicitly at {path}"
        );
    }
    Ok(())
}

#[test]
fn worn_primitives_report_only_the_zones_bound_by_the_original_skin() -> TestResult {
    let dir = tempfile::tempdir()?;
    let [male, female] = BodyVariant::ALL.map(zone_positions);
    let shirt = [TestZoneChunk::new(TORSO, 1)];
    let mut entries = jacketed_entries([&male, &female], (&shirt, &[]), &[], &[]);
    let stem = "Data\\Objects\\Characters\\Human\\male_v7\\armor\\test\\m_suit";
    let skin = format!("{stem}.skin");
    let mesh = format!("{stem}.skinm");
    entries.retain(|entry| {
        !entry.path().eq_ignore_ascii_case(&skin) && !entry.path().eq_ignore_ascii_case(&mesh)
    });
    let model = TestModel::zoned(&[[0.0, 1.2], [0.0, 1.3], [0.0, 1.4]])
        .with_skeleton(&armour_male_skeleton())
        .with_bone_map(false, vec![vec![(0, 255)]; 9])
        .with_mesh_extra_words(vec![0xEBBD_944A, 0xDEAD_BEEF]);
    entries.push(TestEntry::stored(skin, model.cgf()));
    entries.push(TestEntry::stored(mesh, model.cgfm()));
    let archive = crate::chf_glb::archive(dir.path(), entries)?;
    let package = build_package(
        &archive,
        &dir.path().join("shirt"),
        BodyVariant::Male,
        &[ZONE_SHIRT],
    )?;
    let map = body_map(&package.manifest)?;
    let mesh = map
        .pointer("/parts/0/meshes/0")
        .ok_or("emitted shirt mesh")?;
    assert_eq!(
        map.pointer("/parts/0/primitives"),
        Some(&json!([{"zone": TORSO, "mesh": mesh, "primitive": 0, "model_lod": 0}])),
        "only the descriptor whose physical table word matches the shirt Record is labelled"
    );
    assert_eq!(
        package.part_primitives(ZONE_SHIRT)?.len(),
        3,
        "unmatched and out-of-table geometry stays in the GLB"
    );
    let evidence = map
        .pointer("/field_origins/~1parts~10~1primitives~10~1zone")
        .ok_or("physical primitive provenance")?;
    assert_eq!(
        evidence
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3),
        "zone needs the original descriptor, hash word and clothing Record"
    );
    Ok(())
}

#[test]
fn the_body_map_uses_the_geometry_lod_that_the_glb_emits() -> TestResult {
    let dir = tempfile::tempdir()?;
    let [male, female] = BodyVariant::ALL.map(zone_positions);
    let archive = crate::chf_glb::archive(
        dir.path(),
        jacketed_entries([&male, &female], (&[], &[]), &[], &[]),
    )?;
    let out = dir.path().join("fallback");
    export(&archive, &out, BodyVariant::Male, &[], true, 1)?;
    let manifest = read_manifest(&out)?;
    let body = body_map(&manifest)?.get("body").ok_or("body map")?;
    assert_eq!(
        body.get("state"),
        Some(&json!("verified")),
        "actual body layout"
    );
    let primitives = body
        .get("primitives")
        .and_then(Value::as_array)
        .ok_or("draw addresses")?;
    assert!(
        !primitives.is_empty()
            && primitives
                .iter()
                .all(|row| row.get("model_lod") == Some(&json!(0))),
        "requested level 1 uses the body's supplied level 0"
    );
    Ok(())
}

fn zone_positions(variant: BodyVariant) -> Vec<[f32; 2]> {
    variant
        .zone_names()
        .iter()
        .map(|zone| {
            let x = if zone.starts_with("l_") {
                -0.2
            } else if zone.starts_with("r_") {
                0.2
            } else {
                0.0
            };
            [x, if zone.contains("foot") { 0.1 } else { 1.2 }]
        })
        .collect()
}

fn export(
    archive: &Path,
    target: &Path,
    variant: BodyVariant,
    pieces: &[&str],
    modular: bool,
    lod: u32,
) -> TestResult {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args(["character", "package"])
        .arg(archive)
        .args(["--body", variant.code(), "--materials", "colours", "--out"])
        .arg(target)
        .arg("--lod")
        .arg(lod.to_string());
    if modular {
        command.arg("--body-zones");
    }
    for piece in pieces {
        command.args(["--piece", piece]);
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "Character package failed: {output:?}"
    );
    Ok(())
}

fn build_package(
    archive: &Path,
    target: &Path,
    variant: BodyVariant,
    pieces: &[&str],
) -> TestResult<Package> {
    export(archive, target, variant, pieces, true, 0)?;
    Package::read(target)
}

fn read_manifest(directory: &Path) -> TestResult<Value> {
    Ok(serde_json::from_slice(&fs::read(
        directory.join("scene.json"),
    )?)?)
}

fn body_map(manifest: &Value) -> TestResult<&Value> {
    assert_eq!(
        manifest.get("version"),
        Some(&json!("1.24")),
        "body-zone package version"
    );
    manifest
        .pointer(MAP)
        .ok_or_else(|| "missing Character zone map".into())
}
