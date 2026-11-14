// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::Duration;

use p4kx_audio::{Codec, Wem};
use p4kx_testkit::{TestCodec, TestWem};

use super::TestResult;

#[test]
fn selected_empty_vorb_falls_back_to_the_embedded_fmt_count() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Vorbis, 1, 48_000, 24_000)
        .chunk(*b"vorb", 12_000_u32.to_le_bytes())
        .chunk(*b"vorb", [])
        .data([1, 2, 3, 4])
        .bytes();
    let wem = Wem::parse(bytes.as_slice())?;
    assert_eq!(
        wem.codec(),
        Codec::Vorbis,
        "the selected fmt declares Vorbis"
    );
    assert_eq!(
        bytes.get(16..20),
        Some(66_u32.to_le_bytes().as_slice()),
        "the selected fmt has the exact embedded-count length"
    );
    assert_eq!(
        bytes.get(44..48),
        Some(24_000_u32.to_le_bytes().as_slice()),
        "the physical fmt count word"
    );
    assert_eq!(
        wem.chunk(*b"vorb"),
        Some(12_000_u32.to_le_bytes().as_slice()),
        "the earlier readable vorb remains physically present"
    );
    assert_eq!(
        bytes.get(98..110),
        Some(b"vorb\0\0\0\0data".as_slice()),
        "the empty selected vorb ends at the first data tag"
    );
    assert_eq!(wem.data(), [1, 2, 3, 4], "the actual first data body");

    assert_eq!(
        wem.sample_count(),
        Some(24_000),
        "an empty selected vorb falls back to fmt instead of reading the next data ID"
    );
    assert_eq!(
        wem.duration(),
        Some(Duration::from_millis(500)),
        "the embedded count at 48 kHz gives half a second"
    );
    Ok(())
}

#[test]
fn selected_short_vorb_without_an_embedded_count_has_no_duration() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Tag(0xffff), 1, 48_000, 24_000)
        .chunk(*b"vorb", [0x34, 0x12])
        .data([1, 2, 3, 4])
        .bytes();
    let wem = Wem::parse(bytes.as_slice())?;
    assert_eq!(
        wem.codec(),
        Codec::Vorbis,
        "the selected fmt declares Vorbis"
    );
    assert_eq!(
        bytes.get(16..20),
        Some(28_u32.to_le_bytes().as_slice()),
        "the selected fmt lacks the exact embedded-count layout"
    );
    assert_eq!(
        bytes.get(44..48),
        Some(24_000_u32.to_le_bytes().as_slice()),
        "the physical fmt count word"
    );
    assert_eq!(
        wem.chunk(*b"vorb"),
        Some([0x34, 0x12].as_slice()),
        "the selected vorb has only two body bytes"
    );
    assert_eq!(
        bytes.get(56..60),
        Some([0x34, 0x12, b'd', b'a'].as_slice()),
        "the old four-byte read crossed into the data tag"
    );

    assert_eq!(
        wem.sample_count(),
        None,
        "neither a two-byte vorb nor a 28-byte fmt holds a usable Vorbis count"
    );
    assert_eq!(wem.duration(), None, "no count means no playing time");
    Ok(())
}

#[test]
fn the_last_complete_vorb_before_data_wins() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Vorbis, 1, 48_000, 24_000)
        .chunk(*b"vorb", 12_000_u32.to_le_bytes())
        .chunk(*b"vorb", 48_000_u32.to_le_bytes())
        .data([1, 2, 3, 4])
        .tail_chunk(*b"vorb", 36_000_u32.to_le_bytes())
        .bytes();
    let wem = Wem::parse(bytes.as_slice())?;
    let counts: Vec<_> = wem
        .chunks()
        .filter(|chunk| chunk.id == *b"vorb")
        .map(|chunk| chunk.body)
        .collect();
    assert_eq!(
        counts,
        [
            12_000_u32.to_le_bytes().as_slice(),
            48_000_u32.to_le_bytes().as_slice(),
            36_000_u32.to_le_bytes().as_slice()
        ],
        "the three distinct original vorb bodies are visible in file order"
    );
    assert_eq!(
        wem.sample_count(),
        Some(48_000),
        "the second vorb is the last complete one before data"
    );
    assert_eq!(
        wem.duration(),
        Some(Duration::from_secs(1)),
        "the selected count at 48 kHz gives one second"
    );
    Ok(())
}
