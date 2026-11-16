// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::{HEADER, TestResult, assert_standard_layout, p4kx, texture};

#[test]
fn split_dds_receipt_marks_only_the_mips_from_their_actual_entries() -> TestResult {
    let directory = tempfile::tempdir()?;
    let dds = texture();
    let archive = directory.path().join("split.p4k");
    let entries = dds
        .entries(HEADER)
        .into_iter()
        .enumerate()
        .map(
            |(index, entry)| {
                if index == 4 { entry.encrypted() } else { entry }
            },
        );
    entries
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .entry(TestEntry::stored(format!("{HEADER}.9"), vec![1, 2, 3]).encrypted())
        .write_to(&archive)?;
    let out = directory.path().join("merged.dds");
    let output = p4kx()?
        .args([
            "--json",
            "--key",
            &p4kx_testkit::test_key().to_hex(),
            "texture",
            "merge",
        ])
        .arg(archive)
        .arg(HEADER)
        .arg("--out")
        .arg(&out)
        .output()?;
    assert!(
        output.status.success(),
        "split merge succeeds: {:?}",
        output.stderr
    );
    let bytes = fs::read(&out)?;
    assert_standard_layout(&bytes, &dds);
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["field_origins"]["/parts"]["containsEncryptedSource"], false,
        "the clear header and its stored length determine the part count"
    );
    let sidecar = report["origin_receipt"]
        .as_str()
        .ok_or("DDS companion path")?;
    let rows = fs::read_to_string(sidecar)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    check_binding(&rows, &bytes)?;
    verify_ranges(&rows)
}

fn check_binding(rows: &[Value], bytes: &[u8]) -> TestResult {
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(bytes)?;
    let expected = serde_json::to_value(digest.finish()?)?;
    let binding = &rows.first().ok_or("receipt binding")?["binding"];
    for field in ["sha256", "byteLength"] {
        assert_eq!(
            binding
                .get(field)
                .ok_or("measured artifact binding field")?,
            expected
                .get(field)
                .ok_or("expected artifact binding field")?,
            "the receipt binds the emitted DDS {field}"
        );
    }
    assert_eq!(
        rows.iter().filter(|row| row["type"] == "source").count(),
        5,
        "the unused encrypted candidate is not a source"
    );
    Ok(())
}

fn verify_ranges(rows: &[Value]) -> TestResult {
    let mut fields = rows.iter().filter(|row| row["type"] == "field");
    let header = fields.next().ok_or("DDS header address")?;
    assert_eq!(
        header["address"],
        json!({"type": "ddsHeaderBytes", "endExclusive": "148"}),
        "raw header"
    );
    assert_eq!(
        header["containsEncryptedSource"], false,
        "clear original header"
    );
    let mut destination = 148;
    let mut header_tail = 148;
    for (level, length) in [2048, 512, 128, 32, 16, 16, 16].into_iter().enumerate() {
        let field = fields.next().ok_or("emitted mip address")?;
        let end = destination + length;
        assert_eq!(
            field["address"],
            json!({
                "type": "ddsMipBytes", "level": level, "surface": "0",
                "start": destination.to_string(), "endExclusive": end.to_string()
            }),
            "exact output range of mip {level}"
        );
        assert_eq!(
            field["containsEncryptedSource"],
            level == 0,
            "only the largest mip is encrypted"
        );
        assert_eq!(
            field["sourceSetComplete"], true,
            "all sources of mip {level} are bound"
        );
        let source_start = if level < 4 {
            0
        } else {
            let start = header_tail;
            header_tail += length;
            start
        };
        check_payload_site(rows, field, source_start, length)?;
        destination = end;
    }
    assert!(
        fields.next().is_none(),
        "every emitted extent is addressed once"
    );
    Ok(())
}

fn check_payload_site(rows: &[Value], field: &Value, start: u32, length: u32) -> TestResult {
    let references = field["sourceUsesRefs"]
        .as_array()
        .ok_or("mip source uses")?;
    let coordinates = json!([start.to_string(), length.to_string()]);
    assert!(
        rows.iter().any(|row| {
            let used = &row["sourceUse"];
            references.contains(&used["id"])
                && used["role"] == "payload"
                && used.pointer("/location/namespace").and_then(Value::as_str) == Some("ddsMip")
                && used.pointer("/location/indices") == Some(&coordinates)
        }),
        "the payload records its original decoded byte range"
    );
    Ok(())
}

#[test]
fn unowned_dds_companion_does_not_replace_the_previous_output() -> TestResult {
    let directory = tempfile::tempdir()?;
    let header = texture().write_files(directory.path(), "source.dds")?;
    let out = directory.path().join("merged.dds");
    let sidecar = directory.path().join("merged.dds.p4kx-origin.jsonl");
    fs::write(&out, b"original DDS destination")?;
    fs::write(&sidecar, b"unrelated contents")?;
    let output = p4kx()?
        .args(["texture", "merge"])
        .arg(header)
        .arg("--out")
        .arg(&out)
        .output()?;
    assert!(!output.status.success(), "unowned companion is a conflict");
    assert_eq!(
        fs::read(out)?,
        b"original DDS destination",
        "artifact stays intact"
    );
    assert_eq!(
        fs::read(sidecar)?,
        b"unrelated contents",
        "unowned file stays intact"
    );
    Ok(())
}
