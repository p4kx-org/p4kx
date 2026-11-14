// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, LoadoutResolver, Piece};
use p4kx_blender::{
    CharacterScene, Manifest, PackageInfo, PackageWriteContext,
    write_character_package_with_context,
};
use p4kx_chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, ArchiveWriteProtection, DataForgeEntry, OpenOptions};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{TestZoneChunk, ZONE_SHIRT, check_glb, mesh_streams, test_key, zone_entries};
use serde_json::{Value, json};

use super::{HEAD_VERTICES, MALE_RIG_HASH, dna, head_entries, head_path, head_rig};
use crate::package_vertex_map::{assert_receipt, rows};
use crate::support::{TestResult, fixture, glb_options};

const GLB: &str = "meshes/m0001.glb";

#[test]
fn shaped_character_map_uses_the_contributing_skin_with_unchanged_render_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (fixture, head_path) = head_fixture()?;
    let archive = Archive::open_with(&fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let output = dir.path().join("character");
    let shaped = Piece::file("head", super::head_path(), None).shaped(dna());
    let plain = Piece::file("head", super::head_path(), None);
    write(&archive, &[plain], &output, false)?;
    let neutral = fs::read(output.join(GLB))?;
    write(&archive, std::slice::from_ref(&shaped), &output, false)?;
    let rendered = fs::read(output.join(GLB))?;
    assert_ne!(
        rendered, neutral,
        "DNA shaping changes actual GLB positions"
    );
    write(&archive, std::slice::from_ref(&shaped), &output, true)?;
    assert_eq!(
        fs::read(output.join(GLB))?,
        rendered,
        "observation leaves GLB bytes unchanged"
    );
    let (sidecar, proof) = map(&output)?;
    let observed = assert_accessors(&proof, &rendered)?;
    let [body, head] = observed.as_slice() else {
        return Err("body and shaped head meshes".into());
    };
    assert_shaped_sources(&archive, &proof, [*body, *head], &head_path)?;
    assert_receipt(&output, &sidecar)?;
    write(&archive, &[shaped], &output, true)?;
    assert_eq!(
        rows(&output.join(sidecar))?,
        proof,
        "stable artifact and replacement"
    );
    Ok(())
}

#[test]
fn covered_body_keeps_its_explicit_unknown_map_beside_proven_clothing() -> TestResult {
    let [male, female] = &BodyVariant::ALL.map(zone_coordinates);
    let entries = zone_entries([male, female], &[TestZoneChunk::new("torso01_zone", 1)]);
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let shirt = resolver.entities().find(ZONE_SHIRT)?;
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("character");
    write(
        &archive,
        &[Piece::wearable(resolver.wearable(&shirt)?)],
        &output,
        true,
    )?;
    let (_, proof) = map(&output)?;
    let glb = fs::read(output.join(GLB))?;
    let indices = assert_accessors(&proof, &glb)?;
    let [body, shirt] = indices.as_slice() else {
        return Err("body and shirt output meshes".into());
    };
    let body_row = row(&proof, "mesh", *body)?;
    assert_eq!(
        body_row.get("mapping"),
        Some(&json!("unknown")),
        "filtered triangles invalidate first-use proof"
    );
    assert_eq!(
        body_row.get("geometryLod"),
        Some(&json!(0)),
        "preserved original geometry receipt"
    );
    assert!(
        source_path(&proof, body_row, "meshChunk")?.ends_with("m_body.skinm"),
        "original physical body"
    );
    assert!(
        first_vertex(&proof, *body).is_err(),
        "no guessed body mapping"
    );
    let shirt_row = row(&proof, "mesh", *shirt)?;
    assert_eq!(
        shirt_row.get("mapping"),
        Some(&json!("present")),
        "clothing retains its mapping"
    );
    assert!(
        source_path(&proof, first_vertex(&proof, *shirt)?, "indexWord")?.ends_with("m_suit.skinm"),
        "shirt's own original indices"
    );
    Ok(())
}

fn head_fixture() -> Result<(crate::support::Fixture, String), Box<dyn std::error::Error>> {
    let mut entries = head_entries(Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build()));
    let head = format!("{}m", head_path().replace('/', "\\"));
    let slot = entries
        .iter()
        .position(|entry| entry.path().ends_with(&head))
        .ok_or("original head geometry")?;
    let head = entries.remove(slot);
    let head_path = head.path().to_owned();
    entries.push(head.encrypted());
    Ok((fixture(entries)?, head_path))
}

fn assert_shaped_sources(
    archive: &Archive,
    proof: &[Value],
    meshes: [usize; 2],
    head_path: &str,
) -> TestResult {
    for (mesh, encrypted) in meshes.into_iter().zip([false, true]) {
        let vertex = first_vertex(proof, mesh)?;
        assert_eq!(
            vertex.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "physical skin encryption"
        );
        assert_eq!(
            vertex.get("sourceSetComplete"),
            Some(&json!(true)),
            "complete direct index proof"
        );

        let path = source_path(proof, vertex, "indexWord")?;
        assert!(
            if encrypted {
                path == head_path
            } else {
                path.ends_with("m_body.skinm")
            },
            "actual producing skin"
        );
        assert_first_use(archive, proof, vertex, path)?;
    }
    Ok(())
}

fn zone_coordinates(variant: BodyVariant) -> Vec<[f32; 2]> {
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

fn write(archive: &Archive, pieces: &[Piece], output: &Path, observed: bool) -> TestResult {
    let models = ArchiveModels::new(archive);
    let armour = Armour::build(
        BodyVariant::Male,
        pieces,
        &models,
        &AssemblyOptions::default(),
    )?;
    let animations = armour.animate(&[], &models);
    let scene = CharacterScene::new("synthetic", &armour, &animations);
    let stop = AtomicBool::new(false);
    let guard = ArchiveWriteProtection::new(archive.path(), &stop)?;
    let context = PackageWriteContext::new(&guard, &stop);
    let context = if observed {
        context.with_vertex_maps()
    } else {
        context
    };
    write_character_package_with_context(
        &scene,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        output,
        &context,
    )?;
    Ok(())
}

fn assert_accessors(proof: &[Value], glb: &[u8]) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    let schema = jsonschema::validator_for(&p4kx_blender::mesh_vertex_map_schema())?;
    for entry in proof {
        assert!(schema.is_valid(entry), "closed source row {entry}");
    }
    let glb = check_glb(glb)?;
    glb.document
        .meshes()
        .map(|mesh| {
            let primitive = mesh.primitives().next().ok_or("rigged primitive")?;
            let position = primitive.get(&Semantic::Positions).ok_or("POSITION")?;
            let row = row(proof, "mesh", mesh.index())?;
            assert_eq!(
                row.get("positionAccessor"),
                Some(&json!(position.index())),
                "same POSITION accessor"
            );
            assert_eq!(
                row.get("vertices"),
                Some(&json!(position.count().to_string())),
                "same POSITION count"
            );
            Ok(mesh.index())
        })
        .collect()
}

fn map(out: &Path) -> Result<(String, Vec<Value>), Box<dyn std::error::Error>> {
    let manifest: Manifest = serde_json::from_slice(&fs::read(out.join("scene.json"))?)?;
    assert_eq!(manifest.version, "1.20", "additive sidecar version");
    let map = manifest
        .meshes
        .first()
        .and_then(|mesh| mesh.vertex_map.as_ref())
        .ok_or("sidecar reference")?;
    Ok((map.clone(), rows(&out.join(map))?))
}

fn row<'a>(
    proof: &'a [Value],
    kind: &str,
    mesh: usize,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    proof
        .iter()
        .find(|row| {
            row.get("type").and_then(Value::as_str) == Some(kind)
                && row.get("mesh") == Some(&json!(mesh))
        })
        .ok_or_else(|| format!("missing {kind} for mesh {mesh}").into())
}

fn first_vertex(proof: &[Value], mesh: usize) -> Result<&Value, Box<dyn std::error::Error>> {
    let vertex = row(proof, "vertex", mesh)?;
    assert_eq!(
        vertex.get("vertex"),
        Some(&json!(0)),
        "first actual output vertex"
    );
    Ok(vertex)
}

fn assert_first_use(archive: &Archive, proof: &[Value], row: &Value, path: &str) -> TestResult {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("original geometry Entry")?;
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut bytes)?;
    let file = ChunkFile::parse(bytes)?;
    let (ordinal, chunk) = file.chunks_of(ivo::SKIN_2).next().ok_or("original mesh")?;
    let data = file.chunk_data(ordinal).ok_or("original mesh payload")?;
    let streams = mesh_streams(data).ok_or("independent stream framing")?;
    let indices = streams
        .streams
        .iter()
        .find(|stream| stream.tag == 0xEECD_C168)
        .ok_or("index stream")?;
    let table = data.get(168..216).ok_or("first submesh row")?;
    let word = |at| -> Result<u32, Box<dyn std::error::Error>> {
        let bytes = table.get(at..at + 4).ok_or("original submesh word")?;
        Ok(u32::from_le_bytes(bytes.try_into()?))
    };
    let offset = indices.data.start + usize::try_from(word(4)?)? * 2;
    let index = u16::from_le_bytes(
        data.get(offset..offset + 2)
            .ok_or("original first-use index word")?
            .try_into()?,
    );
    assert_eq!(
        row.get("sourceVertex"),
        Some(&json!(u32::from(index) + word(16)?)),
        "original index plus page base"
    );
    let uses = field_use(proof, row, "indexWord")?;
    let start = chunk.offset() + offset as u64;
    assert_eq!(
        uses.pointer("/location/indices"),
        Some(&json!([start.to_string(), (start + 2).to_string()])),
        "exact original index word span"
    );
    Ok(())
}

fn source_path<'a>(
    proof: &'a [Value],
    row: &Value,
    field: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    let source_use = field_use(proof, row, field)?;
    proof
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|source| source.get("id") == source_use.get("source"))
        .and_then(|source| source.pointer("/path/value").and_then(Value::as_str))
        .ok_or_else(|| "actual source path".into())
}

fn field_use<'a>(
    proof: &'a [Value],
    row: &Value,
    field: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let refs = row
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("original use references")?;
    proof
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .find(|used| {
            used.get("id").is_some_and(|id| refs.contains(id))
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
        })
        .ok_or_else(|| "actual field use".into())
}
