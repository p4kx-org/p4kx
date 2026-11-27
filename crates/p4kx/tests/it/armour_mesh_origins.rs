// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_assembly::{
    ArchiveModels, Armour, AssemblyOptions, BodyVariant, ForgePalettes, LoadoutResolver, Piece,
};
use p4kx_extraction::{Archive, DataForgeEntry, GlbOptions};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{armour_entries, check_glb};
use serde_json::Value;

use super::{TestResult, check_artifact_binding, contributors, mount_archive};

#[test]
fn unmounted_armour_glb_binds_emitted_geometry_without_changing_the_bytes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = mount_archive(directory.path(), armour_entries(), false)?;
    let out = directory.path().join("armour.glb");
    let result = p4kx_testkit::p4kx()?
        .args(["armour", "glb"])
        .arg(&archive)
        .args(["--piece", "TEST_Suit", "--body", "male", "--out"])
        .arg(&out)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&out)?;
    assert_eq!(
        bytes,
        unobserved_glb(&archive, None)?,
        "mesh capture preserves the ordinary rigged GLB"
    );
    let receipt = fs::read_to_string(out.with_extension("glb.p4kx-origin.jsonl"))?;
    let rows = receipt
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    check_artifact_binding(&bytes, &rows)?;
    check_geometry(&bytes, &rows, "m_suit.skinm", None)?;
    assert!(
        rows.iter().all(|row| !row
            .pointer("/address/path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.starts_with("/nodes/"))),
        "an unmounted body or skinned suit does not have a worn-mount origin"
    );
    Ok(())
}

pub(super) fn unobserved_glb(
    path: &Path,
    loadout: Option<&str>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let pieces = if let Some(loadout) = loadout {
        let record = resolver.entities().find(loadout)?;
        let tree = resolver.resolve(&record)?;
        resolver.worn(&tree)
    } else {
        let record = resolver.entities().find("TEST_Suit")?;
        vec![Piece::wearable(resolver.wearable(&record)?)]
    };
    let mut models = ArchiveModels::new(&archive);
    if pieces.iter().any(Piece::has_port_mount) {
        models = models.with_model_node_sources();
    }
    let armour = Armour::build_with_palettes(
        BodyVariant::Male,
        &pieces,
        &models,
        &ForgePalettes::new(&forge),
        &AssemblyOptions::default(),
    )?;
    let mut bytes = Vec::new();
    armour.write_glb_animated_with(
        &armour.animate(&[], &models),
        &GlbOptions::default().with_texture_mip(1),
        &models.textures(),
        &mut bytes,
    )?;
    Ok(bytes)
}

pub(super) fn check_geometry(
    bytes: &[u8],
    rows: &[Value],
    source_tail: &str,
    encrypted: Option<bool>,
) -> TestResult {
    let artifact = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("aggregate GLB origin")?;
    assert_eq!(
        artifact.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "emitted geometry and mount observations form a partial source set"
    );
    assert_eq!(
        artifact.get("containsEncryptedSource"),
        Some(&serde_json::to_value(encrypted)?),
        "artifact encryption reflects the observed source uses"
    );
    let aggregate = artifact
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("aggregate source uses")?;

    let source_id = rows
        .iter()
        .find(|row| {
            row.pointer("/source/path/value")
                .and_then(Value::as_str)
                .is_some_and(|path| path.ends_with(source_tail))
        })
        .and_then(|row| row.pointer("/source/id"))
        .ok_or("the original geometry entry")?;
    let glb = check_glb(bytes)?;
    for accessor in glb
        .document
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .filter_map(|primitive| primitive.get(&Semantic::Positions))
    {
        let usage = contributors(
            rows,
            &format!("/accessors/{}/count", accessor.index()),
            None,
        )?;
        if let Some(chunk) = usage.iter().find(|usage| geometry_of(usage, source_id)) {
            let reference = chunk.get("id").ok_or("geometry use reference")?;
            assert!(
                aggregate.contains(reference),
                "the same geometry contributes to the artifact"
            );
            return Ok(());
        }
    }
    Err(format!("no emitted POSITION count originates in {source_tail}").into())
}

fn geometry_of(usage: &Value, source: &Value) -> bool {
    usage.get("source") == Some(source)
        && usage.get("role").and_then(Value::as_str) == Some("conversion")
        && usage.pointer("/location/namespace").and_then(Value::as_str) == Some("modelChunk")
        && usage.pointer("/location/field").and_then(Value::as_str) == Some("geometry")
}
