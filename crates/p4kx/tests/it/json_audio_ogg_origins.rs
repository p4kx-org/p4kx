// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    ArchiveBuilder, TestCodebook, TestEntry, TestOpus, TestOpusFraming, TestVorbis,
};
use serde_json::Value;

use super::{
    Input, SOUND, TestResult, check_format_origins, check_name_origin, check_source, contributors,
};

const FORMAT_BODY: u64 = 20;
const STORED_RATE_WORD: Range<usize> = 24..28;
const AOTUV_ONLY: u16 = 97;

#[test]
fn ogg_summary_binds_the_selected_entry_and_both_encoder_rates() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("duplicates.p4k");
    let standalone = directory.path().join("standalone.wem");
    let mut opus = TestOpus::new(TestOpusFraming::Wwise, 2, 24_000)
        .wem()?
        .bytes();
    opus.get_mut(STORED_RATE_WORD)
        .ok_or("original Opus format rate word")?
        .copy_from_slice(&24_000_u32.to_le_bytes());
    let vorbis = TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .codebook(TestCodebook::Library(AOTUV_ONLY))
        .wem()
        .bytes();
    fs::write(&standalone, &vorbis)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("unused.txt", b"unused".to_vec()))
        .entry(TestEntry::zstd(SOUND, opus).encrypted())
        .entry(TestEntry::zstd(SOUND, vorbis))
        .write_to(&archive)?;

    let encrypted = Input::Encrypted;
    let opus = ogg_json(&archive, encrypted, &directory.path().join("archive.ogg"))?;
    assert_eq!(
        opus["codec"], "opus",
        "the first physical Entry is selected"
    );
    assert_eq!(
        opus["channels"], 2,
        "Opus mapping covers the declared channels"
    );
    assert_eq!(opus["sample_rate"], 48_000, "effective Opus rate");
    let source = check_source(&opus, encrypted, 1)?;
    check_name_origin(&opus, source, &archive, encrypted)?;
    check_format_origins(
        &opus,
        source,
        encrypted,
        FORMAT_BODY,
        &[("/channels", "channels")],
    )?;
    check_opus_rate(&opus, source)?;

    let external = Input::Standalone;
    let vorbis = ogg_json(
        &standalone,
        external,
        &directory.path().join("standalone.ogg"),
    )?;
    assert_eq!(vorbis["codec"], "vorbis", "the standalone input is Vorbis");
    assert_eq!(vorbis["channels"], 1, "stored Vorbis channels");
    assert_eq!(vorbis["sample_rate"], 24_000, "stored Vorbis rate");
    let source = check_source(&vorbis, external, 0)?;
    check_name_origin(&vorbis, source, &standalone, external)?;
    check_format_origins(
        &vorbis,
        source,
        external,
        FORMAT_BODY,
        &[("/channels", "channels"), ("/sample_rate", "sampleRate")],
    )
}

fn ogg_json(path: &Path, input: Input, out: &Path) -> Result<Value, Box<dyn Error>> {
    let result = p4kx_testkit::p4kx()?
        .args(["--json", "audio", "ogg"])
        .arg(path)
        .args(input.entry())
        .arg("--out")
        .arg(out)
        .output()?;
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let document: Value = serde_json::from_slice(&result.stdout)?;
    let bytes = fs::read(out)?;
    assert_eq!(bytes.get(..4), Some(b"OggS".as_slice()), "Ogg container");
    assert_eq!(
        document.get("size").and_then(Value::as_u64),
        Some(u64::try_from(bytes.len())?),
        "the summary retains the output size"
    );
    for field in [
        "/codec",
        "/channels",
        "/sample_rate",
        "/sample_count",
        "/duration_seconds",
        "/codebooks",
        "/size",
        "/loop",
    ] {
        contributors(&document, field, input.payload_marker())?;
    }
    check_companion(out, &document, &bytes, input)?;
    Ok(document)
}

fn check_companion(out: &Path, document: &Value, bytes: &[u8], input: Input) -> TestResult {
    let name = out
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Ogg fixture output")?;
    let companion = format!("{name}.p4kx-origin.jsonl");
    assert_eq!(
        document.get("originReceipt").and_then(Value::as_str),
        Some(companion.as_str()),
        "the report names the sibling"
    );
    let rows = fs::read_to_string(out.with_file_name(companion))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let digest = measured.finish()?;
    let header = rows.first().ok_or("Ogg origin header")?;
    assert_eq!(
        header.pointer("/binding/sha256").and_then(Value::as_str),
        Some(digest.sha256()),
        "the companion binds the exact written stream"
    );
    check_ranges(&rows, input.payload_marker(), digest.byte_length())
}

fn check_ranges(rows: &[Value], encrypted: Option<bool>, byte_length: u64) -> TestResult {
    let mut boundary = 0;
    for field in ["identification", "commentsAndSetup", "audio"] {
        let marker = rows
            .iter()
            .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(field))
            .ok_or_else(|| format!("missing Ogg byte field {field}"))?;
        assert_eq!(
            marker.pointer("/address/type").and_then(Value::as_str),
            Some("oggBytes"),
            "{field}"
        );
        assert_eq!(
            marker.get("containsEncryptedSource"),
            Some(&serde_json::json!(encrypted)),
            "{field}"
        );
        let start = marker
            .pointer("/address/start")
            .and_then(Value::as_str)
            .ok_or("Ogg field start")?
            .parse::<u64>()?;
        let end = marker
            .pointer("/address/endExclusive")
            .and_then(Value::as_str)
            .ok_or("Ogg field end")?
            .parse::<u64>()?;
        assert_eq!(
            start, boundary,
            "{field} starts at the preceding page boundary"
        );
        assert!(end > start, "{field} occupies at least one page");
        boundary = end;
    }
    assert_eq!(boundary, byte_length, "the last field includes EOS");
    Ok(())
}

fn check_opus_rate(document: &Value, source: &Value) -> TestResult {
    let uses = contributors(document, "/sample_rate", Some(true))?;
    assert!(
        uses.iter().all(|used| {
            &used["source"] == source && used["scope"] == "payload" && used["role"] != "payload"
        }),
        "the fixed rate has only decision inputs from the selected Opus Entry"
    );
    let format_offset = FORMAT_BODY.to_string();
    assert!(
        uses.iter().any(|used| {
            used["role"] == "selector"
                && used.pointer("/location/namespace").and_then(Value::as_str) == Some("wemChunk")
                && used.pointer("/location/indices/0").and_then(Value::as_str)
                    == Some(format_offset.as_str())
                && used.pointer("/location/field").and_then(Value::as_str) == Some("formatTag")
        }),
        "the selected original codec tag supplies the rate decision"
    );
    Ok(())
}
