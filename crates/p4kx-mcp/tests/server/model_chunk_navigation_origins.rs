// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ChunkFile;
use p4kx_testkit::{TestChunkFile, TestCrchModel, TestEntry, TestModel};
use serde_json::{Value, json};

use crate::audio_origin_assertions as inventory;
use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

const CRCH: &str = "Data/cube.cgf";
const MODEL: &str = "Data/rig.skin";
const GEOMETRY: &str = "Data/rig.skinm";
const LOD_MODEL: &str = "Data/rig_lod1.skin";
const LOD_GEOMETRY: &str = "Data/rig_lod1.skinm";

#[test]
fn chunk_navigation_counts_even_an_empty_admitted_companion() -> TestResult {
    let skin = TestModel::two_bone_skin();
    let model = skin.cgf();
    let geometry = skin.cgfm();
    let empty = TestChunkFile::ivo().bytes();
    let counts = [
        (MODEL, declared_count(&model)?),
        (GEOMETRY, declared_count(&geometry)?),
        (LOD_MODEL, declared_count(&empty)?),
        (LOD_GEOMETRY, declared_count(&geometry)?),
    ];
    assert_eq!(
        counts.get(2).map(|(_, count)| *count),
        Some(0),
        "empty admitted companion"
    );
    let install = Install::new(vec![
        TestEntry::zstd(MODEL, model),
        TestEntry::zstd(GEOMETRY, geometry.clone()),
        TestEntry::zstd(LOD_MODEL, empty).encrypted(),
        TestEntry::zstd(LOD_GEOMETRY, geometry),
        TestEntry::zstd("Data/unrelated.skinm", b"unselected").encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": MODEL, "section": "chunks", "limit": 1}),
    )?;
    let page = response.get("structuredContent").ok_or("chunk page")?;
    assert_all_addresses(&response)?;
    assert_eq!(
        page.get("total").and_then(Value::as_u64),
        Some(counts.iter().map(|(_, count)| count).sum()),
        "all parsed file tables contribute to the total exactly once"
    );
    assert_eq!(page.pointer("/items/0/file"), Some(&json!("model")));
    assert!(page.get("next_cursor").is_some(), "multiple physical rows");
    assert_eq!(field(page, "total")?, field(page, "next_cursor")?);
    assert_count_words(page, "total", &counts, true)?;
    for name in ["archive", "model", "section"] {
        marker(field(page, name)?, false);
    }
    let item_inputs = uses(page, "items")?;
    assert!(!item_inputs.is_empty(), "the first row has original input");
    for site in item_inputs {
        assert_eq!(
            inventory::source_path(page, site)?,
            MODEL,
            "local item scope"
        );
    }
    let overview = server.ok("get_model_info", &json!({"path": MODEL}))?;
    assert_eq!(overview.pointer("/counts/chunks"), page.get("total"));
    assert_count_words(&overview, "counts/chunks", &counts, true)?;
    Ok(())
}

#[test]
fn crch_counts_use_the_same_header_words_without_claiming_other_files() -> TestResult {
    let model = TestCrchModel::cube().bytes();
    let counts = [(CRCH, declared_count(&model)?)];
    let install = Install::new(vec![TestEntry::stored(CRCH, model).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": CRCH, "section": "chunks"}),
    )?;
    let page = response.get("structuredContent").ok_or("CrCh chunk page")?;
    assert_all_addresses(&response)?;
    assert_count_words(page, "total", &counts, true)?;
    assert!(
        page.get("next_cursor").is_none(),
        "complete small CrCh page"
    );
    assert!(
        field(page, "next_cursor").is_err(),
        "no origin at an absent field"
    );
    Ok(())
}

#[test]
fn empty_chunk_pages_preserve_the_original_zero_and_require_their_carrier() -> TestResult {
    let empty = TestChunkFile::ivo().bytes();
    assert_eq!(declared_count(&empty)?, 0);
    let install = Install::new(vec![TestEntry::stored(MODEL, empty).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": MODEL, "section": "chunks"}),
    )?;
    let page = response
        .get("structuredContent")
        .ok_or("empty chunk page")?;
    assert_all_addresses(&response)?;
    assert_eq!(page.get("total"), Some(&json!(0)));
    assert_eq!(page.get("items"), Some(&json!([])));
    assert_eq!(field(page, "items")?, field(page, "total")?);
    assert_count_words(page, "total", &[(MODEL, 0)], true)?;
    assert!(
        field(page, "next_cursor").is_err(),
        "no origin for absent cursor"
    );

    let tools = server.tools()?;
    let schema = tools
        .iter()
        .find(|tool| tool.get("name") == Some(&json!("list_model_items")))
        .and_then(|tool| tool.get("outputSchema"))
        .ok_or("MCP chunk item schema")?;
    let validator = jsonschema::validator_for(schema)?;
    assert!(
        validator.is_valid(page),
        "the emitted chunk page satisfies its schema"
    );
    let mut missing_origin = page.clone();
    missing_origin
        .as_object_mut()
        .ok_or("chunk page fields")?
        .remove("fieldOrigins");
    assert!(
        !validator.is_valid(&missing_origin),
        "current chunk pages require their origins"
    );
    Ok(())
}

fn declared_count(bytes: &[u8]) -> TestResult<u64> {
    let count = bytes.get(8..12).ok_or("declared table count")?.try_into()?;
    let count = u32::from_le_bytes(count);
    let parsed = ChunkFile::parse(bytes)?;
    assert_eq!(
        u64::from(count),
        parsed.chunks().len() as u64,
        "fixture's original count"
    );
    Ok(u64::from(count))
}

fn assert_count_words(
    page: &Value,
    address: &str,
    counts: &[(&str, u64)],
    encrypted: bool,
) -> TestResult {
    let origin = field(page, address)?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "the retained template count evidence must report its known encryption"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(false)),
        "counts with logical part names have an incomplete composition witness"
    );
    for (label, a, b) in [
        ("signature", "0", "4"),
        ("version", "4", "8"),
        ("rowCount", "8", "12"),
        ("tableOffset", "12", "16"),
    ] {
        assert_site(page, address, "cryEngineChunkHeader", label, &[a, b])?;
    }
    let mut paths = Vec::new();
    for site in uses(page, address)? {
        let path = inventory::source_path(page, site)?;
        assert!(
            counts.iter().any(|(expected, _)| path == *expected),
            "only admitted files"
        );
        if site.pointer("/location/field") == Some(&json!("rowCount")) {
            assert_eq!(
                site.get("role"),
                Some(&json!("conversion")),
                "count word role"
            );
            paths.push(path);
        }
    }
    paths.sort_unstable();
    let mut expected: Vec<_> = counts.iter().map(|(path, _)| *path).collect();
    expected.sort_unstable();
    assert_eq!(
        paths, expected,
        "one physical count word per original table"
    );
    Ok(())
}
