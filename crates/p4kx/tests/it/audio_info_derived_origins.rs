// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod evidence;
mod fixture;

use std::error::Error;
use std::fs;

use p4kx_extraction::Wem;
use serde_json::{Value, json};

use self::evidence::{assert_site, exclude_chunks, exclude_site};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const PCM_FORMAT: u64 = 124;
const PCM_SMPL: u64 = 160;
const PCM_DATA: u64 = 300;
const VORBIS_FORMAT: u64 = 20;
const EMPTY_VORB: u64 = 116;

#[test]
fn rifx_info_uses_the_last_pre_data_format_and_loop_with_the_first_data_length() -> TestResult {
    let bytes = fixture::rifx_pcm()?;
    check_original_pcm(&bytes)?;
    let document = audio_info(&bytes)?;

    for (field, expected) in [
        ("codec", json!("pcm")),
        ("codec_name", json!("PCM")),
        ("format_tag", json!(0xfffe)),
        ("channels", json!(2)),
        ("sample_rate", json!(32_000)),
        ("sample_count", json!(8)),
        ("duration_seconds", json!(0.00025)),
        ("loop", json!({"start": 2, "end": 8, "frames": 6})),
    ] {
        assert_eq!(document.get(field), Some(&expected), "{field}");
    }

    assert_site(
        &document,
        "/channels",
        "wemChunk",
        &[PCM_FORMAT],
        "channels",
        "payload",
    )?;
    assert_site(
        &document,
        "/sample_count",
        "wemRiffHeader",
        &[],
        "signature",
        "conversion",
    )?;
    for field in [
        "/sample_count",
        "/duration_seconds",
        "/loop/end",
        "/loop/frames",
    ] {
        assert_site(
            &document,
            field,
            "wemChunk",
            &[PCM_DATA],
            "bodyLength",
            "conversion",
        )?;
    }
    check_loop_sites(&document)?;
    exclude_chunks(&document, &[20, 44, 112, 148, 340, 364, 432])
}

fn check_original_pcm(bytes: &[u8]) -> TestResult {
    assert_eq!(
        bytes.get(..4),
        Some(b"RIFX".as_slice()),
        "the fixture selects big-endian WAVE"
    );
    let wem = Wem::parse(bytes)?;
    let first_format = wem.chunk(*b"fmt ").ok_or("first format body")?;
    assert_eq!(
        first_format.get(2..4),
        Some(1_u16.to_be_bytes().as_slice()),
        "the first fmt actually declares mono"
    );
    assert_eq!(
        bytes.get(124..128),
        Some([0xff, 0xfe, 0, 2].as_slice()),
        "the later pre-data fmt declares the stereo extensible format"
    );
    assert_eq!(
        bytes.get(128..132),
        Some(32_000_u32.to_be_bytes().as_slice()),
        "the later pre-data fmt declares a distinct big-endian rate"
    );
    assert_eq!(
        bytes.get(292..296),
        Some(b"data".as_slice()),
        "this is the actual first data header"
    );
    assert_eq!(
        bytes.get(296..300),
        Some(32_u32.to_be_bytes().as_slice()),
        "the first data header declares 32 bytes"
    );
    assert_eq!(
        bytes.get(300..332),
        Some(wem.data()),
        "the parser selects the first data body"
    );
    assert_eq!(
        bytes.get(152..156),
        Some(b"smpl".as_slice()),
        "this is the actual second pre-data smpl header"
    );
    assert_eq!(
        bytes.get(188..192),
        Some(4_u32.to_be_bytes().as_slice()),
        "the chosen smpl declares four loops"
    );
    assert_eq!(
        bytes.get(248..260),
        Some([0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0].as_slice()),
        "the third loop is forward, starts at 2, and has a raw zero end"
    );
    assert_eq!(
        wem.loop_points().map(|points| (points.start, points.end)),
        Some((2, 8)),
        "the decoder selects the first fitting forward loop"
    );
    Ok(())
}

fn check_loop_sites(document: &Value) -> TestResult {
    for field in ["/loop", "/loop/start", "/loop/end", "/loop/frames"] {
        assert_site(
            document,
            field,
            "wemLoopPrefix",
            &[PCM_SMPL, 3],
            "forwardAndFit",
            "selector",
        )?;
    }
    let slot = [PCM_SMPL, 2];
    assert_site(
        document,
        "/loop/start",
        "wemLoop",
        &slot,
        "start",
        "payload",
    )?;
    assert_site(
        document,
        "/loop/end",
        "wemLoop",
        &slot,
        "lastFrame",
        "selector",
    )?;
    assert_site(
        document,
        "/loop/frames",
        "wemLoop",
        &slot,
        "start",
        "conversion",
    )
}

#[test]
fn empty_selected_vorb_uses_the_embedded_count_without_reading_the_next_chunk_id() -> TestResult {
    let bytes = fixture::empty_vorb();
    check_original_vorb(&bytes)?;
    let document = audio_info(&bytes)?;
    assert_eq!(
        document.get("codec"),
        Some(&json!("vorbis")),
        "the selected format still reports Vorbis"
    );
    assert_eq!(
        document.get("sample_count"),
        Some(&json!(24_000)),
        "the selected vorb is empty and the next data tag is outside its body"
    );
    assert_eq!(
        document.get("duration_seconds"),
        Some(&json!(0.5)),
        "the fallback count plays for half a second"
    );
    for field in ["/sample_count", "/duration_seconds"] {
        assert_site(
            &document,
            field,
            "wemChunk",
            &[EMPTY_VORB],
            "bodyLength",
            "selector",
        )?;
    }
    assert_site(
        &document,
        "/sample_count",
        "wemChunk",
        &[VORBIS_FORMAT],
        "sampleCount",
        "payload",
    )?;
    exclude_site(&document, "wemChunk", &[EMPTY_VORB], "sampleCount")?;
    exclude_chunks(&document, &[94, 106, 136])
}

fn check_original_vorb(bytes: &[u8]) -> TestResult {
    let wem = Wem::parse(bytes)?;
    assert_eq!(
        bytes.get(12..16),
        Some(b"fmt ".as_slice()),
        "the first chunk is the original Vorbis fmt"
    );
    assert_eq!(
        bytes.get(16..20),
        Some(66_u32.to_le_bytes().as_slice()),
        "the actual fmt length permits the embedded count"
    );
    assert_eq!(
        bytes.get(44..48),
        Some(24_000_u32.to_le_bytes().as_slice()),
        "the original fmt holds the embedded count"
    );
    assert_eq!(
        wem.chunk(*b"vorb"),
        Some(12_000_u32.to_le_bytes().as_slice()),
        "the first vorb physically holds a different count"
    );
    assert_eq!(
        bytes.get(108..112),
        Some(b"vorb".as_slice()),
        "the selected vorb is the second pre-data chunk of that tag"
    );
    assert_eq!(
        bytes.get(112..116),
        Some(0_u32.to_le_bytes().as_slice()),
        "the selected vorb has no body bytes"
    );
    let following = bytes.get(116..120).ok_or("chunk ID after the empty vorb")?;
    assert_eq!(
        following, b"data",
        "the next four bytes belong to the data ID"
    );
    assert_eq!(
        u32::from_le_bytes(following.try_into()?),
        1_635_017_060,
        "the out-of-chunk read would decode this tag as a count"
    );
    assert_eq!(
        wem.data(),
        [1, 2, 3, 4],
        "the first actual data body starts after its header"
    );
    Ok(())
}

#[test]
fn switch_opus_info_keeps_the_rate_guard_without_using_seek_or_packet_bytes() -> TestResult {
    let bytes = fixture::switch_opus();
    check_original_switch(&bytes);

    let document = audio_info(&bytes)?;
    assert_eq!(
        document.get("codec"),
        Some(&json!("opus")),
        "the selected Switch format maps to Opus"
    );
    assert_eq!(
        document.get("format_tag"),
        Some(&json!(fixture::SWITCH_FORMAT_TAG)),
        "the raw Switch format tag remains visible"
    );
    assert_eq!(
        document.get("sample_rate"),
        Some(&json!(48_000)),
        "the displayed Opus playback rate is fixed at 48 kHz"
    );
    assert_eq!(
        document.get("sample_count"),
        Some(&Value::Null),
        "a different authored Switch rate rules out the fmt count"
    );
    assert_eq!(
        document.get("duration_seconds"),
        Some(&Value::Null),
        "the withheld count also withholds duration"
    );
    assert_site(
        &document,
        "/sample_rate",
        "wemChunk",
        &[20],
        "formatTag",
        "selector",
    )?;
    for field in ["/sample_count", "/duration_seconds"] {
        assert_site(
            &document,
            field,
            "wemChunk",
            &[20],
            "sampleRate",
            "selector",
        )?;
    }
    exclude_site(&document, "wemChunk", &[20], "sampleCount")?;
    exclude_chunks(&document, &[56, 68])
}

fn check_original_switch(bytes: &[u8]) {
    assert_eq!(
        bytes.get(24..28),
        Some(24_000_u32.to_le_bytes().as_slice()),
        "the Switch fmt declares a 24 kHz source rate"
    );
    assert_eq!(
        bytes.get(44..48),
        Some(1_200_u32.to_le_bytes().as_slice()),
        "the rejected fmt field contains a nonzero count"
    );
    assert_eq!(
        bytes.get(48..52),
        Some(b"seek".as_slice()),
        "a real seek chunk precedes the data"
    );
    assert_eq!(
        bytes.get(56..59),
        Some([0xff, 0xfe, 0xfd].as_slice()),
        "the seek body is opaque to audio info"
    );
    assert_eq!(
        bytes.get(68..),
        Some([0xff, 0xfe, 0xfd, 0xfc].as_slice()),
        "the first data body holds the unparsed packet bytes"
    );
}

fn audio_info(bytes: &[u8]) -> TestResult<Value> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("derived-info.wem");
    fs::write(&file, bytes)?;
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "audio", "info"])
        .arg(file)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}
