// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loop points of looping WEMs: reading them from the `smpl` chunk and keeping them in WAV and Ogg.

use p4kx_audio::{AudioError, Loop, Wem};
use p4kx_testkit::{TestCodec, TestWem, test_signal};

type TestResult = Result<(), AudioError>;

const FRAMES: u32 = 4_800;

fn pcm() -> TestWem {
    TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &test_signal(FRAMES, 2))
}

#[test]
fn the_loop_is_read_from_the_smpl_chunk_with_an_exclusive_end() -> TestResult {
    let wem = Wem::parse(pcm().smpl_loop(0, 1_000, 3_999).bytes())?;

    assert_eq!(
        wem.loop_points(),
        Some(Loop {
            start: 1_000,
            end: 4_000
        }),
        "start and the frame after the inclusive end"
    );
    assert_eq!(wem.loop_points().map(Loop::frames), Some(3_000), "frames");
    Ok(())
}

#[test]
fn loops_that_do_not_fit_the_sample_count_are_dropped() -> TestResult {
    for (start, end) in [(0, FRAMES), (4_000, 3_000), (u32::MAX, u32::MAX), (10, 9)] {
        let wem = Wem::parse(pcm().smpl_loop(0, start, end).bytes())?;
        assert_eq!(wem.loop_points(), None, "{start}..={end}");
        assert_eq!(
            wem.sample_count(),
            Some(u64::from(FRAMES)),
            "the sound stays readable"
        );
    }
    let wem = Wem::parse(pcm().smpl_loop(0, 0, FRAMES - 1).bytes())?;
    assert_eq!(
        wem.loop_points(),
        Some(Loop {
            start: 0,
            end: u64::from(FRAMES)
        }),
        "a loop over the whole sound"
    );
    Ok(())
}

#[test]
fn the_first_forward_loop_is_taken_in_either_byte_order() -> TestResult {
    let wem = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &test_signal(FRAMES, 1))
        .big_endian()
        .smpl_loop(1, 10, 20)
        .smpl_loop(0, 100, 199);

    assert_eq!(
        Wem::parse(wem.bytes())?.loop_points(),
        Some(Loop {
            start: 100,
            end: 200
        }),
        "the ping-pong loop is skipped"
    );
    Ok(())
}

#[test]
fn a_truncated_or_overcounted_smpl_chunk_gives_no_loop_and_no_panic() -> TestResult {
    let mut smpl = vec![0; 0x24];
    smpl.get_mut(0x1C..0x20)
        .unwrap_or_default()
        .copy_from_slice(&u32::MAX.to_le_bytes());
    smpl.extend_from_slice(&[0; 0x17]);
    for len in 0..smpl.len() {
        let body = smpl.get(..len).unwrap_or_default().to_vec();
        let wem = Wem::parse(pcm().chunk(*b"smpl", body).bytes())?;
        assert_eq!(wem.loop_points(), None, "{len} bytes");
    }
    Ok(())
}

#[test]
fn a_raw_end_of_zero_loops_to_the_end_of_the_sound() -> TestResult {
    for start in [0, 100] {
        let wem = Wem::parse(pcm().smpl_loop(0, start, 0).bytes())?;
        assert_eq!(
            wem.loop_points(),
            Some(Loop {
                start: u64::from(start),
                end: u64::from(FRAMES)
            }),
            "start {start}"
        );
    }
    let at_end = Wem::parse(pcm().smpl_loop(0, FRAMES, 0).bytes())?;
    assert_eq!(at_end.loop_points(), None, "an empty loop at the end");
    Ok(())
}

#[test]
fn a_sound_without_a_known_sample_count_has_no_loop() -> TestResult {
    let wem = TestWem::encoded(TestCodec::Tag(0xAAC0), 1, 48_000, 960).smpl_loop(0, 0, 99);

    assert_eq!(Wem::parse(wem.bytes())?.loop_points(), None, "no loop");
    Ok(())
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        *bytes.get(offset..)?.first_chunk::<4>()?,
    ))
}

/// Returns the chunks of a RIFF WAVE file after checking that its RIFF size covers them.
fn wav_chunks(wav: &[u8]) -> Vec<([u8; 4], &[u8])> {
    assert_eq!(wav.get(..4), Some(&b"RIFF"[..]), "RIFF");
    assert_eq!(
        u32_at(wav, 4).map(|size| size as usize + 8),
        Some(wav.len()),
        "the RIFF size covers the file"
    );
    let mut chunks = Vec::new();
    let mut position = 12;
    while let (Some(id), Some(len)) = (
        wav.get(position..).and_then(<[u8]>::first_chunk::<4>),
        u32_at(wav, position + 4),
    ) {
        let body = position + 8;
        let end = body + len as usize;
        chunks.push((*id, wav.get(body..end).unwrap_or_default()));
        position = end + len as usize % 2;
    }
    chunks
}

#[test]
fn the_wav_carries_the_loop_as_a_smpl_chunk_before_the_samples() -> TestResult {
    let wem = Wem::parse(pcm().smpl_loop(0, 1_000, 3_999).bytes())?;
    let mut wav = Vec::new();

    let report = wem.write_wav(&mut wav)?;

    let chunks = wav_chunks(&wav);
    let ids: Vec<[u8; 4]> = chunks.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [*b"fmt ", *b"smpl", *b"data"], "chunk order");
    let smpl = chunks.get(1).map_or_default(|(_, body)| *body);
    let fields: Vec<u32> = (0..15)
        .filter_map(|index| u32_at(smpl, index * 4))
        .collect();
    assert_eq!(
        fields,
        [0, 0, 20_833, 60, 0, 0, 0, 1, 0, 0, 0, 1_000, 3_999, 0, 0],
        "sample period in ns, unity note, one infinite forward loop with an inclusive end"
    );
    let data = chunks.get(2).map_or_default(|(_, body)| *body);
    let samples: Vec<i16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(i16::from_le_bytes)
        .collect();
    assert_eq!(samples, test_signal(FRAMES, 2), "the sound is written once");
    assert_eq!(report.size, wav.len() as u64, "size");
    assert_eq!(
        report.loop_points,
        Some(Loop {
            start: 1_000,
            end: 4_000
        }),
        "the report names the loop"
    );
    Ok(())
}

#[test]
fn a_wav_without_a_loop_has_no_smpl_chunk() -> TestResult {
    let mut wav = Vec::new();

    let report = Wem::parse(pcm().bytes())?.write_wav(&mut wav)?;

    let ids: Vec<[u8; 4]> = wav_chunks(&wav).iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [*b"fmt ", *b"data"], "chunks");
    assert_eq!(report.loop_points, None, "no loop");
    Ok(())
}
