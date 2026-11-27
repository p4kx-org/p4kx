// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, TestCrchModel, TestEntry, TestModel, TestSubmesh, p4kx, test_key,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MODEL: &str = "Data/Objects/Test/cube.cgf";
const BASE: &str = "Data/Objects/Test/cube.cgfm";
const LOD_MODEL: &str = "Data/Objects/Test/cube_lod1.cgf";
const LOD: &str = "Data/Objects/Test/cube_lod1.cgfm";
const TABLE_START_WORD: Range<usize> = 12..16;
const TABLE_ROW_BYTES: usize = 16;
const UNKNOWN_TYPE: u32 = 0x00C0_FFEE;

#[test]
fn model_info_binds_original_submesh_words_to_their_actual_levels() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let later = TestModel::cube()
        .with_submeshes(vec![submesh(1, 0, 3), submesh(0, 3, 6), submesh(1, 9, 12)])
        .with_geometry_chunk(UNKNOWN_TYPE, [7; 12]);
    let model = TestModel::cube().with_lod(later);
    let base = model.cgfm();
    let lod = model.lod_cgfm(1).ok_or("LOD geometry")?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.cgf()))
        .entry(TestEntry::zstd(BASE, base.clone()))
        .entry(TestEntry::zstd(
            LOD_MODEL,
            model.lod_cgf(1).ok_or("LOD model")?,
        ))
        .entry(TestEntry::zstd(LOD, lod.clone()).encrypted())
        .write_to(&path)?;
    let info = listing(&path, Some(MODEL))?;
    let unknown = info["chunks"]
        .as_array()
        .and_then(|chunks| chunks.iter().find(|chunk| chunk["type_id"] == UNKNOWN_TYPE))
        .ok_or("the original unknown chunk")?;
    assert_eq!(unknown["file"], "lod1-geometry", "encrypted companion");
    assert_eq!(
        unknown["type"],
        Value::Null,
        "unrecognized original type ID"
    );
    let levels = [
        ("geometry", BASE, &base, false),
        ("lod1-geometry", LOD, &lod, true),
    ];
    for (level, (role, entry, bytes, encrypted)) in levels.into_iter().enumerate() {
        let submesh = format!("/lods/{level}/meshes/0/submeshes/0");
        let geometry = Geometry {
            entry,
            bytes,
            encrypted,
        };
        assert_word(&info, &submesh, ("material_id", "materialId", 2), &geometry)?;
        assert_unknown(&info, &format!("{submesh}/first_index"))?;
        let expected: &[usize] = if level == 0 { &[12] } else { &[2, 5] };
        assert_triangle_counts(&info, level, &geometry, expected)?;
        assert_chunk_words(&info, role, &geometry, 4)?;
    }
    Ok(())
}

#[test]
fn model_info_binds_crch_chunk_type_ids_to_two_original_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let bytes = TestCrchModel::cube().bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, bytes.clone()).encrypted())
        .write_to(&path)?;
    let info = listing(&path, Some(MODEL))?;
    assert_unknown(&info, "/lods/0/meshes/0/triangles")?;
    assert_unknown(&info, "/lods/0/triangles")?;
    assert_chunk_words(
        &info,
        "model",
        &Geometry {
            entry: MODEL,
            bytes: &bytes,
            encrypted: true,
        },
        2,
    )?;
    Ok(())
}

#[test]
fn model_info_reports_unknown_origin_for_loose_submesh_fields() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("cube.cgf");
    let model = TestModel::cube();
    fs::write(&path, model.cgf())?;
    fs::write(dir.path().join("cube.cgfm"), model.cgfm())?;
    let info = listing(&path, None)?;
    let submesh = "/lods/0/meshes/0/submeshes/0";
    for name in ["material_id", "index_count", "first_index"] {
        assert_unknown(&info, &format!("{submesh}/{name}"))?;
    }
    for name in ["type_id", "type", "index", "file", "usage"] {
        assert_unknown(&info, &format!("/chunks/0/{name}"))?;
    }
    assert_unknown(&info, "/lods/0/meshes/0/triangles")?;
    assert_unknown(&info, "/lods/0/triangles")?;
    assert_eq!(
        info.get("provenance"),
        Some(&json!({"sources": [], "uses": []}))
    );
    Ok(())
}

const fn submesh(node: u16, first_index: u32, index_count: u32) -> TestSubmesh {
    TestSubmesh {
        material_id: 1,
        node,
        first_index,
        index_count,
        first_vertex: 0,
        vertex_count: 24,
    }
}

fn assert_triangle_counts(
    info: &Value,
    level: usize,
    geometry: &Geometry<'_>,
    expected: &[usize],
) -> TestResult {
    let parent = format!("/lods/{level}");
    let rows = info
        .pointer(&format!("{parent}/meshes"))
        .and_then(Value::as_array)
        .ok_or("LOD meshes")?;
    assert_eq!(rows.len(), expected.len(), "all emitted meshes");
    let mut lod_words = Vec::new();
    for (index, (mesh, &triangles)) in rows.iter().zip(expected).enumerate() {
        let mesh_path = format!("{parent}/meshes/{index}");
        let count = mesh["submeshes"].as_array().ok_or("mesh submeshes")?.len();
        let mut mesh_words = Vec::new();
        for ordinal in 0..count {
            mesh_words.push(assert_word(
                info,
                &format!("{mesh_path}/submeshes/{ordinal}"),
                ("index_count", "indexCount", 4),
                geometry,
            )?);
        }
        assert_triangle_words(info, &mesh_path, &mesh_words, geometry.encrypted, triangles)?;
        lod_words.extend(mesh_words);
    }
    assert_triangle_words(
        info,
        &parent,
        &lod_words,
        geometry.encrypted,
        expected.iter().sum(),
    )
}

fn assert_triangle_words(
    info: &Value,
    parent: &str,
    expected: &[Range<usize>],
    encrypted: bool,
    triangles: usize,
) -> TestResult {
    let path = format!("{parent}/triangles");
    assert_eq!(info.pointer(&path), Some(&json!(triangles)), "{path} value");
    let origin = field_origin(info, &path)?;
    assert_eq!(origin["sourceSetComplete"], false, "{path} selectors");
    assert_eq!(
        origin["containsEncryptedSource"].as_bool(),
        encrypted.then_some(true),
        "{path} encryption",
    );
    let ids = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("triangle input uses")?;
    assert_eq!(ids.len(), expected.len(), "count words at {path}");
    for id in ids {
        let row = origin_row(info, "/provenance/uses", id)?;
        assert_eq!(row["role"], "conversion", "{path} computes the count");
        let word = row.get("location").ok_or("conversion word")?;
        assert_eq!(word["field"], "indexCount", "{path} original input");
        assert!(
            expected
                .iter()
                .any(|range| word["indices"]
                    == json!([range.start.to_string(), range.end.to_string(),])),
            "{path} geometry word {word}",
        );
    }
    Ok(())
}

fn listing(path: &Path, entry: Option<&str>) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx()?;
    command
        .args(["--json", "--key", &test_key().to_hex(), "model", "info"])
        .arg(path);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let result = command.output()?;
    assert!(result.status.success(), "model info: {result:?}");
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn field_origin<'a>(info: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    assert!(info.pointer(path).is_some(), "emitted model field {path}");
    info.get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing origin at {path}").into())
}

fn assert_unknown(info: &Value, path: &str) -> TestResult {
    assert_eq!(
        field_origin(info, path)?,
        &json!({"containsEncryptedSource": null, "sourceSetComplete": false, "sourceUsesRefs": []}),
        "the evidence at {path} must stay unknown"
    );
    Ok(())
}

struct Geometry<'a> {
    entry: &'a str,
    bytes: &'a [u8],
    encrypted: bool,
}

fn assert_chunk_words(
    info: &Value,
    role: &str,
    geometry: &Geometry<'_>,
    width: usize,
) -> TestResult {
    let table_start = usize::try_from(u32::from_le_bytes(
        geometry
            .bytes
            .get(TABLE_START_WORD)
            .ok_or("table offset")?
            .try_into()?,
    ))?;
    let chunks = info["chunks"].as_array().ok_or("model chunks")?;
    let mut file_rows = 0;
    for (ordinal, chunk) in chunks.iter().enumerate() {
        if chunk["file"] != role {
            continue;
        }
        let index = usize::try_from(chunk["index"].as_u64().ok_or("chunk index")?)?;
        assert_eq!(index, file_rows, "physical ordinal within {role}");
        let start = table_start + index * TABLE_ROW_BYTES;
        let path = format!("/chunks/{ordinal}");
        let range = assert_word(info, &path, ("type_id", "typeId", width), geometry)?;
        assert_eq!(range, start..start + width, "original table row for {path}");
        assert_derived_chunk_words(info, &path, geometry.encrypted)?;
        file_rows += 1;
    }
    assert!(file_rows > 0, "{role} chunk words are tested");
    Ok(())
}

fn assert_derived_chunk_words(info: &Value, parent: &str, encrypted: bool) -> TestResult {
    let numeric = field_origin(info, &format!("{parent}/type_id"))?;
    let id = numeric
        .pointer("/sourceUsesRefs/0")
        .ok_or("the original numeric kind reference")?;
    let kind = origin_row(info, "/provenance/uses", id)?;
    let owner = kind.get("source").ok_or("numeric kind physical owner")?;
    let signature = [("signature", ["0", "4"])];
    let addressing = [("rowCount", ["8", "12"]), ("tableOffset", ["12", "16"])];
    for (field, words, contains_kind) in [
        ("type", signature.as_slice(), true),
        ("index", addressing.as_slice(), false),
    ] {
        let path = format!("{parent}/{field}");
        let origin = field_origin(info, &path)?;
        assert_eq!(
            origin["containsEncryptedSource"], encrypted,
            "{path} marker"
        );
        assert_eq!(origin["sourceSetComplete"], true, "{path} source set");
        let ids = origin["sourceUsesRefs"]
            .as_array()
            .ok_or("chunk origin uses")?;
        assert_eq!(
            ids.len(),
            words.len() + usize::from(contains_kind),
            "{path} uses"
        );
        assert_eq!(
            ids.contains(id),
            contains_kind,
            "{path} uses the numeric kind"
        );
        let sites = ids
            .iter()
            .map(|id| origin_row(info, "/provenance/uses", id))
            .collect::<Result<Vec<_>, _>>()?;
        for (field, bytes) in words {
            let range = json!(bytes);
            assert!(
                sites.iter().any(|site| {
                    site.get("source") == Some(owner)
                        && site.get("role").and_then(Value::as_str) == Some("payload")
                        && site.pointer("/location/namespace").and_then(Value::as_str)
                            == Some("cryEngineChunkHeader")
                        && site.pointer("/location/field").and_then(Value::as_str) == Some(*field)
                        && site.pointer("/location/indices") == Some(&range)
                }),
                "{path} has the exact {field} range in its owning file"
            );
        }
    }
    Ok(())
}

fn assert_word(
    info: &Value,
    parent: &str,
    field: (&str, &str, usize),
    geometry: &Geometry<'_>,
) -> Result<Range<usize>, Box<dyn Error>> {
    let (output, original, width) = field;
    let path = format!("{parent}/{output}");
    let origin = field_origin(info, &path)?;
    assert_eq!(
        origin["containsEncryptedSource"], geometry.encrypted,
        "source at {path}"
    );
    assert_eq!(origin["sourceSetComplete"], true, "original word at {path}");
    let uses = origin["sourceUsesRefs"].as_array().ok_or("source uses")?;
    let [id] = uses.as_slice() else {
        return Err(format!("the field {path} must address one original word").into());
    };
    let use_row = origin_row(info, "/provenance/uses", id)?;
    assert_eq!(use_row["role"], "payload", "the word supplies {path}");
    let location = use_row.get("location").ok_or("original word location")?;
    assert_eq!(
        location.get("field").and_then(Value::as_str),
        Some(original),
        "the word for {path}"
    );
    assert_eq!(
        location.get("namespace").and_then(Value::as_str),
        Some("model-byte-range"),
        "the address for {path}"
    );
    let source = origin_row(info, "/provenance/sources", &use_row["source"])?;
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(geometry.entry),
        "geometry for {path}"
    );
    let indices = location
        .get("indices")
        .and_then(Value::as_array)
        .ok_or("byte range")?;
    let [start, end] = indices.as_slice() else {
        return Err("expected two original byte coordinates".into());
    };
    let start: usize = start.as_str().ok_or("start decimal")?.parse()?;
    let end: usize = end.as_str().ok_or("end decimal")?.parse()?;
    assert_eq!(
        end.checked_sub(start),
        Some(width),
        "original word width at {path}"
    );
    let word = geometry.bytes.get(start..end).ok_or("geometry word")?;
    let mut widened = [0_u8; size_of::<u32>()];
    widened
        .get_mut(..width)
        .ok_or("model word width")?
        .copy_from_slice(word);
    assert_eq!(
        info.pointer(&path),
        Some(&json!(u32::from_le_bytes(widened))),
        "the original value at {path}"
    );
    Ok(start..end)
}

fn origin_row<'a>(info: &'a Value, path: &str, id: &Value) -> Result<&'a Value, Box<dyn Error>> {
    info.pointer(path)
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(id)))
        .ok_or_else(|| format!("unresolved origin id {id} in {path}").into())
}
