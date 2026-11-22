// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ChunkFile;
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry};
use serde_json::{Value, json};

use super::{ALIAS_TARGET, LITERAL, main_system, normal_catalog, only_row, source};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::TestResult;

const CLEAR: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ec";
const ENCRYPTED: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";
const CLEAR_MEMBER: &str = "z-clear-first.soc";
const ENCRYPTED_MEMBER: &str = "a-encrypted-second.soc";

#[test]
fn the_first_tag_id_and_the_complete_count_have_independent_origins() -> TestResult {
    let clear = clear_xml();
    let clear_soc = soc(&clear);
    let selected = target(clear_soc.clone());
    let fixture = source([main_system(LITERAL)], [selected])?;
    let catalog = normal_catalog(&fixture)?;
    let placement = only_row(&catalog)?;
    let counts = proof::cell(placement, "tagCounts", "present")?;
    let [clear_row, encrypted_row] = proof::rows(counts, "/value")? else {
        return Err("two canonical tag rows ordered by GUID".into());
    };
    let clear_guid = proof::present(clear_row, "tagGuid", &json!(CLEAR), Some(false))?;
    let encrypted_guid = proof::present(encrypted_row, "tagGuid", &json!(ENCRYPTED), Some(true))?;
    let clear_count = proof::present(clear_row, "entities", &json!(3), Some(true))?;
    let encrypted_count = proof::present(encrypted_row, "entities", &json!(1), Some(true))?;
    let survey = proof::at(counts, "/sourceUsesRefs")?;
    for count in [
        clear_count,
        encrypted_count,
        proof::cell(placement, "entityCount", "present")?,
    ] {
        assert_eq!(
            proof::at(count, "/sourceUsesRefs")?,
            survey,
            "all exact totals cite the same survey"
        );
    }
    let clear_id = tag_use(&catalog, clear_guid, CLEAR_MEMBER)?;
    let encrypted_id = tag_use(&catalog, encrypted_guid, ENCRYPTED_MEMBER)?;
    assert_eq!(
        clear_id.pointer("/location/field"),
        Some(&json!("@tAgId")),
        "original attribute spelling survives"
    );
    assert_eq!(
        encrypted_id.pointer("/location/field"),
        Some(&json!("@TagId")),
        "encrypted member supplies its own positive TagId"
    );
    verify_text_position(clear_id, &clear_soc, &clear)?;
    proof::closure(&catalog)
}

fn verify_text_position(used: &Value, bytes: &[u8], xml: &str) -> TestResult {
    let file = ChunkFile::parse(bytes)?;
    let chunk = file.chunks().get(1).ok_or("original entity XML chunk")?;
    let document = roxmltree::Document::parse(xml)?;
    let node = document
        .descendants()
        .find(|node| node.has_tag_name("Tag"))
        .ok_or("original Tag")?;
    assert_eq!(
        used.pointer("/location/indices"),
        Some(&json!([
            "1",
            chunk.offset().to_string(),
            (chunk.offset() + chunk.size()).to_string(),
            node.id().get().to_string(),
            "1"
        ])),
        "the source selects an exact original chunk, XML node and attribute occurrence"
    );
    Ok(())
}

fn tag_use<'a>(catalog: &'a Value, cell: &Value, path: &str) -> TestResult<&'a Value> {
    let uses = proof::uses(catalog, cell)?;
    let [site] = uses.as_slice() else {
        return Err("a GUID retains exactly its first positive original TagId".into());
    };
    assert_eq!(
        (site.get("role"), site.pointer("/location/namespace")),
        (Some(&json!("payload")), Some(&json!("socEntityTag.text"))),
        "the exact positive tag operand is payload"
    );
    assert_eq!(
        proof::source(catalog, site)?.pointer("/path/value"),
        Some(&json!(path)),
        "a neighboring tag member cannot supply the identity"
    );
    Ok(*site)
}

fn clear_xml() -> String {
    format!(
        r#"<Entities>
        <Entity><Tags><Tag ignored="x" tAgId="{CLEAR}"/><Tag TagId="{CLEAR}"/></Tags></Entity>
        <Entity><Tags><Tag TagId="malformed"/><Tag TagId="{CLEAR}"/></Tags></Entity>
        </Entities>"#
    )
}

fn soc(xml: &str) -> Vec<u8> {
    TestChunkFile::crch()
        .chunk(0x0001, 0, b"unrelated".to_vec())
        .chunk(0x0004, 1, xml.as_bytes())
        .bytes()
}

fn target(clear: Vec<u8>) -> TestEntry {
    let encrypted_xml = format!(
        r#"<Entities><Entity><Tags><Tag TagId="{CLEAR}"/><Tag TagId="{ENCRYPTED}"/></Tags></Entity></Entities>"#
    );
    let container = ArchiveBuilder::v2()
        .entry(
            TestEntry::stored("alias-target.xml", r#"<ObjectContainer version="14"/>"#)
                .with_encryption_field(0),
        )
        .entry(TestEntry::stored(CLEAR_MEMBER, clear).with_encryption_field(0))
        .entry(TestEntry::stored(ENCRYPTED_MEMBER, soc(&encrypted_xml)).encrypted())
        .build();
    TestEntry::stored(ALIAS_TARGET, container)
}
