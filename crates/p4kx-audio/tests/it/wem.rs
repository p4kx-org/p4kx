// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Parsing WEM headers: RIFF and RIFX, the Wwise `fmt` variants, codec detection and corrupt
//! input.

use std::time::Duration;

use p4kx_audio::{AudioError, ByteOrder, Codec, Wem};
use p4kx_testkit::{TestCodec, TestWem, test_signal};

type TestResult = Result<(), AudioError>;

#[test]
fn a_pcm_wem_with_each_fmt_layout_reports_its_format() -> TestResult {
    let samples = test_signal(480, 2);
    for codec in [
        TestCodec::Pcm,
        TestCodec::PcmWithExtension,
        TestCodec::WwisePcm,
    ] {
        let bytes = TestWem::pcm(codec, 2, 48_000, &samples).bytes();

        let wem = Wem::parse(bytes.as_slice())?;

        assert_eq!(wem.codec(), Codec::Pcm, "{codec:?} is PCM");
        assert_eq!(wem.channels(), 2, "{codec:?} channels");
        assert_eq!(wem.sample_rate(), 48_000, "{codec:?} sample rate");
        assert_eq!(wem.sample_count(), Some(480), "{codec:?} frames");
        assert_eq!(
            wem.duration(),
            Some(Duration::from_millis(10)),
            "{codec:?} duration"
        );
        assert_eq!(wem.byte_order(), ByteOrder::Little, "{codec:?} is RIFF");
        assert_eq!(wem.data().len(), 480 * 4, "{codec:?} data chunk");
    }
    Ok(())
}

#[test]
fn a_big_endian_rifx_wem_is_read_with_big_endian_fields() -> TestResult {
    let samples = test_signal(100, 1);
    let bytes = TestWem::pcm(TestCodec::WwisePcm, 1, 44_100, &samples)
        .big_endian()
        .bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.byte_order(), ByteOrder::Big, "RIFX is big-endian");
    assert_eq!(wem.codec(), Codec::Pcm, "the tag is read big-endian");
    assert_eq!(wem.channels(), 1, "channels");
    assert_eq!(wem.sample_rate(), 44_100, "sample rate");
    assert_eq!(wem.sample_count(), Some(100), "frames");
    Ok(())
}

#[test]
fn every_wwise_format_tag_is_detected() -> TestResult {
    let cases = [
        (0x0002, Codec::ImaAdpcm),
        (0x0011, Codec::ImaAdpcm),
        (0x0069, Codec::ImaAdpcm),
        (0x0161, Codec::Xwma),
        (0x0162, Codec::Xwma),
        (0x0165, Codec::Xma2),
        (0x0166, Codec::Xma2),
        (0xAAC0, Codec::Aac),
        (0xFFF0, Codec::Dsp),
        (0xFFFB, Codec::Hevag),
        (0xFFFC, Codec::Atrac9),
        (0x3039, Codec::Opus),
        (0x3040, Codec::Opus),
        (0x3041, Codec::Opus),
        (0x8311, Codec::PtAdpcm),
        (0x1234, Codec::Unknown(0x1234)),
    ];
    for (tag, codec) in cases {
        let bytes = TestWem::encoded(TestCodec::Tag(tag), 2, 48_000, 960)
            .block_align(0x48)
            .bytes();

        let wem = Wem::parse(bytes)?;

        assert_eq!(wem.codec(), codec, "tag {tag:#06x}");
        assert_eq!(wem.format_tag(), tag, "tag {tag:#06x} is kept");
    }
    Ok(())
}

#[test]
fn wwise_vorbis_takes_its_sample_count_from_the_fmt_chunk() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Vorbis, 2, 48_000, 72_000).bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.codec(), Codec::Vorbis, "tag 0xFFFF");
    assert_eq!(wem.sample_count(), Some(72_000), "frames");
    assert_eq!(
        wem.duration(),
        Some(Duration::from_millis(1500)),
        "duration"
    );
    assert_eq!(wem.codec().to_string(), "Wwise Vorbis", "display name");
    Ok(())
}

#[test]
fn older_wwise_vorbis_takes_its_sample_count_from_the_vorb_chunk() -> TestResult {
    let mut vorb = 72_000u32.to_le_bytes().to_vec();
    vorb.resize(0x2A, 0);
    let bytes = TestWem::encoded(TestCodec::Tag(0xFFFF), 2, 48_000, 960)
        .chunk(*b"vorb", vorb)
        .bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.codec(), Codec::Vorbis, "tag 0xFFFF");
    assert_eq!(wem.sample_count(), Some(72_000), "the vorb chunk's count");
    Ok(())
}

#[test]
fn wwise_vorbis_with_another_fmt_size_and_no_vorb_chunk_has_no_sample_count() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Tag(0xFFFF), 2, 48_000, 960).bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.codec(), Codec::Vorbis, "tag 0xFFFF");
    assert_eq!(
        wem.sample_count(),
        None,
        "offset 0x18 of a 28-byte fmt chunk is not a sample count"
    );
    assert_eq!(wem.duration(), None, "no duration");
    Ok(())
}
#[test]
fn opus_takes_its_sample_count_from_the_fmt_chunk() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Tag(0x3041), 1, 48_000, 24_000).bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.sample_count(), Some(24_000), "frames");
    Ok(())
}

#[test]
fn adpcm_sample_counts_follow_from_the_block_size() -> TestResult {
    let ima = TestWem::encoded(TestCodec::Tag(0x0002), 2, 48_000, 0)
        .block_align(0x48)
        .data(vec![0; 0x48 * 3])
        .bytes();
    let ptadpcm = TestWem::encoded(TestCodec::Tag(0x8311), 1, 48_000, 0)
        .block_align(0x24)
        .data(vec![0; 0x24 * 2])
        .bytes();

    assert_eq!(
        Wem::parse(ima)?.sample_count(),
        Some(3 * 64),
        "IMA: 4 header bytes, the header sample and 63 of 64 nibbles per channel and block"
    );
    assert_eq!(
        Wem::parse(ptadpcm)?.sample_count(),
        Some(2 * 64),
        "PTADPCM: 5 header bytes and 62 nibbles per channel and frame"
    );
    Ok(())
}

#[test]
fn chunks_before_data_are_skipped_including_odd_sized_ones() -> TestResult {
    let samples = test_signal(10, 1);
    let bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &samples)
        .chunk(*b"hash", [7; 16])
        .chunk(*b"junk", [0; 3])
        .chunk(*b"akd ", [1; 8])
        .bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.sample_count(), Some(10), "the data chunk is found");
    Ok(())
}

#[test]
fn an_odd_sized_data_chunk_at_the_end_needs_no_pad_byte() -> TestResult {
    let bytes = TestWem::encoded(TestCodec::Vorbis, 1, 48_000, 10)
        .data(vec![1; 255])
        .bytes();

    let wem = Wem::parse(bytes)?;

    assert_eq!(wem.data().len(), 255, "the whole data chunk");
    Ok(())
}

fn parse_error(bytes: &[u8]) -> Option<AudioError> {
    Wem::parse(bytes).err()
}

#[test]
fn input_that_is_not_a_wem_is_rejected() {
    assert!(
        matches!(parse_error(b""), Some(AudioError::Truncated { .. })),
        "empty input"
    );
    assert!(
        matches!(parse_error(b"OggS\0\0\0\0WAVE"), Some(AudioError::NotWem)),
        "another magic"
    );
    assert!(
        matches!(
            parse_error(b"PLUG\x08\0\0\0hash\0\0\0\0"),
            Some(AudioError::PluginMedia)
        ),
        "Wwise plug-in media"
    );
    assert!(
        matches!(parse_error(b"RIFF\x04\0\0\0AVI "), Some(AudioError::NotWem)),
        "a RIFF file that is not WAVE"
    );
}

#[test]
fn truncated_wems_are_rejected() {
    let bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &test_signal(100, 1)).bytes();
    for len in [4, 11, 12, 20, 30, 40, bytes.len() - 1] {
        let error = parse_error(bytes.get(..len).unwrap_or_default());
        assert!(
            matches!(error, Some(AudioError::Truncated { .. })),
            "{len} bytes give {error:?}"
        );
    }
}

#[test]
fn a_wem_without_fmt_or_data_is_rejected() {
    let no_fmt = b"RIFF\x0c\0\0\0WAVEdata\0\0\0\0";
    assert!(
        matches!(parse_error(no_fmt), Some(AudioError::MissingChunk("fmt "))),
        "no fmt chunk"
    );
    let pcm = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[]).bytes();
    let mut fmt_only = pcm.get(..36).unwrap_or_default().to_vec();
    fmt_only.splice(4..8, 28u32.to_le_bytes());
    assert!(
        matches!(
            parse_error(&fmt_only),
            Some(AudioError::MissingChunk("data"))
        ),
        "no data chunk"
    );
}

#[test]
fn contradictory_fmt_fields_are_rejected() {
    let cases = [
        TestWem::pcm(TestCodec::Pcm, 0, 48_000, &[]).bytes(),
        TestWem::pcm(TestCodec::Pcm, 1, 0, &[1, 2]).bytes(),
        TestWem::pcm(TestCodec::Pcm, 2, 48_000, &[1, 2])
            .block_align(3)
            .bytes(),
    ];
    for bytes in cases {
        assert!(
            matches!(parse_error(&bytes), Some(AudioError::InvalidFormat(_))),
            "{bytes:02x?}"
        );
    }
}

#[test]
fn a_short_fmt_chunk_is_rejected() {
    let bytes = b"RIFF\x1c\0\0\0WAVEfmt \x08\0\0\0\x01\0\x01\0\x80\xbb\0\0data\0\0\0\0";

    assert!(
        matches!(parse_error(bytes), Some(AudioError::InvalidFormat(_))),
        "an 8-byte fmt chunk"
    );
}

#[test]
fn chunk_sizes_beyond_the_file_are_rejected_without_overflow() {
    let mut bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[1, 2, 3]).bytes();
    if let Some(size) = bytes.get_mut(16..20) {
        size.copy_from_slice(&u32::MAX.to_le_bytes());
    }

    assert!(
        matches!(parse_error(&bytes), Some(AudioError::Truncated { .. })),
        "a fmt chunk of 4 GiB"
    );
}

mod vorb_short;
