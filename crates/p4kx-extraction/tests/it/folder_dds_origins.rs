// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::RecordExportReport;
use p4kx_testkit::check_dds_origin_pair;

use super::{Fixture, TestResult, tree};

const DDS: &str = "Data/Textures/rock_diff.dds";
const RECEIPT: &str = "Data/Textures/rock_diff.dds.p4kx-origin.jsonl";
const WAV_RECEIPT: &str = "Data/Sounds/Media/5.wav.p4kx-origin.jsonl";

#[test]
fn a_converted_folder_equals_what_extraction_with_conversion_writes() -> TestResult {
    let fixture = Fixture::new()?;
    let expected = fixture.extract("converted", true)?;
    let raw = fixture.extract("raw", false)?;

    let report = fixture.convert(&raw, "from-folder", None, true)?;

    assert!(report.is_complete(), "complete: {report:?}");
    let [artifact] = report.merged_texture_artifacts() else {
        return Err("one observed loose DDS artifact".into());
    };
    assert_eq!(artifact.path, DDS, "same emitted DDS path");
    assert_eq!(artifact.origin_receipt, RECEIPT, "durable sidecar path");
    let mut written = tree(&fixture.folder("from-folder"))?;
    let mut archived = tree(&expected)?;
    let archive_receipt = archived.remove(RECEIPT).ok_or("Archive DDS receipt")?;
    let loose_receipt = written.remove(RECEIPT).ok_or("loose DDS receipt")?;
    let dds = written.get(DDS).ok_or("the written DDS")?;
    check_dds_origin_pair(dds, &archive_receipt, &loose_receipt)?;
    let archive_wav = archived.remove(WAV_RECEIPT).ok_or("Archive WAV receipt")?;
    let folder_wav = written.remove(WAV_RECEIPT).ok_or("folder WAV receipt")?;
    assert_eq!(
        pcm_encryption(&archive_wav)?,
        Some(false),
        "plain Archive payload"
    );
    assert_eq!(
        pcm_encryption(&folder_wav)?,
        None,
        "observed file with unknown P4K encryption"
    );
    for path in ["Data/Ships/text.xml", "Data/readme.bin"] {
        let receipt = format!("{path}.p4kx-origin.jsonl");
        let archive_raw = archived.remove(&receipt).ok_or("Archive raw receipt")?;
        let folder_raw = written.remove(&receipt).ok_or("folder raw receipt")?;
        assert_eq!(opaque_encryption(&archive_raw)?, Some(false));
        assert_eq!(opaque_encryption(&folder_raw)?, None);
    }
    check_cryxml_origin_pair(&mut archived, &mut written)?;
    assert_eq!(written, archived, "identical payload files");
    assert!(
        written
            .keys()
            .any(|name| Path::new(name).extension().is_some_and(|ext| ext == "png")),
        "the texture became a PNG: {:?}",
        written.keys().collect::<Vec<_>>()
    );
    assert!(
        written.keys().any(|name| name == "Data/Sounds/Media/5.wav"),
        "the sound was decoded"
    );
    assert_eq!(report.records().map(RecordExportReport::written), Some(1));
    Ok(())
}

#[test]
fn loose_merge_replaces_old_archive_proof_for_identical_payload_bytes() -> TestResult {
    let fixture = Fixture::new()?;
    let target = fixture.extract("converted", true)?;
    let archive_receipt = fs::read(target.join(RECEIPT))?;
    let archived_dds = fs::read(target.join(DDS))?;
    let raw = fixture.extract("raw", false)?;

    let report = fixture.convert(&raw, "converted", None, true)?;
    assert!(report.is_complete(), "{report:?}");
    let [artifact] = report.merged_texture_artifacts() else {
        return Err("the replacement must have one new loose origin receipt".into());
    };
    assert_eq!(
        artifact.origin_receipt, RECEIPT,
        "the old location is reused"
    );
    let written = fs::read(target.join(DDS))?;
    assert_eq!(
        written, archived_dds,
        "the loose merge emits the same DDS bytes"
    );
    let loose_receipt = fs::read(target.join(RECEIPT))?;
    check_dds_origin_pair(&written, &archive_receipt, &loose_receipt)
}

fn check_cryxml_origin_pair(
    archive: &mut std::collections::BTreeMap<String, Vec<u8>>,
    loose: &mut std::collections::BTreeMap<String, Vec<u8>>,
) -> TestResult {
    use serde_json::{Value, json};

    const XML: &str = "Data/Ships/gladius.xml";
    const ORIGIN: &str = "Data/Ships/gladius.xml.p4kx-origin.jsonl";
    let xml = std::str::from_utf8(loose.get(XML).ok_or("converted loose XML")?)?;
    let start = xml.find("<Ship").ok_or("converted root element")? + 1;
    let expected = json!({
        "type": "xmlBytes",
        "start": start.to_string(),
        "endExclusive": (start + "Ship".len()).to_string(),
        "field": "elementName",
    });
    for (files, encrypted) in [(archive, json!(false)), (loose, Value::Null)] {
        let receipt = files.remove(ORIGIN).ok_or("converted XML companion")?;
        let mut seen = false;
        for line in std::str::from_utf8(&receipt)?.lines() {
            let row: Value = serde_json::from_str(line)?;
            if row.get("address") == Some(&expected) {
                assert_eq!(
                    row.get("containsEncryptedSource"),
                    Some(&encrypted),
                    "the shared XML field keeps the actual input encryption marker"
                );
                assert_eq!(
                    row.get("sourceSetComplete"),
                    Some(&json!(true)),
                    "the original CryXml field was fully observed"
                );
                assert!(
                    row.get("sourceUsesRefs")
                        .and_then(Value::as_array)
                        .is_some_and(|refs| !refs.is_empty()),
                    "the emitted XML element name cites its original source field"
                );
                seen = true;
            }
        }
        assert!(seen, "the companion covers the original XML root name");
    }
    Ok(())
}

fn opaque_encryption(bytes: &[u8]) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    for line in std::str::from_utf8(bytes)?.lines() {
        let row: serde_json::Value = serde_json::from_str(line)?;
        if row
            .pointer("/address/type")
            .and_then(serde_json::Value::as_str)
            == Some("opaquePayload")
        {
            assert_eq!(
                row.get("sourceSetComplete")
                    .and_then(serde_json::Value::as_bool),
                Some(true),
                "the original payload is fully observed"
            );
            return Ok(row
                .get("containsEncryptedSource")
                .and_then(serde_json::Value::as_bool));
        }
    }
    Err("opaque payload origin is missing".into())
}

fn pcm_encryption(bytes: &[u8]) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    for line in std::str::from_utf8(bytes)?.lines() {
        let row: serde_json::Value = serde_json::from_str(line)?;
        if row
            .pointer("/address/field")
            .and_then(serde_json::Value::as_str)
            == Some("pcmSamples")
        {
            assert_eq!(
                row.get("sourceSetComplete")
                    .and_then(serde_json::Value::as_bool),
                Some(true),
                "original observed WEM"
            );
            assert!(
                row.get("sourceUsesRefs")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|refs| !refs.is_empty()),
                "the WEM supplies original samples"
            );
            return Ok(row
                .get("containsEncryptedSource")
                .ok_or("encryption state")?
                .as_bool());
        }
    }
    Err("missing WAV sample field".into())
}
