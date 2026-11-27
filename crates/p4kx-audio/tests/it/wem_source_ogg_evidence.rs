// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Cursor};
use std::ops::Range;

use p4kx_audio::{Codebooks, OggOutputRanges, OggReport, OggWriter, Wem, WemSource};
use p4kx_testkit::{TestCodebook, TestOpus, TestOpusFraming, TestVorbis};

use super::pages::{Page, packets, pages};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn opus_evidence_separates_replayed_seek_words_from_the_checked_table() -> TestResult {
    let declared = 960;
    let fixture = TestOpus::new(TestOpusFraming::Wwise, 1, 9_600).declared_sample_count(declared);
    let originals = fixture.packets()?;
    let bytes = fixture.wem()?.bytes();
    let mut source = WemSource::open(Cursor::new(&bytes), bytes.len() as u64)?;
    let seek = source
        .chunks()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|chunk| chunk.id == *b"seek")
        .ok_or("original seek table")?;
    let mut output = Vec::new();
    let evidence = source
        .inspect()?
        .opus_ogg()?
        .write_with_evidence(&mut output)?;

    let packet_count = (u64::from(declared) + u64::from(originals.pre_skip)).div_ceil(960);
    let data_count = originals
        .packets
        .iter()
        .take(usize::try_from(packet_count)?)
        .map(|packet| packet.len() as u64)
        .sum::<u64>();
    let data_start = Wem::parse(&bytes)?.metadata().data_chunk().body_offset;
    assert_eq!(
        evidence.replayed_seek_table,
        Some(seek.body_offset..seek.body_offset + packet_count * 2),
        "only the replayed packet size words contributed"
    );
    assert_eq!(
        evidence.ranges.seek_table,
        Some(seek.body_offset..seek.body_end_exclusive()),
        "checked table includes unplayed audio"
    );
    assert_eq!(
        evidence.replayed_audio_packets,
        data_start..data_start + data_count,
        "actual replayed original packets"
    );
    assert!(
        evidence.embedded_header_pages.is_none(),
        "the headers came from framed Wwise metadata"
    );
    assert_output(&bytes, &output, evidence.report, &evidence.output, 1)
}

#[test]
fn switch_preskip_binds_the_checked_first_duration_header() -> TestResult {
    for (payload, preskip) in [(&[0xf8][..], 120_u16), (&[0xfb, 2][..], 240)] {
        let mut data = vec![0; 4];
        data.extend_from_slice(&u32::try_from(payload.len())?.to_be_bytes());
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(payload);
        let bytes = TestOpus::new(TestOpusFraming::Switch, 1, 9_600)
            .declared_sample_count(480)
            .wem()?
            .data(data)
            .bytes();
        let start = Wem::parse(&bytes)?.metadata().data_chunk().body_offset + 4;
        let mut source = WemSource::open(Cursor::new(&bytes), bytes.len() as u64)?;
        let mut output = Vec::new();
        let evidence = source
            .inspect()?
            .opus_ogg()?
            .write_with_evidence(&mut output)?;
        let exact = evidence
            .switch_pre_skip
            .as_ref()
            .ok_or("Switch pre-skip input")?;
        assert_eq!(
            exact.packet_size,
            start..start + 4,
            "first original size word"
        );
        assert_eq!(
            exact.duration_header,
            start + 8..start + 8 + payload.len() as u64,
            "TOC and count when needed"
        );
        let written = pages(&output)?;
        let first = written.first().ok_or("identification page")?;
        assert_eq!(
            first.body.get(10..12),
            Some(preskip.to_le_bytes().as_slice()),
            "the output uses the first packet duration"
        );
    }
    Ok(())
}

#[test]
fn embedded_opus_evidence_covers_shared_and_continued_pages() -> TestResult {
    let data = shared_header_ogg()?;
    let parsed = pages(&data)?;
    let [first, continued, last] = parsed.as_slice() else {
        return Err(
            "expected shared first page, continued second page and final audio page".into(),
        );
    };
    assert!(
        continued.continued(),
        "the original comment packet spans both pages"
    );
    assert!(
        last.last(),
        "the original part after the admitted packet is checked"
    );
    let bytes = TestOpus::new(TestOpusFraming::Ogg, 1, 9_600)
        .declared_sample_count(960)
        .wem()?
        .data(data)
        .bytes();
    let data = Wem::parse(&bytes)?.metadata().data_chunk();
    let start = data.body_offset;
    let first_end = start + page_size(first);
    let second_end = first_end + page_size(continued);

    let mut source = WemSource::open(Cursor::new(&bytes), bytes.len() as u64)?;
    let mut output = Vec::new();
    let evidence = source
        .inspect()?
        .opus_ogg()?
        .write_with_evidence(&mut output)?;
    let headers = evidence.embedded_header_pages.ok_or("embedded headers")?;
    assert_eq!(
        headers.identification,
        start..first_end,
        "first checked page"
    );
    assert_eq!(
        headers.comments,
        start..second_end,
        "both checked comment pages"
    );
    assert_eq!(
        evidence.replayed_audio_packets,
        first_end..second_end,
        "the first audio packet shares the second comment page"
    );
    assert_eq!(
        evidence.ranges.checked_data.end,
        data.body_end_exclusive(),
        "final input page"
    );
    assert!(
        evidence.replayed_seek_table.is_none(),
        "embedded Ogg has no seek table"
    );
    assert_output(&bytes, &output, evidence.report, &evidence.output, 1)
}

#[test]
fn vorbis_codebook_evidence_stops_after_the_original_lookahead_packet() -> TestResult {
    let default = TestVorbis::new(2, 48_000, &[false; 40]);
    let aotuv = TestVorbis::new(1, 22_050, &[false; 40]).codebook(TestCodebook::Library(97));
    assert_vorbis_codebook_prefix(&default, Codebooks::Default)?;
    assert_vorbis_codebook_prefix(&aotuv, Codebooks::AoTuV)
}

fn assert_vorbis_codebook_prefix(fixture: &TestVorbis, codebooks: Codebooks) -> TestResult {
    let wem = fixture.wem();
    let mut data = Wem::parse(wem.bytes())?.data().to_vec();
    let setup_size = u16::from_le_bytes(*data.first_chunk::<2>().ok_or("setup length")?);
    data.extend_from_slice(&[0; 4]);
    let bytes = wem.data(data).bytes();
    let data = Wem::parse(&bytes)?.metadata().data_chunk();
    let audio_start = data.body_offset + 2 + u64::from(setup_size);
    let checked_prefix = fixture
        .packets()
        .iter()
        .take(17)
        .map(|packet| 2 + packet.len() as u64)
        .sum::<u64>();

    let mut source = WemSource::open(Cursor::new(&bytes), bytes.len() as u64)?;
    let mut output = Vec::new();
    let evidence = source
        .inspect()?
        .vorbis_ogg()?
        .write_with_evidence(&mut output)?;
    assert_eq!(
        evidence.report.codebooks,
        Some(codebooks),
        "the returned codebook choice matches the fixture"
    );
    assert_eq!(
        evidence.codebook_audio_packets,
        audio_start..audio_start + checked_prefix,
        "sixteen validated audio packets and one fetched lookahead packet"
    );
    assert_eq!(
        evidence.replayed_audio_packets,
        audio_start..data.body_end_exclusive(),
        "the whole replay including trailing empty selectors"
    );
    assert_eq!(
        evidence.ranges.audio_packets, evidence.replayed_audio_packets,
        "framing checked"
    );
    assert_output(&bytes, &output, evidence.report, &evidence.output, 2)
}

fn shared_header_ogg() -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut writer = OggWriter::new(&mut bytes, 13);
    writer.write_packet(b"OpusHead\x01\x01\0\0\x80\xbb\0\0\0\0\0", 0)?;
    let mut tags = b"OpusTags\0\0\0\0\0\0\0\0".to_vec();
    tags.resize(1 << 16, 0);
    writer.write_packet(&tags, 0)?;
    writer.write_packet(&[0xf8], 960)?;
    writer.end_page()?;
    writer.write_packet(&[0xf8], 1_920)?;
    writer.finish()?;
    Ok(bytes)
}

const fn page_size(page: &Page) -> u64 {
    (27 + page.lacing.len() + page.body.len()) as u64
}

fn assert_output(
    original: &[u8],
    bytes: &[u8],
    report: OggReport,
    output: &OggOutputRanges,
    other_header_packets: usize,
) -> TestResult {
    let mut ordinary = Vec::new();
    assert_eq!(
        Wem::parse(original)?.write_ogg(&mut ordinary)?,
        report,
        "ordinary report"
    );
    assert_eq!(ordinary, bytes, "ordinary Ogg stream");
    assert_eq!(
        output.identification.start, 0,
        "identification starts at stream start"
    );
    assert_eq!(
        output.identification.end, output.comments_and_setup.start,
        "first boundary"
    );
    assert_eq!(
        output.comments_and_setup.end, output.audio.start,
        "second boundary"
    );
    assert_eq!(output.audio.end, bytes.len() as u64, "EOS ends the stream");
    let read = |extent: &Range<u64>| -> Result<Vec<Page>, Box<dyn std::error::Error>> {
        let start = usize::try_from(extent.start)?;
        let end = usize::try_from(extent.end)?;
        let slice = bytes.get(start..end).ok_or("page bounds")?;
        pages(slice).map_err(Into::into)
    };
    let ident = read(&output.identification)?;
    let headers = read(&output.comments_and_setup)?;
    let audio = read(&output.audio)?;
    assert_eq!(packets(&ident).len(), 1, "complete identification packet");
    assert_eq!(
        packets(&headers).len(),
        other_header_packets,
        "complete additional headers"
    );
    assert!(
        ident.first().is_some_and(Page::first),
        "the first range includes BOS"
    );
    assert!(
        audio.last().is_some_and(Page::last),
        "the audio range includes EOS"
    );
    Ok(())
}
