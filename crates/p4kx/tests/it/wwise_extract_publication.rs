// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Cursor, Write};
use std::path::Path;

use lewton::inside_ogg::OggStreamReader;
use p4kx_extraction::OriginDigestWriter;
use serde_json::Value;

use super::{TestResult, count, extract, fixture, names, pcm, xma};

pub(super) fn assert_sound_receipt(file: &Value, address: &str) -> TestResult {
    assert_eq!(
        file.get("origin_receipt_failure"),
        Some(&Value::Null),
        "companion publication completed"
    );
    let output = Path::new(file["file"].as_str().ok_or("exported sound path")?);
    let receipt = Path::new(
        file["origin_receipt"]
            .as_str()
            .ok_or("sound receipt path")?,
    );
    let leaf = output.file_name().ok_or("sound file name")?;
    let mut receipt_name = leaf.to_os_string();
    receipt_name.push(".p4kx-origin.jsonl");
    assert_eq!(
        fs::canonicalize(receipt)?,
        fs::canonicalize(output.with_file_name(receipt_name))?,
        "the report identifies the canonical sibling"
    );

    let rows = fs::read_to_string(receipt)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&fs::read(output)?)?;
    let digest = measured.finish()?;
    let binding = rows
        .first()
        .ok_or("sound receipt header")?
        .get("binding")
        .ok_or("sound binding")?;
    assert_eq!(
        binding["sha256"],
        digest.sha256(),
        "the companion binds the exact committed bytes"
    );
    assert_eq!(
        binding["byteLength"],
        digest.byte_length().to_string(),
        "the binding covers the whole file"
    );
    assert_eq!(
        rows.last().ok_or("sound receipt end")?["type"],
        "end",
        "the companion was written completely"
    );
    assert!(
        rows.iter().any(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some(address)
                && row["containsEncryptedSource"] == false
                && row["sourceSetComplete"] == true
                && row["sourceUsesRefs"]
                    .as_array()
                    .is_some_and(|uses| !uses.is_empty())
        }),
        "plain Archive input is cited for {address}"
    );
    Ok(())
}

#[test]
fn a_trigger_with_several_sounds_is_named_by_trigger_and_wem() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "play_multi", &[], 1)?;
    assert_eq!(
        names(&fixture.out)?,
        [
            "Play_Multi_100.ogg",
            "Play_Multi_100.ogg.p4kx-origin.jsonl",
            "Play_Multi_101.wav",
            "Play_Multi_101.wav.p4kx-origin.jsonl",
            "Play_Multi_102.wem",
            "Play_Multi_102.wem.p4kx-origin.jsonl",
            "Play_Multi_103.wav",
            "Play_Multi_103.wav.p4kx-origin.jsonl",
        ]
    );
    assert_eq!(report["format"], "ogg");
    assert_eq!(count(&report, "written"), Some(4));
    assert_eq!(count(&report, "unconverted"), Some(1));
    assert_eq!(count(&report, "failed"), Some(1));
    let failure = report["failures"][0]["wem_id"].as_u64();
    assert_eq!(failure, Some(104));
    for (file, kind) in report["files"]
        .as_array()
        .ok_or("reported sound files")?
        .iter()
        .zip(["oggBytes", "wavBytes", "opaquePayload", "wavBytes"])
    {
        assert_sound_receipt(file, kind)?;
    }
    let ogg = fs::read(fixture.out.join("Play_Multi_100.ogg"))?;
    OggStreamReader::new(Cursor::new(ogg))?;
    assert_eq!(fs::read(fixture.out.join("Play_Multi_102.wem"))?, xma());
    let wav = fs::read(fixture.out.join("Play_Multi_101.wav"))?;
    assert_eq!(wav.get(..4), Some(&b"RIFF"[..]));
    Ok(())
}

#[test]
fn raw_keeps_the_wem_bytes() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Single", &["--format", "raw"], 0)?;
    assert_eq!(report["format"], "raw");
    assert_eq!(report["files"][0]["format"], "wem");
    assert_eq!(fs::read(fixture.out.join("Play_Single.wem"))?, pcm());
    Ok(())
}

#[test]
fn conflict_policies_decide_about_existing_files() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture, "Play_Single", &[], 0)?;
    let target = fixture.out.join("Play_Single.wav");
    let receipt = fixture.out.join("Play_Single.wav.p4kx-origin.jsonl");
    let previous = fs::read(&receipt)?;
    fs::write(&target, b"old")?;
    let skipped = extract(&fixture, "Play_Single", &[], 0)?;
    assert_eq!(count(&skipped, "kept"), Some(1));
    assert_eq!(
        skipped["files"][0].get("origin_receipt"),
        Some(&Value::Null)
    );
    assert_eq!(fs::read(&target)?, b"old");
    assert_eq!(fs::read(&receipt)?, previous);
    let replaced = extract(&fixture, "Play_Single", &["--conflict", "if-changed"], 0)?;
    assert_eq!(count(&replaced, "written"), Some(1));
    assert_sound_receipt(&replaced["files"][0], "wavBytes")?;
    let published = fs::read(&receipt)?;
    let unchanged = extract(&fixture, "Play_Single", &["--conflict", "if-changed"], 0)?;
    assert_eq!(count(&unchanged, "kept"), Some(1));
    assert_eq!(
        unchanged["files"][0].get("origin_receipt"),
        Some(&Value::Null)
    );
    assert_eq!(fs::read(&receipt)?, published);
    fs::write(&target, b"old")?;
    let overwritten = extract(&fixture, "Play_Single", &["--conflict", "overwrite"], 0)?;
    assert_eq!(count(&overwritten, "written"), Some(1));
    assert_ne!(fs::read(&target)?, b"old");
    Ok(())
}
