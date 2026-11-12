// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestChunkFile, TestEntry, TestModel, TestSkeleton};
use serde_json::{Value, json};

use crate::model_bone_origin_checks::{
    assert_all_addresses, assert_bone_span, assert_compiled_2_tip_sites, assert_prefix,
    assert_retained_bone_positions, assert_site, field, marker, uses,
};
use crate::support::{Install, TestResult};

const PRIMARY: &str = "Data/skeleton.chr";
const COMPILED: u32 = 0xC201_973C;
const COMPILED_2: u32 = 0xC201_1111;
const UNRELATED: u32 = 0x1234_5678;
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn preferred_bone_layout_uses_the_first_selected_chunk_and_stored_pose_words() -> TestResult {
    let mut stored = TestSkeleton::two_bones().compiled_bones_2();
    let tip_world_x = 128..132;
    stored
        .get_mut(tip_world_x)
        .ok_or("fixture world word")?
        .copy_from_slice(&7.5_f32.to_le_bytes());
    let file = TestChunkFile::ivo()
        .chunk(COMPILED, 0, [1])
        .chunk(UNRELATED, 0, [2])
        .chunk(COMPILED_2, 0, stored)
        .chunk(COMPILED_2, 0, [3]);
    let start = *file.offsets().get(2).ok_or("selected chunk start")?;
    let install = Install::new(vec![
        TestEntry::deflated(PRIMARY, file.bytes()).encrypted(),
        TestEntry::stored("data/SKELETON.CHR", "losing path occurrence"),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.call(
        "list_model_items",
        &json!({"path":"data/SKELETON.CHR","section":"bones","limit":1}),
    )?;
    let root = &first["structuredContent"];
    assert_eq!(root["total"], 2);
    assert_eq!(root["items"][0]["name"], "root");
    assert_eq!(field(root, "model")?["containsEncryptedSource"], false);
    marker(field(root, "items/0/name")?, true);
    assert_prefix(root, "total", 3)?;
    assert_all_addresses(&first)?;
    let second = server.call(
        "list_model_items",
        &json!({
            "path":"data/SKELETON.CHR","section":"bones","limit":1,"cursor":root["next_cursor"]
        }),
    )?;
    let reply = &second["structuredContent"];
    assert_eq!(reply["items"][0]["index"], 1);
    assert_eq!(reply["items"][0]["name"], "tip");
    assert_eq!(reply["items"][0]["world"]["translation"][0], 7.5);
    assert!(reply["items"][0]["node"].is_null());
    assert!(reply.get("next_cursor").is_none());
    assert!(field(reply, "next_cursor").is_err());
    assert_compiled_2_tip_sites(reply, start)?;
    assert_eq!(
        reply["provenance"]["sources"].as_array().map(Vec::len),
        Some(1)
    );
    assert_retained_bone_positions(reply, 2, 1..2)?;
    assert_all_addresses(&second)
}

#[test]
fn compact_bones_keep_the_original_node_word_and_separate_world_pose_table() -> TestResult {
    let file = TestChunkFile::ivo()
        .chunk(COMPILED, 0, TestSkeleton::two_bones().compiled_bones())
        .chunk(COMPILED, 0, [1])
        .chunk(UNRELATED, 0, [2])
        .table_at_end();
    let start = *file.offsets().first().ok_or("compact skeleton offset")?;
    let bytes = file.bytes();
    let table = bytes.get(12..16).ok_or("original table offset")?;
    let table = u64::from(u32::from_le_bytes(table.try_into()?));
    let install = Install::new(vec![TestEntry::stored(PRIMARY, bytes)])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok(
        "list_model_items",
        &json!({"path":PRIMARY,"section":"bones"}),
    )?;
    assert_eq!(reply["items"][1]["node"], 1);
    assert_eq!(reply["items"][1]["parent"], 0);
    marker(field(&reply, "items/1/node")?, false);
    assert_site(
        &reply,
        "total",
        "cryEngineChunkTable",
        "skeletonTypePrefix",
        &[&table.to_string(), &(table + 4).to_string(), "16", "3"],
    )?;
    for (output, source, span) in [
        ("node", "node", 78..80),
        ("parent", "parent", 72..74),
        ("world/translation/1", "world.translation", 193..197),
        ("relative/rotation/0", "relative.rotation", 117..121),
    ] {
        assert_bone_span(
            &reply,
            &format!("items/1/{output}"),
            source,
            0,
            1,
            start + span.start..start + span.end,
        )?;
    }
    Ok(())
}

#[test]
fn an_encrypted_lod_skeleton_does_not_change_the_primary_bone_origin() -> TestResult {
    let lod = TestModel::cube()
        .with_skeleton(&TestSkeleton::chain(6))
        .with_bone_map(false, vec![vec![(5, 255)]; 24]);
    let model = TestModel::two_bone_skin().with_lod(lod);
    let path = "Data/skin.skin";
    let install = Install::new(vec![
        TestEntry::stored(path, model.cgf()),
        TestEntry::stored("Data/skin.skinm", model.cgfm()),
        TestEntry::stored(
            "Data/skin_lod1.skinm",
            model.lod_cgfm(1).ok_or("LOD geometry")?,
        ),
        TestEntry::zstd(
            "Data/skin_lod1.skin",
            model.lod_cgf(1).ok_or("LOD skeleton")?,
        )
        .encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let overview = server.ok("get_model_info", &json!({"path":path}))?;
    assert_eq!(overview["lods"].as_array().map(Vec::len), Some(2));
    let reply = server.ok("list_model_items", &json!({"path":path,"section":"bones"}))?;
    assert_eq!(reply["total"], 2);
    marker(field(&reply, "items/1/controller_id")?, false);
    assert_eq!(
        reply["provenance"]["sources"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        reply
            .pointer("/provenance/sources/0/path/value")
            .and_then(Value::as_str),
        Some(path)
    );
    Ok(())
}

#[test]
fn an_empty_bone_list_proves_the_original_survey_for_its_container() -> TestResult {
    let install = Install::new(vec![
        TestEntry::stored(
            PRIMARY,
            TestChunkFile::ivo().chunk(UNRELATED, 0, [1]).bytes(),
        )
        .encrypted(),
        TestEntry::stored(
            "Data/compact.cgf",
            TestChunkFile::crch().chunk(0x1100, 1, [1]).bytes(),
        ),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let ivo = server.call(
        "list_model_items",
        &json!({"path":PRIMARY,"section":"bones"}),
    )?;
    let body = &ivo["structuredContent"];
    assert_eq!(body["items"], json!([]));
    assert_eq!(body["total"], 0);
    assert_prefix(body, "items", 1)?;
    marker(field(body, "total")?, true);
    assert_all_addresses(&ivo)?;
    let crch = server.ok(
        "list_model_items",
        &json!({"path":"Data/compact.cgf","section":"bones"}),
    )?;
    assert_eq!(crch["items"], json!([]));
    marker(field(&crch, "total")?, false);
    assert_eq!(
        uses(&crch, "total")?.len(),
        1,
        "the selected container layout only"
    );
    assert_site(
        &crch,
        "total",
        "cryEngineChunkHeader",
        "signature",
        &["0", "4"],
    )?;
    let tools = server.tools()?;
    let schema = &tools
        .iter()
        .find(|tool| tool["name"] == "list_model_items")
        .ok_or("model item tool")?["outputSchema"];
    assert_eq!(
        schema.pointer("/allOf/0/if/properties/section/const"),
        Some(&json!("bones"))
    );
    assert_eq!(
        schema.pointer("/allOf/0/then/required"),
        Some(&json!(["fieldOrigins", "provenance"]))
    );
    Ok(())
}

#[test]
fn bone_pages_preserve_every_row_without_exceeding_the_structured_response_budget() -> TestResult {
    let count = 180_u16;
    let file = TestChunkFile::ivo()
        .chunk(COMPILED_2, 0, TestSkeleton::chain(count).compiled_bones_2())
        .bytes();
    let install = Install::new(vec![TestEntry::stored(PRIMARY, file)])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"path": PRIMARY, "section": "bones", "limit": 1000});
    let mut next = 0;
    loop {
        let response = server.call("list_model_items", &args)?;
        let reply = &response["structuredContent"];
        assert_eq!(reply["total"], count);
        assert!(serde_json::to_vec(reply)?.len() <= RESPONSE_LIMIT);
        let items = reply["items"].as_array().ok_or("bone rows")?;
        assert!(!items.is_empty(), "each page advances");
        let first = next;
        for item in items {
            assert_eq!(item["index"], next);
            next += 1;
        }
        assert_retained_bone_positions(reply, 0, first..next)?;
        assert_all_addresses(&response)?;
        if let Some(cursor) = reply.get("next_cursor") {
            args["cursor"] = cursor.clone();
        } else {
            assert!(first > 0, "metadata forces pagination");
            break;
        }
    }
    assert_eq!(next, usize::from(count));
    Ok(())
}
