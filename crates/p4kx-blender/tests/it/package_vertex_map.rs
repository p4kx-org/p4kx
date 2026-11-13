// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{Manifest, PackageInfo, PackageWriteContext, write_package_with_context};
use p4kx_chunks::ChunkFile;
use p4kx_extraction::{Archive, ArchiveWriteProtection, OpenOptions, OriginDigestWriter};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{TestEntry, TestModel, check_glb, livery_entries, mesh_streams, test_key};
use serde_json::{Value, json};

use crate::support::{TestResult, assemble, fixture, glb_options};

const GEOMETRY: &str = r"Data\Objects\Test\livery_ship.cgfm";

#[test]
fn archive_geometry_vertex_sidecar_binds_actual_glb_and_original_encrypted_ranges() -> TestResult {
    let (cgfm, mesh) = geometry()?;
    let mut entries = livery_entries();
    let slot = entries
        .iter_mut()
        .find(|entry| entry.path() == GEOMETRY)
        .ok_or("geometry Entry")?;
    *slot = TestEntry::zstd(GEOMETRY, cgfm.as_slice()).encrypted();
    let fixture = fixture(entries)?;
    let archive = Archive::open_with(&fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let out = tempfile::tempdir()?;
    let plain = out.path().join("plain");
    let observed = out.path().join("observed");
    write_pair(&archive, &plain, &observed)?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(observed.join("scene.json"))?)?;
    assert_eq!(manifest.version, "1.19", "opt-in enables the additive wire");
    let root = manifest.meshes.first().ok_or("root GLB")?;
    let map = root
        .vertex_map
        .as_ref()
        .ok_or("root vertex map reference")?;
    let glb = fs::read(observed.join(&root.file))?;
    assert_eq!(
        fs::read(plain.join(&root.file))?,
        glb,
        "unchanged render bytes"
    );
    assert!(!plain.join(map).exists(), "default omits vertex map data");
    let rows = rows(&observed.join(map))?;
    assert_map(&rows, &root.file, &glb)?;
    let file = ChunkFile::parse(cgfm.as_slice())?;
    let chunk = file.chunks().first().ok_or("geometry chunk")?;
    let streams = mesh_streams(&mesh).ok_or("original physical streams")?;
    let indices = streams
        .streams
        .iter()
        .find(|stream| stream.tag == 0xEECD_C168)
        .ok_or("original index stream")?;
    let base = chunk.offset();
    assert_first(&rows, base + indices.data.start as u64, base + 184)?;
    assert_receipt(&observed, map)?;
    let before = fs::read(observed.join(map))?;
    write_pair(&archive, &plain, &observed)?;
    assert_eq!(
        fs::read(observed.join(map))?,
        before,
        "repeated package replaces its own map"
    );
    Ok(())
}

fn geometry() -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    let mut mesh = TestModel::cube().skin_chunk();
    mesh.get_mut(176..180)
        .ok_or("submesh count word")?
        .copy_from_slice(&3_u32.to_le_bytes());
    mesh.get_mut(184..188)
        .ok_or("submesh page base")?
        .copy_from_slice(&4_u32.to_le_bytes());
    let file = p4kx_testkit::TestChunkFile::ivo()
        .chunk(p4kx_chunks::ivo::SKIN_2.id(), 0x900, mesh.as_slice())
        .bytes();
    Ok((file, mesh))
}

fn write_pair(archive: &Archive, plain: &Path, observed: &Path) -> TestResult {
    let assembly = assemble(archive, "TEST_Livery")?;
    let models = ArchiveModels::new(archive);
    let cancelled = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(archive.path(), &cancelled)?;
    let context = PackageWriteContext::new(&protection, &cancelled);
    for (out, context) in [(plain, context), (observed, context.with_vertex_maps())] {
        write_package_with_context(
            &assembly,
            &models,
            &models.textures(),
            &glb_options(0),
            &PackageInfo::default(),
            out,
            &context,
        )?;
    }
    Ok(())
}

pub(super) fn rows(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    io::BufReader::new(fs::File::open(path)?)
        .lines()
        .map(|line| Ok(serde_json::from_str::<Value>(&line?)?))
        .collect()
}

fn assert_map(rows: &[Value], glb_path: &str, bytes: &[u8]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_blender::mesh_vertex_map_schema())?;
    for row in rows {
        assert!(schema.is_valid(row), "closed vertex map row {row}");
    }
    let glb = check_glb(bytes)?;
    let primitive = glb
        .document
        .meshes()
        .next()
        .and_then(|mesh| mesh.primitives().next())
        .ok_or("actual GLB primitive")?;
    let position = primitive
        .get(&Semantic::Positions)
        .ok_or("actual POSITION")?;
    let mesh = row(rows, "mesh")?;
    assert_eq!(
        mesh.get("positionAccessor"),
        Some(&json!(position.index())),
        "actual written accessor"
    );
    assert_eq!(
        mesh.get("vertices"),
        Some(&json!(position.count().to_string())),
        "actual written count"
    );
    let mut digest = OriginDigestWriter::new(io::sink());
    io::copy(&mut &*bytes, &mut digest)?;
    let digest = digest.finish()?;
    let binding = row(rows, "artifact")?;
    assert_eq!(
        binding.get("glb"),
        Some(&json!(glb_path)),
        "same GLB manifest path"
    );
    assert_eq!(
        binding.pointer("/artifact/sha256"),
        Some(&json!(digest.sha256())),
        "same GLB bytes"
    );
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&json!("end")),
        "complete durable stream"
    );
    Ok(())
}

fn assert_first(rows: &[Value], index: u64, page: u64) -> TestResult {
    let vertex = row(rows, "vertex")?;
    assert_eq!(
        vertex.get("vertex"),
        Some(&json!(0)),
        "first written POSITION"
    );
    assert_eq!(
        vertex.get("sourceVertex"),
        Some(&json!(4)),
        "u16 index zero plus original page base four"
    );
    assert_eq!(
        vertex.get("containsEncryptedSource"),
        Some(&json!(true)),
        "only the geometry companion was encrypted"
    );
    assert_eq!(
        vertex.get("sourceSetComplete"),
        Some(&json!(true)),
        "known original physical contributors"
    );
    let refs = vertex
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("actual refs")?;
    for (label, start, end) in [
        ("indexWord", index, index + 2),
        ("vertexPageBase", page, page + 4),
    ] {
        let used = rows
            .iter()
            .find_map(|row| {
                let used = row.get("sourceUse")?;
                (used.get("id").is_some_and(|id| refs.contains(id))
                    && used.pointer("/location/field") == Some(&json!(label)))
                .then_some(used)
            })
            .ok_or("first vertex physical word")?;
        assert_eq!(
            used.pointer("/location/indices"),
            Some(&json!([start.to_string(), end.to_string()])),
            "actual physical {label} range"
        );
        let source = rows
            .iter()
            .filter_map(|row| row.get("source"))
            .find(|source| source.get("id") == used.get("source"))
            .ok_or("physical source reference")?;
        assert_eq!(
            source.pointer("/path/value"),
            Some(&json!(GEOMETRY)),
            "selected original geometry companion"
        );
    }
    Ok(())
}

fn row<'a>(rows: &'a [Value], kind: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    rows.iter()
        .find(|row| row.get("type").and_then(Value::as_str) == Some(kind))
        .ok_or_else(|| format!("missing {kind} row").into())
}

pub(super) fn assert_receipt(out: &Path, name: &str) -> TestResult {
    let receipt: Value = serde_json::from_slice(&fs::read(out.join("receipt.json"))?)?;
    let files = receipt
        .get("files")
        .and_then(Value::as_array)
        .ok_or("receipt files")?;
    let file = files
        .iter()
        .find(|row| row.get("path").and_then(Value::as_str) == Some(name))
        .ok_or("vertex map receipt")?;
    let mut digest = OriginDigestWriter::new(io::sink());
    io::copy(&mut fs::File::open(out.join(name))?, &mut digest)?;
    assert_eq!(
        file.get("sha256"),
        Some(&json!(digest.finish()?.sha256())),
        "measured sidecar receipt"
    );
    Ok(())
}
