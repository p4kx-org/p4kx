// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use egui_kittest::kittest::Queryable;
use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::{
    FieldOrigin, InputScope, SourceInventory, SourceLocation, SourceUseDescription, SourceUseRole,
};
use p4kx_gui::{Language, ModelInfoProvenance, ModelPreview, Unavailable, ViewerSupport};
use p4kx_testkit::{TestCrchModel, TestEntry, TestModel, TestSubmesh};
use serde_json::Value;

use super::info_chunk_origins::{assert_marker, reveal_info_row};
use crate::viewing::{Site, TestResult, model_preview, opened_at, select};

const MAIN: &str = "Data/Objects/mesh.cgf";
const BASE: &str = "Data/Objects/mesh.cgfm";
const LEVEL: &str = "Data/Objects/mesh_lod2.cgfm";

#[test]
fn visible_lod_rows_and_material_ids_follow_only_their_physical_words() -> TestResult {
    let shown: Vec<_> = (1..=24_u16).collect();
    let model = cube_with_materials(&shown)?
        .with_lod(TestModel::cube())
        .with_lod(cube_with_materials(&[25; 7])?);
    let base = model.cgfm();
    let level = model.lod_cgfm(2).ok_or("LOD 2 fixture geometry")?;
    let entries = vec![
        TestEntry::zstd(MAIN, model.cgf()),
        TestEntry::zstd(BASE, base.clone()),
        TestEntry::zstd(
            "Data/Objects/mesh_lod2.cgf",
            model.lod_cgf(2).ok_or("LOD 2 fixture model")?,
        ),
        TestEntry::zstd(LEVEL, level.clone()).encrypted(),
    ];
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        synthetic_site(entries),
        |builder| builder,
    )?;
    select(&mut harness, "mesh.cgf")?;
    let preview = model_preview(harness.state()).ok_or("model preview")?;
    let info = preview.info().ok_or("model facts")?;
    let displayed: Vec<_> = info
        .lods
        .iter()
        .map(|lod| (lod.level, lod.triangles))
        .collect();
    assert_eq!(
        displayed,
        [(0, 24), (2, 7)],
        "the omitted level did not create a row"
    );
    assert_eq!(
        info.material_ids.len(),
        25,
        "one material is omitted by the display"
    );
    assert_ivo_origins(provenance(preview)?, &base, &level, &shown)?;

    harness.get_by_label("Info").click();
    harness.run_steps(4);
    let listed: Vec<_> = shown.iter().map(u16::to_string).collect();
    let label = format!("{}, …", listed.join(", "));
    assert_marker(&mut harness, &label, "Source encryption unknown")?;
    assert!(
        harness.query_by_label("Listed material IDs").is_some(),
        "the tooltip scopes provenance to the displayed IDs"
    );
    reveal_info_row(&mut harness, "7")?;
    assert_marker(&mut harness, "7", "Includes encrypted source data")
}

fn assert_ivo_origins(
    proof: &ModelInfoProvenance,
    base: &[u8],
    level: &[u8],
    visible: &[u16],
) -> TestResult {
    let mesh = proof.mesh_summary();
    let [first, second] = mesh.lod_counts.as_slice() else {
        return Err("one origin row per displayed level".into());
    };
    for row in [first, second] {
        assert_eq!(
            row.vertices.contains_encrypted_source(),
            None,
            "no authored IVO vertex count"
        );
        assert!(
            row.vertices.source_uses_refs().is_empty(),
            "no substitute vertex source"
        );
    }
    assert_eq!(
        first.triangles.contains_encrypted_source(),
        None,
        "partial clear evidence"
    );
    assert_eq!(
        second.triangles.contains_encrypted_source(),
        Some(true),
        "encrypted LOD 2"
    );
    assert_runs(
        &first.triangles,
        proof.inventory(),
        BASE,
        "indexCount",
        base,
        &[3; 24],
    )?;
    assert_runs(
        &second.triangles,
        proof.inventory(),
        LEVEL,
        "indexCount",
        level,
        &[3; 7],
    )?;

    let ids = &mesh.material_ids;
    assert_eq!(
        ids.contains_encrypted_source(),
        None,
        "the encrypted ID is hidden"
    );
    let words: Vec<_> = visible.iter().copied().map(u32::from).collect();
    assert_runs(ids, proof.inventory(), BASE, "materialId", base, &words)
}

#[test]
fn crch_lod_counts_and_generated_material_have_their_own_origin_sites() -> TestResult {
    let bytes = TestCrchModel::cube().bytes();
    let original = ChunkFile::parse(bytes.as_slice())?;
    let offset = original
        .chunks()
        .get(3)
        .ok_or("physics fixture chunk")?
        .offset();
    let entries = vec![TestEntry::zstd(MAIN, bytes).encrypted()];
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        synthetic_site(entries),
        |builder| builder,
    )?;
    select(&mut harness, "mesh.cgf")?;
    let preview = model_preview(harness.state()).ok_or("model preview")?;
    let info = preview.info().ok_or("model facts")?;
    assert_eq!(
        info.material_ids,
        [0],
        "the generated material ID is listed"
    );
    let lod = info.lods.first().ok_or("CrCh LOD")?;
    assert_eq!(
        (lod.vertices, lod.triangles),
        (24, 12),
        "decoded CrCh counts"
    );
    let proof = provenance(preview)?;
    let mesh = proof.mesh_summary();
    let [row] = mesh.lod_counts.as_slice() else {
        return Err("one CrCh origin row".into());
    };
    for (origin, field, start) in [
        (&row.vertices, "vertexCount", offset + 96),
        (&row.triangles, "triangleCount", offset + 100),
    ] {
        assert_crch_count(origin, proof.inventory(), field, start)?;
    }
    let selector = uses_from(&mesh.material_ids, proof.inventory(), MAIN)?;
    let [site] = selector.as_slice() else {
        return Err("one generated material selector".into());
    };
    assert_eq!(
        site.role(),
        SourceUseRole::Selector,
        "generated ID selector"
    );
    assert!(
        matches!(site.location(), SourceLocation::Subresource { namespace, field, .. }
            if namespace == "cryEngineChunkHeader" && field == "signature"),
        "the container signature selects the generated ID"
    );
    assert_eq!(
        mesh.material_ids.contains_encrypted_source(),
        Some(true),
        "encrypted selector"
    );
    Ok(())
}

fn assert_crch_count(
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    name: &str,
    start: u64,
) -> TestResult {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "encrypted count"
    );
    let used = uses_from(origin, inventory, MAIN)?;
    assert_eq!(used.len(), 2, "one selector and one exact count");
    assert!(used.iter().any(|site| {
        site.role() == SourceUseRole::Selector
            && matches!(site.location(), SourceLocation::Subresource { field, .. } if field == "primitive")
    }), "the selected physics primitive");
    assert!(used.iter().any(|site| {
        site.role() == SourceUseRole::Conversion
            && matches!(site.location(), SourceLocation::Subresource { namespace, field, indices }
                if namespace == "model-byte-range" && field == name && indices.as_slice() == [start, start + 4])
    }), "the exact {name} word");
    Ok(())
}

fn assert_runs(
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    path: &str,
    name: &str,
    bytes: &[u8],
    expected: &[u32],
) -> TestResult {
    let mut words = Vec::new();
    for site in uses_from(origin, inventory, path)? {
        assert_eq!(site.scope(), InputScope::Payload, "decoded payload");
        assert_eq!(site.role(), SourceUseRole::Conversion, "value input");
        let SourceLocation::Subresource {
            namespace,
            field,
            indices,
        } = site.location()
        else {
            return Err("expected submesh coordinate".into());
        };
        assert_eq!(
            namespace, "model-submesh-words",
            "retained original word runs"
        );
        assert_eq!(field, name, "the submesh field");
        let [_, start, end, stride, count] = indices.as_slice() else {
            return Err("expected a finite word run".into());
        };
        for index in 0..*count {
            let offset = index * *stride;
            let word = bytes
                .get(usize::try_from(start + offset)?..usize::try_from(end + offset)?)
                .ok_or("word range in original fixture")?;
            words.push(match word {
                [a, b] => u32::from(u16::from_le_bytes([*a, *b])),
                [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
                _ => return Err("unexpected word width".into()),
            });
        }
    }
    words.sort_unstable();
    assert_eq!(
        words, expected,
        "all and only the actual original {name} words"
    );
    Ok(())
}

fn uses_from<'a>(
    origin: &FieldOrigin,
    inventory: &'a SourceInventory,
    path: &str,
) -> Result<Vec<&'a SourceUseDescription>, Box<dyn Error>> {
    origin
        .source_uses_refs()
        .iter()
        .map(|reference| {
            let used = inventory
                .iter_uses()
                .find(|used| used.id() == reference)
                .ok_or("source use")?;
            let source = inventory
                .iter_sources()
                .find(|source| source.id() == used.source())
                .ok_or("physical Entry")?;
            assert_eq!(
                serde_json::to_value(source)?
                    .pointer("/path/value")
                    .and_then(Value::as_str),
                Some(path),
                "the exact original archive Entry"
            );
            Ok(used)
        })
        .collect()
}

fn provenance(preview: &ModelPreview) -> Result<&ModelInfoProvenance, Box<dyn Error>> {
    preview
        .info_provenance()
        .ok_or("model info provenance")?
        .map_err(|error| format!("model info provenance: {error}").into())
}

fn cube_with_materials(ids: &[u16]) -> Result<TestModel, Box<dyn Error>> {
    let submeshes: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(index, &material_id)| {
            Ok(TestSubmesh {
                material_id,
                node: 1,
                first_index: u32::try_from(index)? * 3,
                index_count: 3,
                first_vertex: 0,
                vertex_count: 3,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(TestModel::cube()
        .with_indices([0, 1, 2].repeat(submeshes.len()))
        .with_submeshes(submeshes))
}

const fn synthetic_site(entries: Vec<TestEntry>) -> Site {
    Site {
        entries,
        installation: false,
        key: true,
    }
}
