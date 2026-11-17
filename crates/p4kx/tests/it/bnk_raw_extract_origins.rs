// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::Output;

use p4kx_extraction::{OriginArtifactDigest, OriginDigestWriter};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestCodec, TestEntry, TestWem, p4kx};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const ENTRY: &str = "Data/Sounds/raw-receipt.bnk";
const CLEAR_ENTRY: &str = "Data/Sounds/raw-clear.bnk";
const MIDI: &[u8] = b"MThd\0\0\0\x06\0\x01\0\x01\x01\xE0";
const DIDX_BODY: usize = 60;
const DIDX_TRIPLET: usize = 12;
const DATA_BODY: usize = DIDX_BODY + 4 * DIDX_TRIPLET + 8;
const DATA_ALIGNMENT: usize = 16;

#[test]
fn raw_bank_receipts_bind_the_actual_generation_and_selected_bytes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let standalone = folder.path().join("same.bnk");
    let out = folder.path().join("out");
    let pcm = TestWem::pcm(TestCodec::Pcm, 1, 8000, &[0, 1, 2, 3]).bytes();
    let prefetched = pcm.get(..12).ok_or("RIFF prefix")?;
    let bytes = TestBank::new(57)
        .wem(9, MIDI)
        .wem(1, pcm.as_slice())
        .wem(8, prefetched)
        .wem(1, b"PLUG\0\0\0\0".as_slice())
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ENTRY, bytes.clone()).encrypted())
        .entry(TestEntry::stored(CLEAR_ENTRY, bytes.clone()))
        .write_to(&archive)?;
    fs::write(&standalone, &bytes)?;

    let fixture = [
        Media {
            id: 1,
            leaf: "1.wem",
            bytes: &pcm,
            ordinal: 1,
            offset: DATA_ALIGNMENT,
            prefetch: false,
        },
        Media {
            id: 8,
            leaf: "8.wem",
            bytes: prefetched,
            ordinal: 2,
            offset: (DATA_ALIGNMENT + pcm.len()).next_multiple_of(DATA_ALIGNMENT),
            prefetch: true,
        },
        Media {
            id: 9,
            leaf: "9.mid",
            bytes: MIDI,
            ordinal: 0,
            offset: 0,
            prefetch: false,
        },
    ];

    let encrypted_output = extract(&archive, Some(ENTRY), &out)?;
    assert_eq!(
        encrypted_output.status.code(),
        Some(0),
        "{encrypted_output:?}"
    );
    let encrypted: Value = serde_json::from_slice(&encrypted_output.stdout)?;
    check_media(&encrypted, &out, &fixture, &Value::Bool(true))?;

    let clear_output = extract(&archive, Some(CLEAR_ENTRY), &out)?;
    assert_eq!(clear_output.status.code(), Some(0), "{clear_output:?}");
    let clear: Value = serde_json::from_slice(&clear_output.stdout)?;
    check_media(&clear, &out, &fixture, &Value::Bool(false))?;

    let file_output = extract(&standalone, None, &out)?;
    assert_eq!(file_output.status.code(), Some(0), "{file_output:?}");
    let file: Value = serde_json::from_slice(&file_output.stdout)?;
    check_media(&file, &out, &fixture, &Value::Null)?;
    check_replacement(&encrypted, &file, &out)
}

fn check_replacement(encrypted: &Value, file: &Value, out: &Path) -> TestResult {
    for (before, after) in extracted(encrypted)?.iter().zip(extracted(file)?) {
        assert_eq!(before["file"], after["file"], "same committed path");
        assert_eq!(
            before["originReceipt"], after["originReceipt"],
            "same sibling name"
        );
        assert_ne!(
            before["originReceiptDigest"], after["originReceiptDigest"],
            "identical payload bytes from different sources still identify different receipts",
        );
    }
    assert_eq!(
        fs::read_dir(out)?.count(),
        6,
        "only media and their companions"
    );
    Ok(())
}

#[test]
fn collision_at_the_origin_lock_preserves_the_prior_bank_output() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bank = folder.path().join("same.bnk");
    let out = folder.path().join("out");
    fs::write(&bank, TestBank::new(57).wem(9, MIDI).bytes())?;
    fs::create_dir(&out)?;
    let old = out.join("9.mid");
    fs::write(&old, b"prior bytes")?;
    let foreign_lock = out.join(".p4kx-origin.lock");
    fs::create_dir(&foreign_lock)?;

    let output = extract(&bank, None, &out)?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["extracted"],
        json!([]),
        "no installed output was reported"
    );
    assert_eq!(
        report["failed"][0]["id"], 9,
        "the failed member is accounted for"
    );
    assert_eq!(fs::read(old)?, b"prior bytes", "uncommitted replacement");
    assert!(foreign_lock.is_dir(), "foreign lock directory stays");
    assert_eq!(
        fs::read_dir(out)?.count(),
        2,
        "no staged or receipt files escaped"
    );
    Ok(())
}

#[test]
fn a_foreign_reserved_companion_blocks_replacement_without_touching_either_file() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bank = folder.path().join("same.bnk");
    let out = folder.path().join("out");
    fs::write(&bank, TestBank::new(57).wem(9, MIDI).bytes())?;
    fs::create_dir(&out)?;
    let old = out.join("9.mid");
    let foreign = out.join("9.mid.p4kx-origin.jsonl");
    fs::write(&old, b"prior bytes")?;
    fs::write(&foreign, b"unrelated contents")?;

    let output = extract(&bank, None, &out)?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["extracted"],
        json!([]),
        "neither file is this attempt's output"
    );
    assert_eq!(
        report["failed"][0]["id"], 9,
        "the collision is a per-member error"
    );
    assert_eq!(fs::read(old)?, b"prior bytes", "the media did not commit");
    assert_eq!(
        fs::read(foreign)?,
        b"unrelated contents",
        "foreign file untouched"
    );
    assert_eq!(
        fs::read_dir(out)?.count(),
        2,
        "no staged or lock files escaped"
    );
    Ok(())
}

struct Media<'a> {
    id: u32,
    leaf: &'a str,
    bytes: &'a [u8],
    ordinal: usize,
    offset: usize,
    prefetch: bool,
}

fn extract(input: &Path, entry: Option<&str>, out: &Path) -> Result<Output, Box<dyn Error>> {
    let mut command = p4kx()?;
    command.args(["--json", "bnk", "extract"]).arg(input);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    Ok(command
        .args(["--format", "wem", "--out"])
        .arg(out)
        .output()?)
}

fn extracted(report: &Value) -> Result<&[Value], &'static str> {
    report["extracted"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or("extracted media")
}

fn check_media(
    report: &Value,
    out: &Path,
    fixture: &[Media<'_>],
    encryption: &Value,
) -> TestResult {
    assert_eq!(report["failed"], json!([]), "all selected raw media commit");
    let rows = extracted(report)?;
    assert_eq!(
        rows.len(),
        fixture.len(),
        "duplicate ID selects one physical triplet"
    );
    for (row, selected) in rows.iter().zip(fixture) {
        assert_eq!(row["id"], selected.id, "stable sorted ID");
        assert_eq!(
            row["prefetch"], selected.prefetch,
            "preserved prefetch state"
        );
        assert_eq!(row["size"], selected.bytes.len(), "retained byte count");
        assert_eq!(
            fs::read(out.join(selected.leaf))?,
            selected.bytes,
            "exact copied bytes"
        );

        check_receipt(row, selected, out, encryption)?;
    }
    Ok(())
}

fn check_receipt(row: &Value, selected: &Media<'_>, out: &Path, encryption: &Value) -> TestResult {
    let name = format!("{}.p4kx-origin.jsonl", selected.leaf);
    assert_eq!(row["originReceipt"], name, "relative committed companion");
    let receipt = fs::read(out.join(&name))?;
    assert_eq!(
        row["originReceiptDigest"],
        serde_json::to_value(digest(&receipt)?)?,
        "measured receipt generation"
    );
    let rows = std::str::from_utf8(&receipt)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("receipt header binding")?;
    let measured = serde_json::to_value(digest(selected.bytes)?)?;
    assert_eq!(binding["type"], "file", "sibling-file binding");
    assert_eq!(
        binding.get("byteLength"),
        Some(measured.get("byteLength").ok_or("measured byte length")?),
        "artifact length binding"
    );
    assert_eq!(
        binding.get("sha256"),
        Some(measured.get("sha256").ok_or("measured digest")?),
        "artifact digest binding"
    );
    assert_eq!(
        rows.last().ok_or("receipt end")?["type"],
        "end",
        "complete companion"
    );

    check_source_uses(&rows, selected, encryption)
}

fn check_source_uses(rows: &[Value], selected: &Media<'_>, encryption: &Value) -> TestResult {
    let payload = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("opaque media field")?;
    assert_eq!(
        &payload["containsEncryptedSource"], encryption,
        "the observed bank controls encryption state"
    );
    assert_eq!(
        payload["sourceSetComplete"], true,
        "the opened bank and extent are known"
    );
    let references = payload["sourceUsesRefs"]
        .as_array()
        .ok_or("field use refs")?;
    let usage = |scope: &str, field: &str, role: &str, start: usize, end: usize| {
        rows.iter()
            .filter_map(|row| row.get("sourceUse"))
            .any(|usage| {
                usage.get("id").is_some_and(|id| references.contains(id))
                    && usage.get("role").and_then(Value::as_str) == Some(role)
                    && usage.pointer("/location/namespace").and_then(Value::as_str) == Some(scope)
                    && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && usage.pointer("/location/indices")
                        == Some(&json!([start.to_string(), end.to_string()]))
            })
    };
    let didx = DIDX_BODY + selected.ordinal * DIDX_TRIPLET;
    let start = DATA_BODY + selected.offset;
    for (scope, field, role, lower, upper) in [
        ("bnkDidx", "mediaId", "selector", didx, didx + 4),
        ("bnkDidx", "dataOffset", "selector", didx + 4, didx + 8),
        ("bnkDidx", "dataLength", "selector", didx + 8, didx + 12),
        (
            "bnkData",
            "bodyLength",
            "selector",
            DATA_BODY - 4,
            DATA_BODY,
        ),
        (
            "bnkEmbeddedWem",
            "payload",
            "payload",
            start,
            start + selected.bytes.len(),
        ),
    ] {
        assert!(
            usage(scope, field, role, lower, upper),
            "missing {role} use for {scope}.{field} at {lower}..{upper}",
        );
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> io::Result<OriginArtifactDigest> {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    measured.finish()
}
