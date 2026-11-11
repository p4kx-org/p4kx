// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::gltf::mesh::Semantic;
use p4kx_testkit::{HEAD_VERTICES, MALE_RIG_HASH, check_glb, head_entries, head_rig, test_key};
use serde_json::Value;

use super::{TestResult, archive, chf, glb, glb_with};

fn check_receipt(
    out: &Path,
    report: &Value,
    expected: &Value,
    positions_encrypted: bool,
) -> TestResult {
    let name = report
        .get("originReceipt")
        .and_then(Value::as_str)
        .ok_or("origin receipt")?;
    assert_eq!(name, "character.glb.p4kx-origin.jsonl", "canonical sibling");
    let receipt = fs::read_to_string(out.with_file_name(name))?;
    let mut rows = receipt.lines().map(serde_json::from_str::<Value>);
    let header = rows.next().ok_or("receipt header")??;
    let bytes = fs::read(out)?;
    assert_eq!(
        header
            .pointer("/binding/path/encoding")
            .and_then(Value::as_str),
        Some("utf8"),
        "fixture name uses portable UTF-8 encoding"
    );
    assert_eq!(
        header
            .pointer("/binding/path/value")
            .and_then(Value::as_str),
        Some("character.glb"),
        "companion binds the relative artifact name"
    );
    assert_eq!(
        header
            .pointer("/binding/byteLength")
            .and_then(Value::as_str),
        Some(bytes.len().to_string().as_str()),
        "receipt length covers the committed GLB"
    );
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&bytes)?;
    let measured = serde_json::to_value(measured.finish()?)?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(measured.get("sha256").ok_or("measured digest")?),
        "receipt digest covers the committed GLB"
    );
    let rows = rows.collect::<Result<Vec<_>, _>>()?;
    let field = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("opaque GLB payload")?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(expected),
        "receipt describes the current artifact producer"
    );
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "receipt has a footer"
    );
    check_head_positions(&bytes, &rows, positions_encrypted)
}

fn check_head_positions(bytes: &[u8], rows: &[Value], encrypted: bool) -> TestResult {
    let checked = check_glb(bytes)?;
    let positions = checked
        .document
        .nodes()
        .find(|node| node.name() == Some("head") && node.mesh().is_some())
        .and_then(|node| node.mesh())
        .and_then(|mesh| mesh.primitives().next())
        .and_then(|primitive| primitive.get(&Semantic::Positions))
        .ok_or("head positions")?;
    let view = positions.view().ok_or("positions buffer view")?;
    for (path, kind) in [
        (format!("/bufferViews/{}", view.index()), "glbBinaryBytes"),
        (format!("/accessors/{}", positions.index()), "glbJsonField"),
    ] {
        let row = rows
            .iter()
            .find(|row| {
                row.pointer("/address/path").and_then(Value::as_str) == Some(path.as_str())
                    && row.pointer("/address/type").and_then(Value::as_str) == Some(kind)
            })
            .ok_or("exact head positions origin")?;
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(encrypted),
            "{kind} classifies only the selected head geometry source"
        );
    }
    Ok(())
}

#[test]
fn an_equal_byte_replacement_updates_the_glb_receipt_and_keeps_field_origins_distinct() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let encrypted_dir = directory.path().join("encrypted");
    let plain_dir = directory.path().join("plain");
    fs::create_dir(&encrypted_dir)?;
    fs::create_dir(&plain_dir)?;
    let entries = head_entries(Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build()));
    let encrypted = archive(
        &encrypted_dir,
        entries
            .iter()
            .cloned()
            .map(|entry| {
                if Path::new(entry.path())
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("dna"))
                {
                    entry.encrypted()
                } else {
                    entry
                }
            })
            .collect(),
    )?;
    let plain = archive(&plain_dir, entries)?;
    let file = chf(directory.path(), "character.chf", Some((MALE_RIG_HASH, 2)))?;
    let out = directory.path().join("character.glb");

    let key = test_key().to_hex();
    let encrypted_run = glb_with(&encrypted, &file, &out, true, &["--key", &key])?;
    assert_eq!(encrypted_run.code, Some(0), "{}", encrypted_run.stderr);
    let report: Value = serde_json::from_str(&encrypted_run.stdout)?;
    assert_eq!(
        report["fieldOrigins"]["/vertices"]["containsEncryptedSource"],
        true
    );
    assert!(report["fieldOrigins"]["/body"]["containsEncryptedSource"].is_null());
    check_receipt(&out, &report, &Value::Bool(true), true)?;
    let bytes = fs::read(&out)?;

    let plain_run = glb(&plain, &file, &out, true)?;
    assert_eq!(plain_run.code, Some(0), "{}", plain_run.stderr);
    assert_eq!(fs::read(&out)?, bytes);
    let report: Value = serde_json::from_str(&plain_run.stdout)?;
    assert_eq!(
        report["fieldOrigins"]["/vertices"]["containsEncryptedSource"],
        false
    );
    check_receipt(&out, &report, &Value::Null, false)?;
    Ok(())
}

#[test]
fn an_invalid_chf_preserves_the_previous_glb_and_its_exact_receipt() -> TestResult {
    let directory = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(directory.path(), head_entries(Some(rig)))?;
    let file = chf(directory.path(), "character.chf", Some((MALE_RIG_HASH, 2)))?;
    let out = directory.path().join("character.glb");
    let initial = glb(&archive, &file, &out, true)?;
    assert_eq!(initial.code, Some(0), "{}", initial.stderr);
    let receipt = out.with_file_name("character.glb.p4kx-origin.jsonl");
    let bytes = fs::read(&out)?;
    let original = fs::read(&receipt)?;

    let bad = directory.path().join("bad.chf");
    fs::write(&bad, b"invalid CHF")?;
    let failed = glb(&archive, &bad, &out, true)?;
    assert_eq!(failed.code, Some(3), "{}", failed.stderr);
    assert_eq!(fs::read(out)?, bytes);
    assert_eq!(fs::read(receipt)?, original);
    Ok(())
}

#[test]
fn the_glb_and_its_receipt_cannot_replace_the_chf_input_or_an_alias() -> TestResult {
    let directory = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(directory.path(), head_entries(Some(rig)))?;
    let file = chf(directory.path(), "character.chf", Some((MALE_RIG_HASH, 2)))?;
    let alias = directory.path().join("alias.glb");
    let output = directory.path().join("character.glb");
    let receipt = directory.path().join("character.glb.p4kx-origin.jsonl");
    fs::hard_link(&file, &alias)?;
    fs::hard_link(&file, &receipt)?;
    let original = fs::read(&file)?;

    for target in [&file, &alias, &output] {
        let run = glb(&archive, &file, target, true)?;
        assert_eq!(run.code, Some(2), "{}: {}", target.display(), run.stderr);
        assert_eq!(fs::read(&file)?, original);
        assert_eq!(fs::read(&alias)?, original);
        assert_eq!(fs::read(&receipt)?, original);
        assert!(!output.exists());
    }
    Ok(())
}
