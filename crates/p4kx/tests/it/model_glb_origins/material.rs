// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::MeshChunkSource;
use p4kx_testkit::{
    ArchiveBuilder, TestEntry, TestModel, TestMtl, TestMtlMaterial, check_glb, test_key,
};
use serde_json::Value;

use super::{BASE, MODEL, TestResult, assert_use, field_at, original_range, receipt};

#[test]
fn single_material_does_not_use_the_original_submesh_word_as_a_slot() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive_path = dir.path().join("fixture.p4k");
    let model = TestModel::cube();
    let material = TestMtl::single(&TestMtlMaterial::new("whole mesh"));
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.cgf()))
        .entry(TestEntry::zstd(BASE, model.cgfm()).encrypted())
        .entry(TestEntry::zstd(
            "Data/Objects/Test/cube.mtl",
            material.cryxml(),
        ))
        .write_to(&archive_path)?;
    let output = dir.path().join("single.glb");
    p4kx_testkit::p4kx()?
        .args(["--key", &test_key().to_hex(), "model", "glb"])
        .arg(&archive_path)
        .arg(MODEL)
        .arg("--out")
        .arg(&output)
        .assert()
        .success();
    let bytes = fs::read(&output)?;
    let rows = receipt(&output, &bytes)?;
    let glb = check_glb(&bytes)?;
    let mut count = 0;
    for mesh in glb.document.meshes() {
        for primitive in mesh.primitives() {
            assert_eq!(
                primitive.material().index(),
                Some(0),
                "shared output material"
            );
            let pointer = format!(
                "/meshes/{}/primitives/{}/material",
                mesh.index(),
                primitive.index()
            );
            let field = field_at(&rows, &pointer)?;
            assert_eq!(field.get("containsEncryptedSource"), Some(&Value::Null));
            assert_eq!(field.get("sourceSetComplete"), Some(&Value::Bool(false)));
            assert_eq!(field.get("sourceUsesRefs"), Some(&Value::Array(Vec::new())));
            count += 1;
        }
    }
    assert_eq!(count, 2, "both physical submeshes use the single material");
    Ok(())
}

pub(super) fn check_material_selectors(
    bytes: &[u8],
    rows: &[Value],
    entry: &str,
    geometry: &[u8],
    encrypted: bool,
) -> TestResult {
    let chunks = ChunkFile::parse(geometry)?;
    let mesh_source = authored_mesh(&chunks)?;
    let glb = check_glb(bytes)?;
    let mut seen = Vec::new();
    for mesh in glb.document.meshes() {
        for primitive in mesh.primitives() {
            let material = primitive.material();
            let index = material.index().ok_or("emitted library material")?;
            let pointer = format!(
                "/meshes/{}/primitives/{}/material",
                mesh.index(),
                primitive.index()
            );
            let field = field_at(rows, &pointer)?;
            let marker = if encrypted {
                Value::Bool(true)
            } else {
                Value::Null
            };
            assert_eq!(
                field.get("containsEncryptedSource"),
                Some(&marker),
                "{pointer}"
            );
            assert_eq!(
                field.get("sourceSetComplete"),
                Some(&Value::Bool(false)),
                "{pointer}"
            );
            let (used, bytes) = original_range(field, rows, entry, geometry)?;
            assert_use(used, "materialId", "selector");
            let id = u16::from_le_bytes(bytes.try_into()?);
            let slot = match id {
                1 => "caps",
                2 => "sides",
                _ => return Err("unexpected slot from the observed original word".into()),
            };
            assert_eq!(
                material.name(),
                Some(slot),
                "actual sub-material at {pointer}"
            );
            check_material_word(&mesh_source, geometry, id, bytes, &pointer)?;
            seen.push((id, index));
        }
    }
    seen.sort_unstable();
    let [(caps, cap_index), (sides, side_index)] = seen.as_slice() else {
        return Err("both source selectors must be covered".into());
    };
    assert_eq!([*caps, *sides], [1, 2], "distinct original material IDs");
    assert_ne!(cap_index, side_index, "distinct dense materials");
    Ok(())
}

fn authored_mesh<'a>(chunks: &'a ChunkFile<&[u8]>) -> Result<MeshChunkSource<'a>, Box<dyn Error>> {
    let (index, chunk) = chunks
        .chunks()
        .iter()
        .enumerate()
        .find(|(_, chunk)| MeshChunkSource::supports(chunk))
        .ok_or("fixture IVO geometry")?;
    Ok(
        MeshChunkSource::read(chunk, chunks.chunk_data(index).ok_or("chunk bytes")?)?
            .ok_or("supported source mesh")?,
    )
}

fn check_material_word(
    mesh: &MeshChunkSource<'_>,
    geometry: &[u8],
    id: u16,
    bytes: &[u8],
    pointer: &str,
) -> TestResult {
    let source = mesh
        .submeshes()
        .find(|row| row.material() == id)
        .ok_or("authored material slot")?;
    let range = source.material_range();
    let expected = geometry
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or("physical slot range")?;
    assert!(
        std::ptr::eq(bytes, expected),
        "exact source word at {pointer}"
    );
    Ok(())
}
