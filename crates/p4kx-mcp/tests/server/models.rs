// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{AnimationSet, Archive, ModelInfo, ModelSource};
use p4kx_testkit::{TestEntry, TestModel, TestMtl, TestMtlMaterial};
use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

#[path = "model_overview_origins.rs"]
mod overview_origins;

const MODEL: &str = "Data/Objects/Test/cube.cgf";
const UNKNOWN_TYPE: u32 = 0x00C0_FFEE;

fn cube() -> TestModel {
    TestModel::cube().with_chunk(UNKNOWN_TYPE, [7; 12])
}

fn library() -> Vec<u8> {
    TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps").diffuse([0.8, 0.1, 0.1]),
        TestMtlMaterial::new("sides").diffuse([0.1, 0.1, 0.8]),
    ])
    .cryxml()
}

fn install() -> TestResult<Install> {
    Install::new(vec![
        TestEntry::zstd(MODEL, cube().cgf()),
        TestEntry::zstd("Data/Objects/Test/cube.cgfm", cube().cgfm()),
        TestEntry::zstd("Data/objects/test/CUBE.mtl", library()),
        TestEntry::zstd("Data/Objects/Test/lonely.cgf", cube().cgf()),
        TestEntry::stored("Data/Objects/Test/broken.cgf", b"not a chunk file".to_vec()),
        TestEntry::stored("Data/Objects/Test/readme.txt", b"hello".to_vec()),
    ])
}

pub(crate) fn all_pages(
    server: &mut Server,
    tool: &str,
    arguments: &Value,
    key: &str,
) -> TestResult<Vec<Value>> {
    let mut arguments = arguments.as_object().cloned().unwrap_or_default();
    let mut all = Vec::new();
    loop {
        let page = server.ok(tool, &Value::Object(arguments.clone()))?;
        all.extend(
            page.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
        match page.get("next_cursor").and_then(Value::as_str) {
            Some(cursor) => arguments.insert("cursor".to_owned(), json!(cursor)),
            None => return Ok(all),
        };
    }
}

#[test]
fn list_chunks_gives_container_and_per_chunk_type_version_offset_and_size() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let listing = server.ok(
        "list_chunks",
        &json!({ "path": "data\\objects\\test\\CUBE.cgf" }),
    )?;

    assert_eq!(
        listing["path"], MODEL,
        "the Entry path as the Archive has it"
    );
    assert_eq!(listing["container"], "#ivo", "container form");
    assert_eq!(listing["container_version"], 0x900, "container version");
    assert_eq!(listing["unknown_count"], 1, "one unknown chunk");
    let chunks = listing["chunks"].as_array().ok_or("chunks")?;
    assert_eq!(listing["chunk_count"], chunks.len(), "all chunks listed");
    assert_eq!(listing["total"], chunks.len(), "total");
    for (position, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk["index"], position, "table order");
        for field in [
            "type_id",
            "type_id_hex",
            "version",
            "byte_order",
            "offset",
            "size",
        ] {
            assert!(
                !chunk[field].is_null(),
                "{field} of chunk {position}: {chunk}"
            );
        }
    }
    let unknown = chunks
        .iter()
        .find(|chunk| chunk["type_id"] == UNKNOWN_TYPE)
        .ok_or("the unknown chunk")?;
    assert_eq!(unknown["known"], false, "not in the table");
    assert!(unknown["type"].is_null(), "no name: {unknown}");
    assert_eq!(unknown["size"], 12, "its data size");
    let known = chunks
        .iter()
        .find(|chunk| chunk["known"] == true)
        .ok_or("a known chunk")?;
    assert!(known["type"].is_string(), "a name: {known}");
    Ok(())
}

#[test]
fn list_chunks_pages_and_filters_by_type() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let whole = server.ok("list_chunks", &json!({ "path": MODEL }))?;
    let whole_chunks = whole["chunks"].as_array().ok_or("chunks")?.clone();

    let first = server.ok("list_chunks", &json!({ "path": MODEL, "limit": 2 }))?;
    assert_eq!(
        first["chunks"].as_array().map(Vec::len),
        Some(2),
        "page size"
    );
    assert!(first["next_cursor"].is_string(), "more follow");
    let paged = all_pages(
        &mut server,
        "list_chunks",
        &json!({ "path": MODEL, "limit": 2 }),
        "chunks",
    )?;
    assert_eq!(paged, whole_chunks, "the pages add up to the whole list");
    let other = server.error(
        "list_chunks",
        &json!({ "path": "Data/Objects/Test/lonely.cgf", "limit": 2, "cursor": first["next_cursor"] }),
    )?;
    assert!(
        other.contains("cursor"),
        "a cursor of another listing: {other}"
    );

    let by_id = server.ok(
        "list_chunks",
        &json!({ "path": MODEL, "type": "0x00C0FFEE" }),
    )?;
    assert_eq!(by_id["total"], 1, "one chunk of that type");
    assert_eq!(
        by_id["chunks"][0]["type_id"], UNKNOWN_TYPE,
        "the unknown chunk"
    );
    assert_eq!(
        by_id["chunk_count"], whole["chunk_count"],
        "the file's count stays"
    );
    let name = whole_chunks
        .iter()
        .find_map(|chunk| chunk["type"].as_str())
        .ok_or("a named chunk")?
        .to_lowercase();
    let by_name = server.ok("list_chunks", &json!({ "path": MODEL, "type": name }))?;
    assert!(
        by_name["total"].as_u64().is_some_and(|total| total >= 1),
        "the name matches ignoring case: {by_name}"
    );
    Ok(())
}

#[test]
fn list_chunks_reports_unknown_types_broken_files_and_missing_entries_clearly() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let unknown_type = server.error(
        "list_chunks",
        &json!({ "path": MODEL, "type": "NoSuchChunk" }),
    )?;
    assert!(
        unknown_type.contains("`NoSuchChunk`") && unknown_type.contains("hexadecimal type ID"),
        "{unknown_type}"
    );
    let bad_id = server.error("list_chunks", &json!({ "path": MODEL, "type": "0xZZ" }))?;
    assert!(bad_id.contains("`0xZZ`"), "{bad_id}");
    let broken = server.error(
        "list_chunks",
        &json!({ "path": "Data/Objects/Test/broken.cgf" }),
    )?;
    assert!(
        broken.contains("cannot read the Entry Data/Objects/Test/broken.cgf as a chunk file"),
        "{broken}"
    );
    let missing = server.error(
        "list_chunks",
        &json!({ "path": "Data/Objects/Missing.cgf" }),
    )?;
    assert!(missing.contains("holds no Entry"), "{missing}");
    let long = server.error("list_chunks", &json!({ "path": "a".repeat(2000) }))?;
    assert!(long.contains("longer than"), "{long}");
    Ok(())
}

#[test]
fn get_model_info_summarises_the_model() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let info = server.ok(
        "get_model_info",
        &json!({ "path": "data/objects/test/CUBE.cgf" }),
    )?;

    assert_eq!(info["model"], MODEL, "Entry path");
    assert_eq!(info["geometry"], "cube.cgfm", "companion");
    assert_eq!(info["material_library"], "objects/test/cube", "library");
    assert_eq!(info["material_ids"], json!([1, 2]), "IDs");
    assert_eq!(info["library"]["status"], "loaded", "library status");
    assert_eq!(
        info["lods"],
        json!([{ "level": 0, "vertices": 24, "triangles": 12, "meshes": 1 }])
    );
    assert_eq!(info["counts"]["nodes"], 2, "nodes");
    assert_eq!(info["counts"]["meshes"], 1, "meshes");
    assert_eq!(info["counts"]["lods"], 1, "lods");
    assert_eq!(info["counts"]["bones"], 0, "bones");
    assert_eq!(info["counts"]["animations"], 0, "animations");
    assert_eq!(info["counts"]["materials"], 3, "materials");
    let sources = info
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("model summary source inventory")?;
    for output in ["counts/materials", "library/single"] {
        let uses = crate::model_item_origin_checks::uses(&info, output)?;
        assert!(
            uses.iter().any(|site| sources.iter().any(|source| {
                source.get("id") == site.get("source")
                    && source.pointer("/path/value").and_then(Value::as_str)
                        == Some("Data/objects/test/CUBE.mtl")
            })),
            "{output} includes the selected material library Entry"
        );
    }
    assert!(
        info["counts"]["chunks"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "chunks"
    );
    Ok(())
}

#[test]
fn model_items_equal_what_model_info_json_holds() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let archive = Archive::open(install.archive())?;
    let source = ModelSource::from_archive(&archive, MODEL)?;
    let expected = serde_json::to_value(ModelInfo::of(&source, &AnimationSet::default()))?;

    for (section, key) in [
        ("materials", "materials"),
        ("nodes", "nodes"),
        ("bones", "bones"),
        ("animations", "animations"),
        ("unread_animations", "unread_animations"),
        ("chunks", "chunks"),
    ] {
        let items = all_pages(
            &mut server,
            "list_model_items",
            &json!({ "path": MODEL, "section": section, "limit": 1 }),
            "items",
        )?;
        assert_eq!(Value::Array(items), expected[key], "section {section}");
    }
    let meshes = all_pages(
        &mut server,
        "list_model_items",
        &json!({ "path": MODEL, "section": "meshes" }),
        "items",
    )?;
    assert_eq!(meshes.len(), 1, "one mesh");
    assert_eq!(meshes[0]["lod"], 0, "its level");
    let mut mesh = meshes[0].clone();
    if let Some(object) = mesh.as_object_mut() {
        object.remove("lod");
    }
    assert_eq!(
        mesh, expected["lods"][0]["meshes"][0],
        "the mesh as the CLI shows it"
    );
    assert_eq!(
        meshes[0]["submeshes"][1],
        json!({ "material_id": 2, "first_index": 12, "index_count": 24 }),
        "a submesh"
    );
    Ok(())
}

#[test]
fn model_items_page_and_mark_unknown_chunks() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let first = server.ok(
        "list_model_items",
        &json!({ "path": MODEL, "section": "nodes", "limit": 1 }),
    )?;
    assert_eq!(first["total"], 2, "two nodes");
    assert_eq!(first["section"], "nodes", "the section");
    assert!(first["next_cursor"].is_string(), "a second page");
    let second = server.ok(
        "list_model_items",
        &json!({ "path": MODEL, "section": "nodes", "limit": 1, "cursor": first["next_cursor"] }),
    )?;
    assert_eq!(second["items"][0]["name"], "cube_mesh", "the second node");
    assert!(second["next_cursor"].is_null(), "the last page");
    let wrong = server.error(
        "list_model_items",
        &json!({ "path": MODEL, "section": "bones", "cursor": first["next_cursor"] }),
    )?;
    assert!(
        wrong.contains("cursor"),
        "a cursor of another section: {wrong}"
    );

    let chunks = all_pages(
        &mut server,
        "list_model_items",
        &json!({ "path": MODEL, "section": "chunks" }),
        "items",
    )?;
    let unknown: Vec<&Value> = chunks
        .iter()
        .filter(|chunk| chunk["usage"] == "unknown")
        .collect();
    assert_eq!(unknown.len(), 1, "one chunk of unknown type: {unknown:?}");
    assert_eq!(unknown[0]["type_id"], UNKNOWN_TYPE, "its type");
    assert!(unknown[0]["type"].is_null(), "without a name");
    Ok(())
}

#[test]
fn model_tools_give_clear_errors() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let broken = server.error(
        "get_model_info",
        &json!({ "path": "Data/Objects/Test/broken.cgf" }),
    )?;
    assert!(
        broken.contains("cannot read the Entry Data/Objects/Test/broken.cgf as a model"),
        "{broken}"
    );
    let lonely = server.error(
        "get_model_info",
        &json!({ "path": "Data/Objects/Test/lonely.cgf" }),
    )?;
    assert!(
        lonely.contains("lonely.cgf") && lonely.contains("as a model"),
        "a model without its geometry companion: {lonely}"
    );
    let text = server.error(
        "list_model_items",
        &json!({ "path": "Data/Objects/Test/readme.txt", "section": "nodes" }),
    )?;
    assert!(text.contains("as a model"), "{text}");
    let missing = server.error(
        "get_model_info",
        &json!({ "path": "Data/Objects/Missing.cgf" }),
    )?;
    assert!(missing.contains("holds no Entry"), "{missing}");
    let section = server.request(
        "tools/call",
        &json!({ "name": "list_model_items", "arguments": { "path": MODEL, "section": "polygons" } }),
    )?;
    assert!(
        section.get("error").is_some()
            || section.pointer("/result/isError") == Some(&Value::Bool(true)),
        "an unknown section is refused: {section}"
    );
    Ok(())
}
