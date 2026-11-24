// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestCrchModel, TestEntry, TestModel, TestSubmesh};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{assert_all_addresses, field, marker, uses};
use crate::support::{Install, TestResult};

const MODEL: &str = "Data/overview.cgf";
const BASE: &str = "Data/overview.cgfm";
const LOD: &str = "Data/overview_lod1.cgfm";
const ROW_STRIDE: u64 = 48;
const INDEX_COUNT: u32 = 3;

#[test]
fn model_overview_tracks_each_original_entry_and_exact_field_paths() -> TestResult {
    for encrypt_base in [false, true] {
        let install = install(&TestModel::cube(), encrypt_base)?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let response = server.call("get_model_info", &json!({"path": "data/OVERVIEW.cgf"}))?;
        let model = response.get("structuredContent").ok_or("model overview")?;
        assert_all_addresses(&response)?;
        assert_eq!(model.get("model"), Some(&json!(MODEL)), "selected path");
        assert_eq!(
            model.get("material_ids"),
            Some(&json!([1, 2])),
            "sorted unique IDs"
        );
        marker(field(model, "model")?, false);
        let name = field(model, "material_library")?;
        assert_marker(name, encrypt_base);
        assert_eq!(field(model, "library/path")?, name, "same authored name");
        assert!(
            !uses(model, "counts/nodes")?.is_empty(),
            "original node selection"
        );
        for (level, entry, encrypted) in [(0, BASE, encrypt_base), (1, LOD, !encrypt_base)] {
            assert_triangles(model, level, entry, encrypted, 12, 2)?;
            for item in 0..2 {
                let output = format!("material_ids/{item}");
                assert_marker(field(model, &output)?, true);
                assert_eq!(
                    original_words(model, &output, entry, "materialId", 2)?.len(),
                    1,
                    "each ID occurs once in each original Entry",
                );
            }
        }
        let tools = server.tools()?;
        let required = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some("get_model_info"))
            .and_then(|tool| tool.pointer("/outputSchema/required"))
            .and_then(Value::as_array)
            .ok_or("required model overview schema fields")?;
        for name in ["fieldOrigins", "provenance"] {
            assert!(required.contains(&json!(name)), "schema requires {name}");
        }
    }
    Ok(())
}

#[test]
fn model_overview_keeps_every_original_count_and_material_word_within_the_budget() -> TestResult {
    const COUNT: u32 = 1_024;
    let install = install(&cube_with_materials((0..COUNT).map(|_| 1))?, false)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let model = server.ok("get_model_info", &json!({"path": MODEL}))?;
    assert!(
        serde_json::to_vec(&model)?.len() <= 256 * 1024,
        "shared response limit"
    );
    for (level, entry, encrypted) in [(0, BASE, false), (1, LOD, true)] {
        assert_triangles(&model, level, entry, encrypted, COUNT, COUNT as usize)?;
        assert_eq!(
            original_words(&model, "material_ids/0", entry, "materialId", 2)?.len(),
            COUNT as usize,
            "all original material words fit in their exact run",
        );
    }
    Ok(())
}

#[test]
fn model_overview_distinguishes_runs_with_gaps_and_generated_ids() -> TestResult {
    let fixture = install(&cube_with_materials([1, 1, 2, 1, 2])?, false)?;
    let mut server = fixture.serve(&["--archive", "LIVE"])?;
    let model = server.ok("get_model_info", &json!({"path": MODEL}))?;
    for entry in [BASE, LOD] {
        let first = original_words(&model, "material_ids/0", entry, "materialId", 2)?;
        let second = original_words(&model, "material_ids/1", entry, "materialId", 2)?;
        let start = *first.first().ok_or("first original word")?;
        let at = |positions: &[u64]| {
            positions
                .iter()
                .map(|position| start + ROW_STRIDE * position)
                .collect::<Vec<_>>()
        };
        assert_eq!(first, at(&[0, 1, 3]), "material 1 only");
        assert_eq!(second, at(&[2, 4]), "material 2 only");
    }
    let crch = Install::new(vec![
        TestEntry::zstd(MODEL, TestCrchModel::cube().bytes()).encrypted(),
    ])?;
    let mut server = crch.serve(&["--archive", "LIVE"])?;
    let generated = server.ok("get_model_info", &json!({"path": MODEL}))?;
    assert_eq!(
        generated.get("material_ids"),
        Some(&json!([0])),
        "generated CrCh ID"
    );
    assert_marker(field(&generated, "material_ids/0")?, true);
    let sites = uses(&generated, "material_ids/0")?;
    let [selector] = sites.as_slice() else {
        return Err("one original layout selector".into());
    };
    assert_eq!(
        selector.get("role"),
        Some(&json!("selector")),
        "layout selector"
    );
    assert_eq!(
        selector.pointer("/location/namespace"),
        Some(&json!("cryEngineChunkHeader")),
        "source format"
    );
    assert_eq!(
        selector.pointer("/location/field"),
        Some(&json!("signature")),
        "generated value has no authored word"
    );
    Ok(())
}

fn cube_with_materials(ids: impl IntoIterator<Item = u16>) -> TestResult<TestModel> {
    let submeshes: Vec<_> = ids
        .into_iter()
        .enumerate()
        .map(|(position, material_id)| {
            Ok(TestSubmesh {
                material_id,
                node: 1,
                first_index: u32::try_from(position)? * INDEX_COUNT,
                index_count: INDEX_COUNT,
                first_vertex: 0,
                vertex_count: 3,
            })
        })
        .collect::<TestResult<Vec<_>>>()?;
    Ok(TestModel::cube()
        .with_indices([0, 1, 2].repeat(submeshes.len()))
        .with_submeshes(submeshes))
}

fn install(cube: &TestModel, encrypt_base: bool) -> TestResult<Install> {
    let model = cube.clone().with_lod(cube.clone());
    let primary = TestEntry::zstd(MODEL, model.cgf());
    let base = TestEntry::zstd(BASE, model.cgfm());
    let lod = TestEntry::zstd(LOD, model.lod_cgfm(1).ok_or("LOD geometry")?);
    Install::new(vec![
        if encrypt_base {
            primary.encrypted()
        } else {
            primary
        },
        if encrypt_base { base.encrypted() } else { base },
        TestEntry::zstd(
            "Data/overview_lod1.cgf",
            model.lod_cgf(1).ok_or("LOD model")?,
        ),
        if encrypt_base { lod } else { lod.encrypted() },
    ])
}

fn assert_triangles(
    model: &Value,
    level: u32,
    entry: &str,
    encrypted: bool,
    triangles: u32,
    count: usize,
) -> TestResult {
    let path = format!("lods/{level}/triangles");
    assert_eq!(
        model.pointer(&format!("/{path}")),
        Some(&json!(triangles)),
        "unchanged triangle total"
    );
    assert_marker(field(model, &path)?, encrypted);
    let words = original_words(model, &path, entry, "indexCount", 4)?;
    assert_eq!(words.len(), count, "all original count coordinates");
    assert!(
        words
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if right - left == ROW_STRIDE)),
        "source words retain the proved row stride",
    );
    Ok(())
}

fn assert_marker(origin: &Value, encrypted: bool) {
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(false)),
        "incomplete selection"
    );
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&if encrypted { json!(true) } else { Value::Null }),
        "known encryption survives incomplete evidence",
    );
}

fn original_words(
    model: &Value,
    output: &str,
    entry: &str,
    label: &str,
    width: u64,
) -> TestResult<Vec<u64>> {
    let sources = model
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let mut words = Vec::new();
    for site in uses(model, output)? {
        let source = sources
            .iter()
            .find(|row| row.get("id") == site.get("source"))
            .ok_or("contributing geometry")?;
        if source.pointer("/path/value").and_then(Value::as_str) != Some(entry) {
            continue;
        }
        assert_eq!(
            site.get("role"),
            Some(&json!("conversion")),
            "original word use role"
        );
        assert_eq!(
            site.pointer("/location/namespace"),
            Some(&json!("model-submesh-words")),
            "decoded word domain"
        );
        assert_eq!(
            site.pointer("/location/field").and_then(Value::as_str),
            Some(label),
            "decoded word field"
        );
        let indices = site
            .pointer("/location/indices")
            .and_then(Value::as_array)
            .ok_or("word run")?;
        let [chunk, start, end, stride, count] = indices.as_slice() else {
            return Err("expected exact word run".into());
        };
        let parsed = |cell: &Value| -> TestResult<u64> {
            Ok(cell.as_str().ok_or("decimal coordinate")?.parse()?)
        };
        let _ordinal = parsed(chunk)?;
        let start = parsed(start)?;
        let end = parsed(end)?;
        let stride = parsed(stride)?;
        assert_eq!(end.checked_sub(start), Some(width), "actual word width");
        assert_eq!(stride, ROW_STRIDE, "original layout stride");
        words.extend((0..parsed(count)?).map(|index| start + index * stride));
    }
    words.sort_unstable();
    Ok(words)
}
