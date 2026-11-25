// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::Wem;
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestOpus, TestOpusFraming, TestVorbis};
use serde_json::{Value, json};

use super::{SOUND, TestResult, field, read_receipt, wav};

#[test]
fn vorbis_wav_receipt_distinguishes_decoded_packets_from_lookahead() -> TestResult {
    let fixture = TestVorbis::new(2, 48_000, &[false; 40])
        .tone(7)
        .sample_count(38 * 128);
    let input = fixture.wem().tail_chunk(*b"data", [0xde, 0xad]).bytes();
    let parsed = Wem::parse(input.as_slice())?;
    let data = parsed.metadata().data_chunk();
    let start = data.body_offset + 2 + fixture.setup().len() as u64;
    let end = data.body_end_exclusive();
    let lookahead_bytes = fixture
        .packets()
        .last()
        .map(Vec::len)
        .ok_or("expected lookahead packet")?;
    let decoded_end = end - 2 - lookahead_bytes as u64;

    let directory = tempfile::tempdir()?;
    let loose = directory.path().join("audio.wem");
    fs::write(&loose, &input)?;
    let rows = run(&loose, None, directory.path(), &parsed)?;
    let samples = field(&rows, "pcmSamples")?;
    let length = field(&rows, "dataLength")?;
    assert_eq!(
        samples.get("containsEncryptedSource"),
        Some(&Value::Null),
        "loose input"
    );

    let uses = Uses(&rows);
    assert_eq!(
        uses.range(samples, "wemVorbis", "audioDecoded", "conversion")?,
        start..decoded_end,
        "only the original Vorbis prefix supplied to the PCM decoder"
    );
    assert_eq!(
        uses.range(samples, "wemVorbis", "audioLookahead", "selector")?,
        decoded_end..end,
        "the final packet participates in lookahead without being decoded"
    );
    assert_eq!(
        uses.range(length, "wemVorbis", "audioChecked", "selector")?,
        start..end,
        "the frame calculation checks the first DATA occurrence"
    );
    Ok(())
}

#[test]
fn opus_wav_receipt_limits_decoder_sources_to_the_replayed_prefix() -> TestResult {
    let input = TestOpus::new(TestOpusFraming::Wwise, 6, 9_600)
        .declared_sample_count(1_500)
        .output_gain(512)
        .wem()?
        .smpl_loop(0, 1, 2)
        .tail_chunk(*b"data", [0xa3, 0xd0])
        .bytes();
    let parsed = Wem::parse(input.as_slice())?;
    let data = parsed.metadata().data_chunk();
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("audio.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOUND, input.clone()).encrypted())
        .write_to(&archive)?;
    let rows = run(&archive, Some(SOUND), directory.path(), &parsed)?;
    let samples = field(&rows, "pcmSamples")?;
    let length = field(&rows, "dataLength")?;
    assert_eq!(
        samples
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "Archive owner"
    );

    let uses = Uses(&rows);
    let replay = uses.range(samples, "wemOpus", "audioReplayed", "conversion")?;
    let checked = uses.range(length, "wemOpus", "audioChecked", "selector")?;
    assert_eq!(
        checked,
        data.body_offset..data.body_end_exclusive(),
        "the preparation range stops at the first DATA occurrence"
    );
    assert_eq!(replay.start, checked.start, "original packet start");
    assert!(
        replay.end < checked.end,
        "authored trim excludes a packet suffix"
    );
    let replay_seek = uses.range(samples, "wemOpus", "seekTableReplayed", "selector")?;
    let checked_seek = uses.range(length, "wemOpus", "seekTableChecked", "selector")?;
    assert_eq!(
        replay_seek.start, checked_seek.start,
        "actual seek word start"
    );
    assert!(
        replay_seek.end < checked_seek.end,
        "unreplayed seek words excluded"
    );
    Ok(())
}

fn run(
    input: &Path,
    entry: Option<&str>,
    directory: &Path,
    original: &Wem<&[u8]>,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let output = directory.join("output.wav");
    let result = wav(input, entry, &output)?;
    assert!(result.status.success(), "{result:?}");
    let summary: Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(
        summary.get("originReceipt").and_then(Value::as_str),
        Some("output.wav.p4kx-origin.jsonl"),
        "receipt name"
    );
    let actual = fs::read(&output)?;
    let mut expected = Vec::new();
    let report = original.write_wav(&mut expected)?;
    assert_eq!(actual, expected, "existing decoded bytes");
    assert_eq!(
        summary.get("sample_count").and_then(Value::as_u64),
        Some(report.sample_count),
        "written sample count"
    );
    assert_eq!(
        summary.get("size").and_then(Value::as_u64),
        Some(report.size),
        "written file size"
    );
    let rows = read_receipt(&directory.join("output.wav.p4kx-origin.jsonl"))?;
    let riff = Wem::parse(actual.as_slice())?;
    let data = riff.metadata().data_chunk();
    let samples = field(&rows, "pcmSamples")?;
    let length = field(&rows, "dataLength")?;
    assert_eq!(
        samples.get("address"),
        Some(&json!({
            "type": "wavBytes",
            "field": "pcmSamples",
            "start": data.body_offset.to_string(),
            "endExclusive": data.body_end_exclusive().to_string()
        })),
        "actual output PCM bytes"
    );
    assert_eq!(
        length.pointer("/address/start").and_then(Value::as_str),
        Some((data.body_offset - 4).to_string().as_str()),
        "selected output size word"
    );
    assert_eq!(
        samples.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "observed source"
    );
    assert_eq!(
        summary
            .pointer("/fieldOrigins/~1sample_count/sourceUsesRefs")
            .ok_or("origin of written frame count")?,
        length
            .get("sourceUsesRefs")
            .ok_or("data length source references")?,
        "summary shares the exact selected frame extent"
    );
    Ok(rows)
}

struct Uses<'rows>(&'rows [Value]);

impl Uses<'_> {
    fn range(
        &self,
        output: &Value,
        namespace: &str,
        field: &str,
        role: &str,
    ) -> Result<Range<u64>, Box<dyn Error>> {
        let references = output
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("WAV source references")?;
        let observation = self
            .0
            .iter()
            .filter_map(|row| row.get("sourceUse"))
            .find(|used| {
                used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                    && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && used.get("role").and_then(Value::as_str) == Some(role)
                    && used.get("id").is_some_and(|id| references.contains(id))
            })
            .ok_or_else(|| format!("missing bound {namespace}.{field} role {role}"))?;
        let Some([start, end]) = observation
            .pointer("/location/indices")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
        else {
            return Err("WAV input source range".into());
        };
        Ok(start.as_str().ok_or("start")?.parse()?..end.as_str().ok_or("end")?.parse()?)
    }
}
