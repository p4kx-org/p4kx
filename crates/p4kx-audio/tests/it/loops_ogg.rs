// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loop points of looping Vorbis and Opus WEMs in the Ogg and WAV they become.

use crate::pages;
use std::io::Cursor;

use lewton::inside_ogg::OggStreamReader;
use p4kx_audio::{Loop, Wem};
use p4kx_testkit::{TestOpus, TestOpusFraming, TestVorbis};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const OPUS_FRAMES: u32 = 9_600;

/// Returns the vendor and the comments of a Vorbis comment or `OpusTags` packet after its magic.
fn comments(packet: &[u8]) -> Option<(String, Vec<String>)> {
    let u32_at = |offset: usize| {
        Some(u32::from_le_bytes(*packet.get(offset..)?.first_chunk::<4>()?) as usize)
    };
    let vendor_len = u32_at(0)?;
    let vendor = String::from_utf8(packet.get(4..4 + vendor_len)?.to_vec()).ok()?;
    let mut offset = 4 + vendor_len;
    let count = u32_at(offset)?;
    offset += 4;
    let mut list = Vec::new();
    for _ in 0..count {
        let len = u32_at(offset)?;
        list.push(String::from_utf8(packet.get(offset + 4..offset + 4 + len)?.to_vec()).ok()?);
        offset += 4 + len;
    }
    Some((vendor, list))
}

fn opus_tags(ogg: &[u8]) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let pages = pages::pages(ogg)?;
    assert!(
        pages.first().is_some_and(pages::Page::first)
            && pages.last().is_some_and(pages::Page::last)
            && pages.get(1).is_some_and(|tags| !tags.continued()),
        "BOS, EOS and the tags on a fresh page"
    );
    let packets = pages::packets(&pages);
    let tags = packets
        .get(1)
        .map_or_default(|(packet, _)| packet.as_slice());
    assert!(
        tags.starts_with(b"OpusTags"),
        "the second packet is OpusTags"
    );
    Ok(comments(tags.get(8..).unwrap_or_default()).ok_or("malformed OpusTags")?)
}

#[test]
fn a_looping_vorbis_wem_becomes_an_ogg_with_loopstart_and_looplength() -> TestResult {
    let bytes = TestVorbis::new(2, 48_000, &[false; 40])
        .sample_count(4_900)
        .wem()
        .smpl_loop(0, 100, 4_099)
        .bytes();
    let wem = Wem::parse(bytes.as_slice())?;
    let mut ogg = Vec::new();

    let report = wem.write_ogg(&mut ogg)?;

    let reader = OggStreamReader::new(Cursor::new(&ogg))?;
    assert_eq!(
        reader.comment_hdr.comment_list,
        [
            ("LOOPSTART".to_owned(), "100".to_owned()),
            ("LOOPLENGTH".to_owned(), "4000".to_owned())
        ],
        "a standard decoder reads the loop comments"
    );
    assert_eq!(report.sample_count, 4_900, "the sound is written once");
    assert_eq!(
        report.loop_points,
        Some(Loop {
            start: 100,
            end: 4_100
        }),
        "the report names the loop"
    );
    Ok(())
}

#[test]
fn a_vorbis_wem_without_a_loop_has_no_comments() -> TestResult {
    let bytes = TestVorbis::new(1, 48_000, &[false; 8]).wem().bytes();
    let mut ogg = Vec::new();

    let report = Wem::parse(bytes.as_slice())?.write_ogg(&mut ogg)?;

    let reader = OggStreamReader::new(Cursor::new(&ogg))?;
    assert!(reader.comment_hdr.comment_list.is_empty(), "no comments");
    assert_eq!(report.loop_points, None, "no loop");
    Ok(())
}

#[test]
fn a_looping_vorbis_wem_becomes_a_wav_with_a_smpl_chunk() -> TestResult {
    let bytes = TestVorbis::new(2, 48_000, &[false; 40])
        .sample_count(4_900)
        .wem()
        .smpl_loop(0, 0, 4_899)
        .bytes();
    let mut wav = Vec::new();

    let report = Wem::parse(bytes.as_slice())?.write_wav(&mut wav)?;

    assert_eq!(
        wav.get(36..40),
        Some(&b"smpl"[..]),
        "smpl after the fmt chunk"
    );
    assert_eq!(wav.get(104..108), Some(&b"data"[..]), "then the samples");
    assert_eq!(
        report.loop_points,
        Some(Loop {
            start: 0,
            end: 4_900
        }),
        "the whole sound loops"
    );
    assert_eq!(report.size, wav.len() as u64, "size");
    Ok(())
}

#[test]
fn a_looping_wwise_opus_wem_gets_loop_tags() -> TestResult {
    let bytes = TestOpus::new(TestOpusFraming::Wwise, 2, OPUS_FRAMES)
        .wem()?
        .smpl_loop(0, 960, 8_639)
        .bytes();
    let wem = Wem::parse(bytes.as_slice())?;
    let mut ogg = Vec::new();

    let report = wem.write_ogg(&mut ogg)?;

    let (_, list) = opus_tags(&ogg)?;
    assert_eq!(list, ["LOOPSTART=960", "LOOPLENGTH=7680"], "loop tags");
    assert_eq!(
        report.loop_points,
        Some(Loop {
            start: 960,
            end: 8_640
        }),
        "the report names the loop"
    );
    Ok(())
}

#[test]
fn opus_loop_tags_count_frames_after_a_nonzero_pre_skip() -> TestResult {
    let sound = TestOpus::new(TestOpusFraming::Wwise, 2, OPUS_FRAMES);
    let pre_skip = sound.packets()?.pre_skip;
    let bytes = sound.wem()?.smpl_loop(0, 480, 4_799).bytes();
    let mut ogg = Vec::new();

    let report = Wem::parse(bytes.as_slice())?.write_ogg(&mut ogg)?;

    assert!(pre_skip > 0, "the encoder delays the signal");
    let packets = pages::packets(&pages::pages(&ogg)?);
    let head = packets.first().map(|(packet, _)| packet.as_slice());
    assert_eq!(
        head.and_then(|head| head.get(10..12)),
        Some(&pre_skip.to_le_bytes()[..]),
        "OpusHead holds the pre-skip"
    );
    let (_, list) = opus_tags(&ogg)?;
    assert_eq!(
        list,
        ["LOOPSTART=480", "LOOPLENGTH=4320"],
        "the pre-skip is not added"
    );
    assert_eq!(
        report.loop_points,
        Some(Loop {
            start: 480,
            end: 4_800
        }),
        "the report names the loop in frames after the pre-skip"
    );
    Ok(())
}

#[test]
fn ogg_opus_in_a_wem_keeps_its_own_tags_and_gains_the_loop_tags() -> TestResult {
    let sound = TestOpus::new(TestOpusFraming::Ogg, 1, OPUS_FRAMES);
    let plain = sound.wem()?.bytes();
    let looping = sound.wem()?.smpl_loop(0, 0, 4_799).bytes();
    let mut source = Vec::new();
    Wem::parse(plain.as_slice())?.write_ogg(&mut source)?;
    let mut ogg = Vec::new();

    Wem::parse(looping.as_slice())?.write_ogg(&mut ogg)?;

    let (vendor, mut list) = opus_tags(&source)?;
    list.extend(["LOOPSTART=0".to_owned(), "LOOPLENGTH=4800".to_owned()]);
    assert_eq!(
        opus_tags(&ogg)?,
        (vendor, list),
        "own tags first, then the loop"
    );
    Ok(())
}

#[test]
fn a_looping_opus_wem_becomes_a_wav_with_a_smpl_chunk() -> TestResult {
    let bytes = TestOpus::new(TestOpusFraming::Wwise, 2, OPUS_FRAMES)
        .wem()?
        .smpl_loop(0, 960, 8_639)
        .bytes();
    let mut wav = Vec::new();

    let report = Wem::parse(bytes.as_slice())?.write_wav(&mut wav)?;

    assert_eq!(
        wav.get(36..40),
        Some(&b"smpl"[..]),
        "smpl after the fmt chunk"
    );
    assert_eq!(wav.get(104..108), Some(&b"data"[..]), "then the samples");
    assert_eq!(
        report.loop_points,
        Some(Loop {
            start: 960,
            end: 8_640
        }),
        "the loop in frames after the pre-skip"
    );
    assert_eq!(
        report.sample_count,
        u64::from(OPUS_FRAMES),
        "the sound is written once"
    );
    assert_eq!(report.size, wav.len() as u64, "size");
    Ok(())
}
