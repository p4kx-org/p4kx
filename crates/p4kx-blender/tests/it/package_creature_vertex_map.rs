// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_assembly::{ArchiveModels, Assembly};
use p4kx_blender::{Manifest, PackageInfo, PackageWriteContext, write_package_with_context};
use p4kx_chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, ArchiveWriteProtection, OpenOptions};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{
    CREATURE_HORNED, TestEntry, TestModel, check_glb, creature_entries, mesh_streams, test_key,
};
use serde_json::{Value, json};

use super::package_primitive_map::{ExpectedSubmesh, assert_draws, assert_submesh, primitive_row};
use super::package_vertex_map::{assert_receipt, rows};
use super::support::{TestResult, assemble, fixture, glb_options};

const SKIN: &str = r"Data\Objects\Characters\Creatures\Test\creature.skinm";
const HORN: &str = r"Data\Objects\Test\horn.cgfm";

#[test]
fn rigged_creature_vertex_map_preserves_both_physical_producers_and_replacement() -> TestResult {
    let skin = TestModel::two_bone_skin();
    let horn = TestModel::cube().scaled(0.25);
    let mut entries = creature_entries();
    let slot = entries
        .iter_mut()
        .find(|entry| entry.path() == SKIN)
        .ok_or("actual skin companion")?;
    *slot = TestEntry::zstd(SKIN, skin.cgfm()).encrypted();
    let fixture = fixture(entries)?;
    let archive = Archive::open_with(&fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let assembly = assemble(&archive, CREATURE_HORNED)?;
    let out = tempfile::tempdir()?;
    let package = out.path().join("creature");
    write(&archive, &assembly, &package, false)?;
    let original = fs::read(package.join("meshes/m0001.glb"))?;
    write(&archive, &assembly, &package, true)?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(package.join("scene.json"))?)?;
    assert_eq!(manifest.version, "1.20", "same additive package contract");
    let combined = manifest.meshes.first().ok_or("creature GLB")?;
    let map = combined
        .vertex_map
        .as_deref()
        .ok_or("creature map reference")?;
    let glb = fs::read(package.join(&combined.file))?;
    assert_eq!(
        glb, original,
        "observation leaves the rigged render output unchanged"
    );
    let proof = rows(&package.join(map))?;
    let expected = [
        (SKIN, true, first_index(&skin)?),
        (HORN, false, first_index(&horn)?),
    ];
    assert_meshes(&proof, &glb, expected)?;
    assert_creature_primitives(&proof, &glb, [&skin, &horn])?;
    assert_receipt(&package, map)?;
    write(&archive, &assembly, &package, true)?;
    assert_eq!(
        rows(&package.join(map))?,
        proof,
        "repeat publishes a complete replacement"
    );
    write(&archive, &assembly, &package, false)?;
    assert!(
        !package.join(map).exists(),
        "ordinary replacement removes the optional sidecar"
    );
    assert_eq!(
        fs::read(package.join(&combined.file))?,
        original,
        "same rig after replacement"
    );
    Ok(())
}

fn write(archive: &Archive, assembly: &Assembly, output: &Path, observe: bool) -> TestResult {
    let models = ArchiveModels::new(archive);
    let cancelled = AtomicBool::new(false);
    let guard = ArchiveWriteProtection::new(archive.path(), &cancelled)?;
    let mut context = PackageWriteContext::new(&guard, &cancelled);
    if observe {
        context = context.with_vertex_maps();
    }
    write_package_with_context(
        assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        output,
        &context,
    )?;
    Ok(())
}

fn assert_meshes(proof: &[Value], bytes: &[u8], expected: [(&str, bool, u64); 2]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_blender::mesh_vertex_map_schema())?;
    for row in proof {
        assert!(validator.is_valid(row), "closed vertex-map row {row}");
    }
    let emitted = check_glb(bytes)?;
    assert_eq!(
        emitted.document.meshes().len(),
        expected.len(),
        "one skinned and one rigid mesh"
    );
    assert_eq!(
        emitted.document.skins().len(),
        1,
        "shared creature skeleton"
    );
    for (mesh, (path, encrypted, original_index)) in emitted.document.meshes().zip(expected) {
        let primitive = mesh.primitives().next().ok_or("drawn primitive")?;
        assert_eq!(
            primitive.get(&Semantic::Joints(0)).is_some(),
            encrypted,
            "skin weights only on skin"
        );
        let position = primitive
            .get(&Semantic::Positions)
            .ok_or("actual POSITION accessor")?;
        let owner = at(proof, "mesh", mesh.index())?;
        assert_eq!(
            owner.get("positionAccessor"),
            Some(&json!(position.index())),
            "same accessor"
        );
        assert_eq!(
            owner.get("vertices"),
            Some(&json!(position.count().to_string())),
            "same accessor count"
        );
        assert_eq!(
            owner.get("selectedLod"),
            Some(&json!(0)),
            "selected source LOD"
        );
        assert_index(proof, mesh.index(), path, encrypted, original_index)?;
    }
    assert_eq!(
        proof.last().and_then(|row| row.get("meshes")),
        Some(&json!("2")),
        "complete combined map"
    );
    Ok(())
}

fn assert_index(
    proof: &[Value],
    mesh: usize,
    path: &str,
    encrypted: bool,
    start: u64,
) -> TestResult {
    let vertex = at(proof, "vertex", mesh)?;
    assert_eq!(
        vertex.get("vertex"),
        Some(&json!(0)),
        "actual first output vertex"
    );
    assert_eq!(
        vertex.get("sourceVertex"),
        Some(&json!(0)),
        "original first stream vertex"
    );
    assert_eq!(
        vertex.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "encryption of actual companion"
    );
    assert_eq!(
        vertex.get("sourceSetComplete"),
        Some(&json!(true)),
        "exact original contributor set"
    );
    let refs = vertex
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("word provenance")?;
    let index = proof
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .find(|source| {
            source.get("id").is_some_and(|id| refs.contains(id))
                && source.pointer("/location/field") == Some(&json!("indexWord"))
        })
        .ok_or("original index word")?;
    assert_eq!(
        index.pointer("/location/indices"),
        Some(&json!([start.to_string(), (start + 2).to_string()])),
        "physical index word range"
    );
    let source = proof
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|source| source.get("id") == index.get("source"))
        .ok_or("physical input identity")?;
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!(path)),
        "actual geometry source"
    );
    Ok(())
}

fn at<'a>(
    proof: &'a [Value],
    kind: &str,
    mesh: usize,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    proof
        .iter()
        .find(|row| row.get("type") == Some(&json!(kind)) && row.get("mesh") == Some(&json!(mesh)))
        .ok_or_else(|| format!("missing {kind} row for mesh {mesh}").into())
}

fn assert_creature_primitives(proof: &[Value], glb: &[u8], models: [&TestModel; 2]) -> TestResult {
    assert_draws(proof, glb)?;
    for (mesh, (model, source)) in models.into_iter().zip([SKIN, HORN]).enumerate() {
        let original = model.cgfm();
        let geometry = ChunkFile::parse(original)?;
        let (_, chunk) = geometry
            .chunks_of(ivo::SKIN_2)
            .next()
            .ok_or("physical mesh chunk")?;
        let decoded = model.skin_chunk();
        let material = decoded
            .get(168..170)
            .ok_or("authored material")?
            .try_into()?;
        let node = decoded.get(170..172).ok_or("authored node")?.try_into()?;
        let row = primitive_row(proof, mesh, 0)?;
        assert_eq!(row.get("modelSubmesh"), Some(&json!(0)), "decoded row");
        assert_submesh(
            proof,
            row,
            &ExpectedSubmesh {
                source,
                encrypted: mesh == 0,
                start: chunk.offset() + 168,
                ordinal: 0,
                node: u16::from_le_bytes(node),
                material: u16::from_le_bytes(material),
            },
        )?;
    }
    Ok(())
}

fn first_index(model: &TestModel) -> Result<u64, Box<dyn std::error::Error>> {
    let original = model.skin_chunk();
    let streams = mesh_streams(&original).ok_or("physical stream offsets")?;
    let stream = streams
        .streams
        .iter()
        .find(|stream| stream.tag == 0xEECD_C168)
        .ok_or("index stream")?;
    let geometry = ChunkFile::parse(model.cgfm())?;
    let (_, chunk) = geometry
        .chunks_of(ivo::SKIN_2)
        .next()
        .ok_or("physical mesh chunk")?;
    Ok(chunk.offset() + stream.data.start as u64)
}
