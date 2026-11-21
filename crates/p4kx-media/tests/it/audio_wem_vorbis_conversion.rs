// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, AudioError, OpenOptions, VorbisError, Wem};
use p4kx_media::{Artifact, AudioOutput, ExportOptions, ExportOutcome, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestVorbis, test_key};
use serde_json::{Value, json};

use super::origins::Origins;
use super::{AUDIO_STREAM, TestResult, check_schema, options};
use crate::support;

#[test]
fn requested_vorbis_ogg_retains_exact_embedded_packet_ranges_and_encryption() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = TestVorbis::new(2, 48_000, &[false, true, false, false])
        .wem()
        .smpl_loop(0, 1, 2)
        .tail_chunk(*b"data", [0x70, 0x74])
        .bytes();
    let bank = TestBank::new(7).wem(49, bytes.clone()).bytes();
    let bank_offset = bank
        .windows(4)
        .position(|chunk| chunk == b"RIFF")
        .ok_or("embedded RIFF header")? as u64;
    let original = Wem::parse(bytes.as_slice())?;
    let start = original.metadata().data_chunk().body_offset;
    let setup = original.data().first_chunk::<2>().ok_or("setup prefix")?;
    let audio_start = start + 2 + u64::from(u16::from_le_bytes(*setup));
    let end = original.metadata().data_chunk().body_end_exclusive();
    let archive = encrypted_bank(folder.path(), &bank)?;
    let out = folder.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &out,
        &ExportOptions {
            audio_output: AudioOutput::Ogg,
            ..options()
        },
    )?)?;
    let records = support::records(&out, &index, AUDIO_STREAM)?;
    check_schema(&records)?;
    let (ordinal, row) = records
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == "audio-bank-wem-conversion")
        .ok_or("embedded Ogg conversion record")?;
    check_request(row);
    let payload = row
        .data
        .pointer("/payload/outcome/payload")
        .ok_or("written Ogg")?;
    let artifact: Artifact = serde_json::from_value(
        payload
            .get("artifact")
            .ok_or("verified Ogg artifact")?
            .clone(),
    )?;
    let base = out.join(&index.artifact_base);
    check_artifact(&base, &artifact, &original, payload, AudioOutput::Ogg)?;
    let fmt = original.metadata().info_source().format_body_offset();
    check_payload(payload, bank_offset, start, audio_start, end, fmt);
    check_origin(&Origins::read(&base)?, ordinal, start, audio_start, end)?;
    Ok(())
}

fn check_artifact(
    base: &std::path::Path,
    artifact: &Artifact,
    original: &Wem<&[u8]>,
    payload: &Value,
    output: AudioOutput,
) -> TestResult {
    let mut expected = Vec::new();
    let frames = match output {
        AudioOutput::Ogg => original.write_ogg(&mut expected)?.sample_count,
        AudioOutput::Wav => original.write_wav(&mut expected)?.sample_count,
        _ => return Err("expected audio conversion".into()),
    };
    assert_eq!(
        fs::read(base.join(&artifact.path))?,
        expected,
        "deterministic output bytes match standalone conversion"
    );
    assert_eq!(
        payload.get("sample_count"),
        Some(&json!(frames.to_string())),
        "actual complete output frame count"
    );
    Ok(())
}

fn check_request(row: &MediaRecord) {
    assert_eq!(row.status, Status::Complete, "Vorbis conversion succeeds");
    assert_eq!(row.subresource.indices, [1, 0], "original embedded WEM");
    assert_eq!(
        row.data.pointer("/payload/requested_format"),
        Some(&json!("ogg")),
        "requested Ogg container"
    );
    assert_eq!(
        row.data.pointer("/payload/source_codec"),
        Some(&json!("vorbis")),
        "source codec"
    );
}

fn check_payload(
    payload: &Value,
    bank_offset: u64,
    start: u64,
    audio_start: u64,
    end: u64,
    fmt: u64,
) {
    assert_eq!(
        payload.get("actual_codec"),
        Some(&json!("vorbis")),
        "actual Ogg codec"
    );
    assert_eq!(
        payload.get("actual_format"),
        Some(&json!("ogg")),
        "actual output container"
    );
    for (path, start, end) in [
        ("/source_range", audio_start, end),
        ("/vorbis_inputs/setup_packet", start, audio_start),
        ("/vorbis_inputs/average_byte_rate", fmt + 8, fmt + 12),
        ("/vorbis_inputs/header", fmt + 24, fmt + 66),
    ] {
        assert_eq!(
            payload.pointer(path),
            Some(&json!({
                "basis":{"type":"entry"},
                "start":bank_offset + start,
                "end_exclusive":bank_offset + end
            })),
            "{path} has the exact enclosing bank coordinates"
        );
    }
}

fn check_origin(
    origins: &Origins,
    ordinal: usize,
    start: u64,
    audio_start: u64,
    end: u64,
) -> TestResult {
    let uses = origins.for_field(
        ordinal,
        1,
        "/data/payload/outcome/payload/artifact/digest/hex",
    )?;
    for (field, start, end) in [
        ("setupPacket", start, audio_start),
        ("audioPackets", audio_start, end),
    ] {
        let expected = json!({
            "type":"subresource", "namespace":"wemVorbisInput", "field":field,
            "indices":["1","0", &start.to_string(), &end.to_string()]
        });
        assert!(
            uses.iter().any(|source| {
                source.get("location") == Some(&expected)
                    && source.get("role") == Some(&json!("conversion"))
            }),
            "{field} is an exact encrypted artifact input"
        );
    }
    Ok(())
}

#[test]
fn requested_vorbis_wav_keeps_decoded_and_lookahead_extents_of_an_encrypted_bank_wem() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let fixture = TestVorbis::new(2, 48_000, &[false; 5])
        .tone(3)
        .sample_count(3 * 128);
    let bytes = fixture.wem().smpl_loop(0, 1, 2).bytes();
    let bank = TestBank::new(7).wem(49, bytes.clone()).bytes();
    let offset = bank
        .windows(4)
        .position(|chunk| chunk == b"RIFF")
        .ok_or("embedded WAV conversion input")? as u64;
    let archive = encrypted_bank(folder.path(), &bank)?;
    let out = folder.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &out,
        &ExportOptions {
            audio_output: AudioOutput::Wav,
            ..options()
        },
    )?)?;
    let records = support::records(&out, &index, AUDIO_STREAM)?;
    check_schema(&records)?;
    let (ordinal, row) = records
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == "audio-bank-wem-conversion")
        .ok_or("embedded WAV conversion record")?;
    let payload = check_wav_payload(row)?;
    let artifact: Artifact =
        serde_json::from_value(payload.get("artifact").ok_or("WAV artifact")?.clone())?;
    let base = out.join(&index.artifact_base);
    let original = Wem::parse(bytes.as_slice())?;
    check_artifact(&base, &artifact, &original, payload, AudioOutput::Wav)?;
    check_wav_packet_proof(
        &fixture,
        &original,
        payload,
        offset,
        &Origins::read(&base)?,
        ordinal,
    )
}

fn check_wav_payload(row: &MediaRecord) -> TestResult<&Value> {
    assert_eq!(
        row.status,
        Status::Complete,
        "Vorbis WAV conversion succeeds"
    );
    assert_eq!(row.subresource.indices, [1, 0], "original embedded WEM");
    for (field, value) in [
        ("/payload/requested_format", "wav"),
        ("/payload/source_codec", "vorbis"),
        ("/payload/outcome/type", "written"),
        ("/payload/outcome/payload/actual_format", "wav"),
        ("/payload/outcome/payload/actual_codec", "pcm"),
    ] {
        assert_eq!(row.data.pointer(field), Some(&json!(value)), "{field}");
    }
    let payload = row
        .data
        .pointer("/payload/outcome/payload")
        .ok_or("written WAV")?;
    let mut mixed = serde_json::to_value(row)?;
    let inputs = mixed
        .pointer_mut("/data/payload/outcome/payload")
        .and_then(Value::as_object_mut)
        .ok_or("written PCM fields")?;
    let range = payload
        .pointer("/vorbis_inputs/header")
        .ok_or("original Vorbis header")?;
    inputs.insert(
        "opus_inputs".to_owned(),
        json!({
            "format":range, "seek_table":null, "embedded_headers":null, "checked_data":range
        }),
    );
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert!(
        !schema.is_valid(&mixed),
        "one PCM mapping cannot carry two source codecs' inputs"
    );
    Ok(payload)
}

fn check_wav_packet_proof(
    fixture: &TestVorbis,
    original: &Wem<&[u8]>,
    payload: &Value,
    offset: u64,
    origins: &Origins,
    ordinal: usize,
) -> TestResult {
    let data = original.metadata().data_chunk();
    let start = data.body_offset + 2 + fixture.setup().len() as u64;
    let final_packet = fixture.packets().pop().ok_or("authored lookahead packet")?;
    let checked_end = data.body_end_exclusive();
    let decoded_end = checked_end - 2 - final_packet.len() as u64;
    for (path, field, end) in [
        ("/source_range", "decodedAudioPackets", decoded_end),
        (
            "/vorbis_inputs/checked_audio_packets",
            "audioPackets",
            checked_end,
        ),
    ] {
        assert_eq!(
            payload.pointer(path),
            Some(&json!({
                "basis":{"type":"entry"},
                "start":offset + start, "end_exclusive":offset + end
            })),
            "{path} has exact enclosing-bank coordinates"
        );
        let uses =
            origins.for_field(ordinal, 1, &format!("/data/payload/outcome/payload{path}"))?;
        let expected = json!({
            "type":"subresource", "namespace":"wemVorbisInput", "field":field,
            "indices":["1","0", start.to_string(), end.to_string()]
        });
        assert!(
            uses.iter()
                .any(|source| source.get("location") == Some(&expected)),
            "{path} retains its exact encrypted source use"
        );
    }
    check_origin(origins, ordinal, data.body_offset, start, checked_end)
}

#[test]
fn undecodable_vorbis_wav_retains_a_diagnostic_without_a_partial_artifact() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut flags = [false; 24];
    flags[18] = true;
    flags[19] = true;
    let fixture = TestVorbis::new(1, 48_000, &flags).native_packets();
    let mut bytes = fixture.wem().bytes();
    let original = Wem::parse(bytes.as_slice())?;
    let data = usize::try_from(original.metadata().data_chunk().body_offset)?;
    let packet = data + 2 + fixture.setup().len() + 19 * 3 + 2;
    *bytes.get_mut(packet).ok_or("late native packet payload")? = 0b0010;
    let original = Wem::parse(bytes.as_slice())?;
    assert!(
        matches!(
            original.write_wav(&mut Vec::new()),
            Err(AudioError::InvalidVorbis(VorbisError::Undecodable { .. }))
        ),
        "the standalone decoder detects the late invalid packet"
    );
    let bank = TestBank::new(7).wem(49, bytes).bytes();
    let archive = encrypted_bank(folder.path(), &bank)?;
    let output = folder.path().join("out");
    let ExportOutcome::Incomplete { index, diagnostic } = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            audio_output: AudioOutput::Wav,
            diagnostic: true,
            ..options()
        },
    )?
    else {
        return Err("Vorbis decode failure must retain a diagnostic generation".into());
    };
    let directory = diagnostic.parent().ok_or("diagnostic directory")?;
    let rows = support::records(directory, &index, AUDIO_STREAM)?;
    check_schema(&rows)?;
    let row = rows
        .iter()
        .find(|row| row.subresource.namespace == "audio-bank-wem-conversion")
        .ok_or("failed Vorbis conversion receipt")?;
    assert_eq!(row.status, Status::Partial, "original source is retained");
    assert_eq!(
        row.data.pointer("/payload/outcome/payload/type"),
        Some(&json!("invalid-representation")),
        "typed decoder failure is retained"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "no complete index"
    );
    for file in fs::read_dir(directory.join(index.artifact_base).join("chunks"))? {
        assert!(
            file?
                .path()
                .extension()
                .is_none_or(|extension| extension != "wav"),
            "there is no partial WAV artifact"
        );
    }
    Ok(())
}
fn encrypted_bank(directory: &std::path::Path, bank: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("notes.txt", b"fixture".as_slice()))
        .entry(TestEntry::zstd("samples.bnk", bank).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
