// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;

use p4kx_audio::{
    AudioError, OggWriter, OpusError, Wem, WemOpusRanges, WemReadError, WemSource, WemSourceChunk,
};
use p4kx_testkit::{TestOpus, TestOpusFraming};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FRAMES: u32 = 9_600;
const SOURCE_PACKET_BOUND: usize = 1 << 16;

#[test]
fn seekable_opus_ogg_matches_all_three_slice_framings_and_selected_ranges() -> TestResult {
    for (framing, channels) in [
        (TestOpusFraming::Wwise, 6),
        (TestOpusFraming::Switch, 2),
        (TestOpusFraming::Ogg, 6),
    ] {
        let bytes = TestOpus::new(framing, channels, FRAMES)
            .declared_sample_count(1_500)
            .output_gain(512)
            .wem()?
            .smpl_loop(0, 1, 2)
            .tail_chunk(*b"data", [0xca, 0xfe])
            .bytes();
        check_framing(&bytes, framing)?;
    }
    Ok(())
}

fn check_framing(bytes: &[u8], framing: TestOpusFraming) -> TestResult {
    let original = Wem::parse(bytes)?;
    let data = original.metadata().data_chunk();
    let forbidden = match framing {
        TestOpusFraming::Switch => Some(data.body_offset..data.body_offset + 4),
        _ => None,
    };
    let ranges = check_parity(&original, bytes, forbidden)?;
    assert_eq!(
        ranges.checked_data.end,
        data.body_end_exclusive(),
        "checked data stays in the original first DATA chunk"
    );
    let fmt_start = original.metadata().info_source().format_body_offset();
    let fmt_len = match framing {
        TestOpusFraming::Wwise => 36,
        TestOpusFraming::Switch => 40,
        TestOpusFraming::Ogg => 28,
    };
    assert_eq!(
        ranges.format,
        fmt_start..fmt_start + fmt_len,
        "selected format prefix"
    );
    match framing {
        TestOpusFraming::Wwise => {
            let seek = selected_seek(bytes)?;
            assert_eq!(
                ranges.seek_table,
                Some(seek.body_offset..seek.body_end_exclusive()),
                "every admitted table size is used"
            );
        }
        TestOpusFraming::Switch => {
            assert_eq!(
                ranges.checked_data.start,
                data.body_offset + 4,
                "seek prefix excluded"
            );
            assert!(ranges.seek_table.is_none(), "no external seek table");
        }
        TestOpusFraming::Ogg => {
            let pages = ranges
                .embedded_headers
                .ok_or("original embedded header pages")?;
            assert_eq!(pages.start, data.body_offset, "original Ogg starts at DATA");
            assert!(
                pages.end <= ranges.checked_data.end,
                "header pages were checked"
            );
        }
    }
    if !matches!(framing, TestOpusFraming::Ogg) {
        assert!(
            ranges.emitted_data.end < ranges.checked_data.end,
            "authored trim writes only a prefix of the validated packets"
        );
    }
    Ok(())
}

fn selected_seek(bytes: &[u8]) -> Result<WemSourceChunk, Box<dyn std::error::Error>> {
    let mut source = WemSource::open(Cursor::new(bytes), bytes.len() as u64)?;
    let mut selected = None;
    for chunk in source.chunks() {
        let chunk = chunk?;
        match &chunk.id {
            b"seek" => selected = Some(chunk),
            b"data" => break,
            _ => {}
        }
    }
    selected.ok_or_else(|| "original packet size table".into())
}

#[test]
fn seekable_opus_ogg_uses_only_the_admitted_seek_words_and_packet_data() -> TestResult {
    let sound = TestOpus::new(TestOpusFraming::Wwise, 1, FRAMES).declared_sample_count(960);
    let fixture = sound.wem()?;
    let original = fixture.bytes();
    let source = Wem::parse(original.as_slice())?;
    let mut table = source
        .chunk(*b"seek")
        .ok_or("source packet sizes")?
        .to_vec();
    let mut data = source.data().to_vec();
    let checked_table = table.len() as u64;
    let checked_data = data.len() as u64;
    table.extend_from_slice(&[0xff, 0xff]);
    data.extend_from_slice(&[0x12, 0x34]);
    let bytes = fixture.chunk(*b"seek", table).data(data).bytes();
    let source = Wem::parse(bytes.as_slice())?;
    let seek = selected_seek(&bytes)?;
    let table_end = seek.body_offset + checked_table;
    let ranges = check_parity(&source, &bytes, Some(table_end..seek.body_end_exclusive()))?;
    assert_eq!(
        ranges.seek_table,
        Some(seek.body_offset..table_end),
        "unused size words excluded"
    );
    let start = source.metadata().data_chunk().body_offset;
    assert_eq!(
        ranges.checked_data,
        start..start + checked_data,
        "unused DATA tail excluded"
    );
    Ok(())
}

fn check_parity(
    original: &Wem<&[u8]>,
    bytes: &[u8],
    forbidden: Option<Range<u64>>,
) -> Result<WemOpusRanges, Box<dyn std::error::Error>> {
    let mut legacy = Vec::new();
    let expected = original.write_ogg(&mut legacy)?;
    let input = BoundedRead {
        inner: Cursor::new(bytes),
        forbidden,
    };
    let mut source = WemSource::open(input, bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    let planned = inspected.opus_ogg()?;
    let ranges = planned.ranges().clone();
    assert_eq!(
        planned.sample_count(),
        expected.sample_count,
        "exact planned frame count"
    );
    let mut output = Vec::new();
    let report = planned.write(&mut output)?;
    assert_eq!(
        report, expected,
        "identical report, pre-skip trim and loop behavior"
    );
    assert_eq!(output, legacy, "identical deterministic Ogg pages");
    Ok(ranges)
}

#[test]
fn seekable_opus_ogg_reads_continued_pages_and_excludes_the_suffix_after_eos() -> TestResult {
    let mut data = continued_ogg()?;
    let checked_len = data.len() as u64;
    data.extend_from_slice(&[0x12, 0x34]);
    let bytes = TestOpus::new(TestOpusFraming::Ogg, 1, FRAMES)
        .declared_sample_count(960)
        .wem()?
        .data(data)
        .bytes();
    let original = Wem::parse(bytes.as_slice())?;
    let start = original.metadata().data_chunk().body_offset;
    let end = start + checked_len;
    assert!(
        checked_len > SOURCE_PACKET_BOUND as u64,
        "the original Ogg spans several bounded page reads"
    );
    let ranges = check_parity(&original, &bytes, Some(end..end + 2))?;
    assert_eq!(
        ranges.checked_data,
        start..end,
        "suffix after the admitted EOS is excluded"
    );
    assert_eq!(
        ranges.emitted_data, ranges.checked_data,
        "the continued packet is emitted whole"
    );
    Ok(())
}

fn continued_ogg() -> Result<Vec<u8>, io::Error> {
    let mut bytes = Vec::new();
    let mut writer = OggWriter::new(&mut bytes, 13);
    let head = b"OpusHead\x01\x01\0\0\x80\xbb\0\0\0\0\0";
    let tags = b"OpusTags\0\0\0\0\0\0\0\0";
    writer.write_packet(head, 0)?;
    writer.end_page()?;
    writer.write_packet(tags, 0)?;
    writer.end_page()?;
    let mut packet = vec![0; SOURCE_PACKET_BOUND];
    if let Some(toc) = packet.first_mut() {
        *toc = 0xF8;
    }
    writer.write_packet(&packet, 960)?;
    writer.finish()?;
    Ok(bytes)
}

#[test]
fn seekable_opus_ogg_preserves_the_typed_crc_and_physical_read_failures() -> TestResult {
    let mut bytes = TestOpus::new(TestOpusFraming::Ogg, 1, FRAMES)
        .wem()?
        .bytes();
    let original = Wem::parse(bytes.as_slice())?;
    let start = original.metadata().data_chunk().body_offset;
    let input = BoundedRead {
        inner: Cursor::new(bytes.as_slice()),
        forbidden: Some(start..start + 1),
    };
    let mut source = WemSource::open(input, bytes.len() as u64)?;
    assert!(
        matches!(source.inspect()?.opus_ogg(), Err(WemReadError::Io(cause))
            if cause.kind() == io::ErrorKind::PermissionDenied),
        "an unreadable source page stays an I/O error"
    );
    let crc = bytes
        .get_mut(usize::try_from(start + 22)?)
        .ok_or("original first page CRC")?;
    *crc ^= 1;
    let mut source = WemSource::open(Cursor::new(bytes.as_slice()), bytes.len() as u64)?;
    assert!(
        matches!(
            source.inspect()?.opus_ogg(),
            Err(WemReadError::Format(AudioError::InvalidOpus(
                OpusError::InvalidOgg("a page's CRC does not match")
            )))
        ),
        "a readable page with wrong CRC stays a format error"
    );
    Ok(())
}

struct BoundedRead<'a> {
    inner: Cursor<&'a [u8]>,
    forbidden: Option<Range<u64>>,
}

impl Read for BoundedRead<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let start = self.inner.position();
        let end = start + output.len() as u64;
        if output.len() > SOURCE_PACKET_BOUND {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unbounded packet read",
            ));
        }
        if self
            .forbidden
            .as_ref()
            .is_some_and(|range| start < range.end && range.start < end)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unselected input bytes",
            ));
        }
        self.inner.read(output)
    }
}

impl Seek for BoundedRead<'_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.inner.seek(to)
    }
}
