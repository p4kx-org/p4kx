// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;

use p4kx_audio::{AudioError, ByteOrder, Wem, WemReadError, WemSource, WemVorbisWriteError};
use p4kx_testkit::{TestCodebook, TestVorbis};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn seekable_vorbis_ogg_matches_the_slice_codec_and_exact_source_extents() -> TestResult {
    let default = TestVorbis::new(2, 48_000, &[false; 40]).sample_count(4_900);
    let separate = TestVorbis::new(1, 22_050, &[false, true, true, false])
        .codebook(TestCodebook::Library(97))
        .native_packets()
        .big_endian()
        .vorb_chunk();
    let inline = TestVorbis::new(2, 44_100, &[false; 5]).codebook(TestCodebook::Inline);
    for (fixture, separate) in [(default, false), (separate, true), (inline, false)] {
        let bytes = fixture
            .wem()
            .smpl_loop(0, 1, 2)
            .tail_chunk(*b"data", [0xde, 0xad])
            .bytes();
        assert_seek_parity(&bytes, separate)?;
    }
    Ok(())
}

#[test]
fn seekable_vorbis_ogg_does_not_read_the_whole_data_chunk() -> TestResult {
    let packet_count = 30_000;
    let fixture = TestVorbis::new(2, 48_000, &vec![false; packet_count]);
    let bytes = fixture.wem().bytes();
    let original = Wem::parse(&bytes)?;
    assert!(
        original.data().len() > usize::from(u16::MAX),
        "the data chunk exceeds the maximum individual Wwise packet size"
    );
    assert_seek_parity(&bytes, false)
}

fn assert_seek_parity(bytes: &[u8], separate: bool) -> TestResult {
    let original = Wem::parse(bytes)?;
    let mut expected = Vec::new();
    let expected_report = original.write_ogg(&mut expected)?;
    let metadata = original.metadata();
    let fmt = metadata.info_source().format_body_offset();
    let data = metadata.data_chunk();
    let size = original
        .data()
        .first_chunk::<2>()
        .ok_or("original setup prefix")?;
    let setup_len = match metadata.byte_order() {
        ByteOrder::Little => u16::from_le_bytes(*size),
        ByteOrder::Big => u16::from_be_bytes(*size),
    };
    let audio_start = data.body_offset + 2 + u64::from(setup_len);
    let reader = BoundedRead {
        inner: Cursor::new(bytes),
        unreadable: None,
        probe: None,
    };
    let mut source = WemSource::open(reader, bytes.len() as u64)?;
    let chunks = source.chunks().collect::<Result<Vec<_>, _>>()?;
    let vorb = chunks.iter().find(|chunk| chunk.id == *b"vorb");
    assert_eq!(vorb.is_some(), separate, "original header storage");
    let header = vorb.map_or(fmt + 24, |chunk| chunk.body_offset);
    let mut inspected = source.inspect()?;
    let planned = inspected.vorbis_ogg()?;
    let ranges = planned.ranges();
    assert_eq!(
        ranges.average_byte_rate,
        fmt + 8..fmt + 12,
        "original bitrate word"
    );
    assert_eq!(ranges.header, header..header + 42, "selected Vorbis header");
    assert_eq!(
        ranges.setup_packet,
        data.body_offset..audio_start,
        "original setup packet"
    );
    assert_eq!(
        ranges.audio_packets,
        audio_start..data.body_end_exclusive(),
        "audio packet bytes stop at the first data chunk"
    );
    assert_eq!(
        planned.sample_count(),
        expected_report.sample_count,
        "prepared frame count"
    );
    let mut actual = Vec::new();
    assert_eq!(
        planned.write(&mut actual)?,
        expected_report,
        "identical output report"
    );
    assert_eq!(actual, expected, "same complete Ogg pages as the slice API");
    Ok(())
}

#[test]
fn seekable_vorbis_ogg_preserves_a_physical_packet_read_failure() -> TestResult {
    let bytes = TestVorbis::new(1, 48_000, &[false, true, false])
        .wem()
        .bytes();
    let original = Wem::parse(&bytes)?;
    let setup = original
        .data()
        .first_chunk::<2>()
        .ok_or("original setup prefix")?;
    let data = original.metadata().data_chunk();
    let audio_start = data.body_offset + 2 + u64::from(u16::from_le_bytes(*setup));
    let reader = BoundedRead {
        inner: Cursor::new(bytes.as_slice()),
        unreadable: Some(audio_start + 2..audio_start + 3),
        probe: None,
    };
    let mut source = WemSource::open(reader, bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    assert!(
        matches!(inspected.vorbis_ogg(), Err(WemReadError::Io(error))
            if error.kind() == io::ErrorKind::PermissionDenied),
        "a denied packet payload is not a codebook rejection"
    );
    Ok(())
}

#[test]
fn seekable_vorbis_ogg_rejects_a_truncated_packet_before_writing() -> TestResult {
    let mut bytes = TestVorbis::new(1, 48_000, &[false, true, false])
        .wem()
        .bytes();
    let original = Wem::parse(&bytes)?;
    let setup = original
        .data()
        .first_chunk::<2>()
        .ok_or("original setup prefix")?;
    let data = original.metadata().data_chunk();
    let offset = usize::try_from(data.body_offset + 2 + u64::from(u16::from_le_bytes(*setup)))?;
    bytes
        .get_mut(offset..offset + 2)
        .ok_or("first audio packet prefix")?
        .copy_from_slice(&u16::MAX.to_le_bytes());
    let mut source = WemSource::open(Cursor::new(&bytes), bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    assert!(
        matches!(
            inspected.vorbis_ogg(),
            Err(WemReadError::Format(AudioError::InvalidVorbis(
                p4kx_audio::VorbisError::PacketOutOfBounds { .. }
            )))
        ),
        "a malformed packet prefix is a format failure during preparation"
    );
    Ok(())
}

#[test]
fn seekable_vorbis_wav_keeps_decode_and_lookahead_ranges_separate() -> TestResult {
    let exact_boundary = TestVorbis::new(2, 48_000, &[false; 40])
        .tone(7)
        .sample_count(38 * 128);
    let native = TestVorbis::new(1, 22_050, &[false, true, true, false])
        .codebook(TestCodebook::Library(97))
        .native_packets()
        .big_endian()
        .vorb_chunk();
    let zero_frames = TestVorbis::new(2, 44_100, &[false; 2])
        .codebook(TestCodebook::Inline)
        .sample_count(0);
    for (fixture, decoded_packets) in [(exact_boundary, 39), (native, 4), (zero_frames, 0)] {
        assert_wav_parity(&fixture, decoded_packets)?;
    }
    Ok(())
}

fn assert_wav_parity(fixture: &TestVorbis, decoded_packets: usize) -> TestResult {
    let bytes = fixture
        .wem()
        .smpl_loop(0, 1, 2)
        .tail_chunk(*b"data", [0xde, 0xad])
        .bytes();
    let original = Wem::parse(bytes.as_slice())?;
    let mut expected = Vec::new();
    let report = original.write_wav(&mut expected)?;
    let data = original.metadata().data_chunk();
    let audio_start = data.body_offset + 2 + fixture.setup().len() as u64;
    let decoded_end = audio_start
        + fixture
            .packets()
            .iter()
            .take(decoded_packets)
            .map(|packet| 2 + packet.len() as u64)
            .sum::<u64>();

    let probe = ReadProbe::default();
    let reader = BoundedRead {
        inner: Cursor::new(bytes.as_slice()),
        unreadable: None,
        probe: Some(&probe),
    };
    let mut source = WemSource::open(reader, bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    let planned = inspected.vorbis_wav()?;
    let checked_end = data.body_end_exclusive();
    assert_eq!(planned.report(), report, "standalone WAV report");
    assert_eq!(
        planned.ranges().audio_packets,
        audio_start..checked_end,
        "all packet framing is checked"
    );
    assert_eq!(
        planned.decoded_audio_packets(),
        &(audio_start..decoded_end),
        "only packets supplied to the decoder"
    );
    probe.furthest.set(0);
    let mut actual = Vec::new();
    planned.write(&mut actual)?;
    assert_eq!(actual, expected, "standalone WAV bytes");
    let expected_read_end = if decoded_packets == 0 { 0 } else { checked_end };
    assert_eq!(probe.furthest.get(), expected_read_end, "bounded lookahead");
    Ok(())
}

#[test]
fn seekable_vorbis_wav_preserves_replay_read_and_sink_errors() -> TestResult {
    let bytes = TestVorbis::new(1, 48_000, &[false, true, false])
        .wem()
        .bytes();
    let probe = ReadProbe::default();
    let reader = BoundedRead {
        inner: Cursor::new(bytes.as_slice()),
        unreadable: None,
        probe: Some(&probe),
    };
    let mut source = WemSource::open(reader, bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    let planned = inspected.vorbis_wav()?;
    probe
        .deny_from
        .set(Some(planned.ranges().audio_packets.start));
    assert!(
        matches!(planned.write(&mut Vec::new()), Err(WemVorbisWriteError::Input(error))
            if error.kind() == io::ErrorKind::PermissionDenied),
        "physical packet replay failures retain the source error"
    );
    probe.deny_from.set(None);
    let planned = inspected.vorbis_wav()?;
    let mut empty_output: &mut [u8] = &mut [];
    assert!(
        matches!(planned.write(&mut empty_output), Err(WemVorbisWriteError::Output(error))
            if error.kind() == io::ErrorKind::WriteZero),
        "sink failures retain the output error"
    );
    Ok(())
}

#[derive(Default)]
struct ReadProbe {
    furthest: Cell<u64>,
    deny_from: Cell<Option<u64>>,
}
struct BoundedRead<'a> {
    inner: Cursor<&'a [u8]>,
    unreadable: Option<Range<u64>>,
    probe: Option<&'a ReadProbe>,
}

impl Read for BoundedRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let position = self.inner.position();
        let end = position + out.len() as u64;
        if out.len() > usize::from(u16::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "read exceeds a Wwise packet",
            ));
        }
        if self
            .unreadable
            .as_ref()
            .is_some_and(|range| position < range.end && range.start < end)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "packet payload denied",
            ));
        }
        if let Some(probe) = self.probe {
            if probe.deny_from.get().is_some_and(|start| end > start) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "packet replay denied",
                ));
            }
            let count = self.inner.read(out)?;
            probe
                .furthest
                .set(probe.furthest.get().max(position + count as u64));
            return Ok(count);
        }
        self.inner.read(out)
    }
}

impl Seek for BoundedRead<'_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.inner.seek(to)
    }
}
