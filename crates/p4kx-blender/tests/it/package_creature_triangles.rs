// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::MeshChunkSource;
use p4kx_testkit::{TestModel, check_glb};
use serde_json::{Value, json};

use crate::support::TestResult;

use super::{HORN, SKIN};

pub(super) fn assert_scene_triangles(
    package: &Value,
    glb: &[u8],
    models: [&TestModel; 2],
) -> TestResult {
    let observed = check_glb(glb)?;
    let mut triangles = 0;
    let mut primitives = 0;
    for mesh in observed.document.meshes() {
        for primitive in mesh.primitives() {
            let accessor = primitive.indices().ok_or("written index accessor")?;
            triangles += accessor.count() / 3;
            primitives += 1;
        }
    }
    assert_eq!(
        package.pointer("/meshes/0/triangles"),
        Some(&json!(triangles)),
        "the package count uses every GLB primitive"
    );
    let scene = package.get("scene_provenance").ok_or("scene provenance")?;
    let origin = scene
        .get("fieldOrigins")
        .and_then(|fields| fields.get("/meshes/0/triangles"))
        .ok_or("triangle field origin")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the encrypted skin contributes"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(false)),
        "unobserved selection keeps the origin partial"
    );
    let references = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("triangle source uses")?;
    assert_eq!(
        references.len(),
        primitives,
        "one use for each emitted count word"
    );
    assert_triangle_source_counts(scene, references, models)
}

fn assert_triangle_source_counts(
    scene: &Value,
    references: &[Value],
    models: [&TestModel; 2],
) -> TestResult {
    let sources = scene
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    let source_uses = scene
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("source use inventory")?;
    for (model, path) in models.into_iter().zip([SKIN, HORN]) {
        let geometry = ChunkFile::parse(model.cgfm())?;
        let (_, chunk) = geometry
            .chunks_of(ivo::SKIN_2)
            .next()
            .ok_or("original geometry chunk")?;
        let decoded = model.skin_chunk();
        let original = MeshChunkSource::read(chunk, &decoded)?.ok_or("original mesh layout")?;
        let source = sources
            .iter()
            .find(|source| source.pointer("/path/value") == Some(&json!(path)))
            .ok_or("original mesh source")?;
        let physical_reference = source.get("id").ok_or("original mesh source id")?;
        assert_count_words(&original, physical_reference, source_uses, references)?;
    }
    Ok(())
}

fn assert_count_words(
    original: &MeshChunkSource<'_>,
    source_id: &Value,
    source_uses: &[Value],
    references: &[Value],
) -> TestResult {
    for submesh in original.submeshes().filter(|row| row.index_count() != 0) {
        let span = submesh.index_count_range();
        let range = json!([span.start.to_string(), span.end.to_string()]);
        let count_operand = source_uses
            .iter()
            .find(|row| {
                row.get("source") == Some(source_id)
                    && row.get("id").is_some_and(|id| references.contains(id))
                    && row.pointer("/location/indices") == Some(&range)
            })
            .ok_or_else(|| format!("bound decoded-file count word at {}", span.start))?;
        assert_eq!(
            count_operand.get("role"),
            Some(&json!("conversion")),
            "the count is a conversion operand"
        );
        assert_eq!(
            count_operand.pointer("/location/field"),
            Some(&json!("indexCount")),
            "source count field"
        );
    }
    Ok(())
}
