// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::Wem;
use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use super::{SOUND, TestResult, contributors};

#[test]
fn wav_summary_uses_written_frame_extent_and_original_loop_selection() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("loop.p4k");
    let output = directory.path().join("loop.wav");
    let bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[1, 2, 3, 4])
        .smpl_loop(1, 0, 3)
        .smpl_loop(0, 1, 2)
        .bytes();
    let original = Wem::parse(bytes.as_slice())?;
    let data_body = original.info_source().data_body_offset();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOUND, bytes).encrypted())
        .write_to(&archive)?;

    let result = p4kx_testkit::p4kx()?
        .args(["--json", "audio", "wav"])
        .arg(archive)
        .arg(SOUND)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert_eq!(
        result.status.code(),
        Some(0),
        "WAV export failed: {result:?}"
    );
    let document: Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(
        document["sample_count"], 4,
        "four complete samples reach the WAV"
    );
    assert_eq!(
        document["size"],
        fs::metadata(&output)?.len(),
        "summary size follows the committed file"
    );
    assert_eq!(
        document["loop"],
        json!({"start":1,"end":3,"frames":2}),
        "the second loop is the first admitted forward loop"
    );
    assert_written_frame_origins(&document, data_body)?;
    assert_written_loop_origins(&document)?;
    Ok(())
}

fn assert_written_frame_origins(document: &Value, data_body: u64) -> TestResult {
    let extent = json!({
        "type":"subresource", "namespace":"wemChunk",
        "indices":[data_body.to_string()], "field":"bodyLength"
    });
    for field in ["/sample_count", "/size"] {
        let inputs = contributors(document, field, Some(true))?;
        assert!(
            inputs
                .iter()
                .any(|input| input.get("location") == Some(&extent)),
            "{field}"
        );
    }
    let uses = document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("published summary origins")?;
    assert!(
        uses.iter().all(
            |used| used.pointer("/location/field").and_then(Value::as_str) != Some("decodedAudio")
        ),
        "sample bytes are used for the waveform, while the summary uses the encoded length"
    );
    Ok(())
}

fn assert_written_loop_origins(document: &Value) -> TestResult {
    for field in ["/loop", "/loop/start", "/loop/end", "/loop/frames"] {
        let inputs = contributors(document, field, Some(true))?;
        assert!(
            inputs.iter().any(|input| {
                input.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("wemLoopPrefix")
                    && input.pointer("/location/indices/1").and_then(Value::as_str) == Some("2")
                    && input.pointer("/location/field").and_then(Value::as_str)
                        == Some("forwardAndFit")
            }),
            "{field} must retain the original two examined loop records"
        );
    }
    Ok(())
}
