// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writing PCM WEMs as WAV, and the typed error for codecs without a decoder.

use p4kx_audio::{AudioError, Codec, WavOutputRanges, WavReport, Wem};
use p4kx_testkit::{TestCodec, TestWem, test_signal};

type TestResult = Result<(), AudioError>;

pub(super) fn assert_output_ranges(
    bytes: &[u8],
    ranges: &WavOutputRanges,
    report: WavReport,
) -> TestResult {
    let wave = Wem::parse(bytes)?;
    let data = wave.metadata().data_chunk();
    assert_eq!(
        ranges.header,
        0..data.body_offset,
        "original WAV header boundary"
    );
    assert_eq!(
        ranges.samples,
        data.body_offset..data.body_end_exclusive(),
        "the output range covers exactly the written sample bytes"
    );
    assert_eq!(report.size, bytes.len() as u64, "complete output size");
    assert_eq!(
        data.body_len,
        report.sample_count * u64::from(report.channels) * 2,
        "samples are interleaved 16-bit PCM"
    );
    Ok(())
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn samples(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(i16::from_le_bytes)
        .collect()
}

#[test]
fn a_stereo_pcm_wem_becomes_a_canonical_wav_with_the_same_samples() -> TestResult {
    let signal = test_signal(480, 2);
    let wem = Wem::parse(TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &signal).bytes())?;
    let mut wav = Vec::new();

    let report = wem.write_wav(&mut wav)?;

    assert_eq!(wav.get(..4), Some(&b"RIFF"[..]), "RIFF magic");
    assert_eq!(u32_at(&wav, 4), Some(36 + 480 * 4), "RIFF size");
    assert_eq!(wav.get(8..16), Some(&b"WAVEfmt "[..]), "WAVE and fmt");
    assert_eq!(u32_at(&wav, 16), Some(16), "a plain 16-byte fmt chunk");
    assert_eq!(u16_at(&wav, 20), Some(1), "PCM");
    assert_eq!(u16_at(&wav, 22), Some(2), "channels");
    assert_eq!(u32_at(&wav, 24), Some(48_000), "sample rate");
    assert_eq!(u32_at(&wav, 28), Some(48_000 * 4), "byte rate");
    assert_eq!(u16_at(&wav, 32), Some(4), "block align");
    assert_eq!(u16_at(&wav, 34), Some(16), "bits per sample");
    assert_eq!(wav.get(36..40), Some(&b"data"[..]), "data chunk");
    assert_eq!(u32_at(&wav, 40), Some(480 * 4), "data size");
    assert_eq!(
        samples(wav.get(44..).unwrap_or_default()),
        signal,
        "the samples"
    );
    assert_eq!(report.sample_count, 480, "frames written");
    assert_eq!(report.size, 44 + 480 * 4, "bytes written");
    Ok(())
}

#[test]
fn big_endian_pcm_is_swapped_to_little_endian() -> TestResult {
    let signal = test_signal(1000, 1);
    let bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &signal)
        .big_endian()
        .bytes();
    let mut wav = Vec::new();

    Wem::parse(bytes)?.write_wav(&mut wav)?;

    assert_eq!(
        samples(wav.get(44..).unwrap_or_default()),
        signal,
        "the samples"
    );
    Ok(())
}

#[test]
fn more_than_two_channels_become_wave_format_extensible_with_the_channel_mask() -> TestResult {
    let signal = test_signal(16, 6);
    let wem = Wem::parse(TestWem::pcm(TestCodec::WwisePcm, 6, 48_000, &signal).bytes())?;
    let mut wav = Vec::new();

    wem.write_wav(&mut wav)?;

    assert_eq!(u32_at(&wav, 16), Some(40), "a 40-byte fmt chunk");
    assert_eq!(u16_at(&wav, 20), Some(0xFFFE), "WAVE_FORMAT_EXTENSIBLE");
    assert_eq!(u16_at(&wav, 36), Some(22), "extension size");
    assert_eq!(u16_at(&wav, 38), Some(16), "valid bits");
    assert_eq!(u32_at(&wav, 40), Some(0x3F), "5.1 channel mask");
    assert_eq!(
        wav.get(44..60),
        Some(&b"\x01\x00\x00\x00\x00\x00\x10\x00\x80\x00\x00\xaa\x00\x38\x9b\x71"[..]),
        "KSDATAFORMAT_SUBTYPE_PCM"
    );
    assert_eq!(wav.get(60..64), Some(&b"data"[..]), "data chunk");
    assert_eq!(
        samples(wav.get(68..).unwrap_or_default()),
        signal,
        "the samples"
    );
    Ok(())
}

#[test]
fn a_trailing_partial_frame_is_dropped() -> TestResult {
    let signal = test_signal(10, 2);
    let mut data: Vec<u8> = signal
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    data.extend_from_slice(&[1, 2, 3]);
    let wem = Wem::parse(
        TestWem::pcm(TestCodec::Pcm, 2, 48_000, &signal)
            .data(data)
            .bytes(),
    )?;
    let mut wav = Vec::new();

    let report = wem.write_wav(&mut wav)?;

    assert_eq!(report.sample_count, 10, "whole frames only");
    assert_eq!(
        samples(wav.get(44..).unwrap_or_default()),
        signal,
        "the samples"
    );
    Ok(())
}

#[test]
fn any_other_codec_is_a_typed_error_naming_it() -> TestResult {
    let cases = [
        (
            TestWem::encoded(TestCodec::Tag(0xAAC0), 1, 48_000, 0),
            "AAC",
        ),
        (
            TestWem::encoded(TestCodec::Tag(0x0166), 2, 48_000, 0),
            "XMA2",
        ),
    ];
    for (wem, name) in cases {
        let wem = Wem::parse(wem.bytes())?;
        let mut wav = Vec::new();

        let error = wem.write_wav(&mut wav).err();

        assert!(
            matches!(error, Some(AudioError::UnsupportedCodec(codec)) if codec == wem.codec()),
            "{name}: {error:?}"
        );
        assert!(
            error.is_some_and(|error| error.to_string().contains(name)),
            "the message names {name}"
        );
        assert!(wav.is_empty(), "{name}: nothing is written");
    }
    assert_eq!(
        Codec::Unknown(0x1234).to_string(),
        "unknown codec 0x1234",
        "unknown tags"
    );
    Ok(())
}
