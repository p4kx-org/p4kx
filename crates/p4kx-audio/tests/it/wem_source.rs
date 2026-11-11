// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

use p4kx_audio::{
    AudioError, ByteOrder, Codec, Wem, WemReadError, WemSampleCountSource, WemSource,
};
use p4kx_testkit::{TestCodec, TestWem};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn wem_source_reads_metadata_and_directory_without_reading_a_large_data_body() -> TestResult {
    let data_len = (512_u32 << 20) + 4;
    let source_len = 44 + u64::from(data_len);
    let mut header = TestWem::pcm(TestCodec::Pcm, 2, 48_000, &[]).bytes();
    write_size(&mut header, 4, u32::try_from(source_len - 8)?)?;
    write_size(&mut header, 40, data_len)?;
    let mut reader = WemSource::open(HeaderOnly(Cursor::new(header)), source_len)?;
    let summary = reader.read_metadata()?;
    assert_eq!(
        summary.sample_count(),
        Some(u64::from(data_len / 4)),
        "the frame count uses only the original DATA length and stereo block alignment"
    );
    assert_eq!(reader.riff_end(), source_len, "large original RIFF extent");
    let chunks = reader.chunks().collect::<Result<Vec<_>, _>>()?;
    let [format, data] = chunks.as_slice() else {
        return Err("one format and one original DATA chunk".into());
    };
    assert_eq!(
        (format.id, format.ordinal, format.body_offset),
        (*b"fmt ", 0, 20),
        "format extent"
    );
    assert_eq!(
        (data.id, data.ordinal, data.body_offset),
        (*b"data", 1, 44),
        "first DATA location"
    );
    assert_eq!(
        data.body_len,
        u64::from(data_len),
        "full original DATA length"
    );
    assert_eq!(
        data.body_end_exclusive(),
        source_len,
        "half-open body extent"
    );
    Ok(())
}

#[test]
fn wem_source_and_slice_use_same_selected_rifx_summary_and_keep_padded_tail() -> TestResult {
    let selected_fmt = [0, 1, 0, 2, 0, 0, 0xBB, 0x80, 0, 2, 0xEE, 0, 0, 4, 0, 16];
    let bytes = TestWem::pcm(TestCodec::Pcm, 1, 22_050, &[0, 1, 2, 3])
        .big_endian()
        .chunk(*b"fmt ", selected_fmt)
        .tail_chunk(*b"fmt ", [0; 16])
        .tail_chunk(*b"LIST", [7; 3])
        .tail_chunk(*b"data", [0; 24])
        .bytes();

    let sound = Wem::parse(bytes.as_slice())?;
    let mut reader = WemSource::open(Cursor::new(bytes.as_slice()), bytes.len() as u64)?;
    let summary = reader.read_metadata()?;
    assert_eq!(
        summary.byte_order(),
        ByteOrder::Big,
        "original RIFX byte order"
    );
    assert_eq!(summary.channels(), 2, "last pre-DATA fmt wins");
    assert_eq!(summary.sample_rate(), 48_000, "selected fmt rate");
    assert_eq!(summary.sample_count(), Some(2), "first DATA wins");
    assert_eq!(
        summary.info_source().format_body_offset(),
        44,
        "actual selected fmt body"
    );
    assert_eq!(
        summary.data_chunk().body_offset,
        68,
        "actual first DATA body"
    );
    assert_eq!(
        summary.duration(),
        sound.duration(),
        "both summary APIs use the same duration"
    );
    let checked = reader.chunks().collect::<Result<Vec<_>, _>>()?;
    let ids: Vec<_> = checked.iter().map(|chunk| chunk.id).collect();
    assert_eq!(
        ids,
        [*b"fmt ", *b"fmt ", *b"data", *b"fmt ", *b"LIST", *b"data"],
        "the checked physical directory retains repeats and post-data chunks"
    );
    assert_eq!(
        ids,
        sound.chunks().map(|chunk| chunk.id).collect::<Vec<_>>(),
        "legacy valid iteration order"
    );
    let list = checked
        .iter()
        .find(|chunk| chunk.id == *b"LIST")
        .ok_or("LIST occurrence")?;
    assert_eq!(list.body_len, 3, "odd body excludes padding");
    assert_eq!(
        list.next_offset,
        list.body_end_exclusive() + 1,
        "next offset includes observed padding"
    );
    Ok(())
}

#[test]
fn wem_source_keeps_switch_declared_rate_and_does_not_decode_seek_or_packets_for_info() -> TestResult
{
    let extension = [0, 0, 0, 0, 0, 0, 0, 0, 120, 0, 0, 0];
    let bytes = TestWem::encoded(TestCodec::Opus(0x3039), 2, 24_000, 120)
        .fmt_extension(extension)
        .chunk(*b"seek", [255; 8])
        .data([255; 4])
        .bytes();
    let legacy = Wem::parse(bytes.as_slice())?;
    let mut stream = WemSource::open(Cursor::new(bytes.as_slice()), bytes.len() as u64)?;
    let summary = stream.read_metadata()?;
    assert_eq!(summary.codec(), Codec::Opus, "actual Switch format tag");
    assert_eq!(summary.declared_sample_rate(), 24_000, "raw rate word");
    assert_eq!(
        summary.sample_rate(),
        48_000,
        "normalised Opus playback rate"
    );
    assert_eq!(
        summary.sample_count(),
        None,
        "wrong raw Switch rate rejects embedded frames"
    );
    assert_eq!(
        summary.sample_count(),
        legacy.sample_count(),
        "legacy summary parity"
    );
    assert_eq!(
        summary.duration(),
        None,
        "no inferred duration from rejected count"
    );
    assert!(
        matches!(
            summary.info_source().sample_count(),
            WemSampleCountSource::Opus {
                rate_checked: true,
                accepted: false
            }
        ),
        "the selected receipt records the reached Switch guard"
    );
    Ok(())
}

#[test]
fn wem_source_directory_reports_a_short_header_after_valid_data() -> TestResult {
    let mut bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[0, 1]).bytes();
    let next_header = bytes.len() as u64;
    bytes.extend_from_slice(b"bad");
    let declared_end = bytes.len() as u64;
    write_size(&mut bytes, 4, u32::try_from(declared_end - 8)?)?;
    let legacy = Wem::parse(bytes.as_slice())?;
    assert_eq!(
        legacy.chunks().count(),
        2,
        "legacy convenience iterator retains its prefix"
    );
    let mut source = WemSource::open(Cursor::new(bytes), declared_end)?;
    assert_eq!(
        source.read_metadata()?.sample_count(),
        Some(2),
        "tail is independent of the selected summary"
    );
    let mut chunks = source.chunks();
    assert_eq!(
        chunks
            .by_ref()
            .take(2)
            .collect::<Result<Vec<_>, _>>()?
            .len(),
        2,
        "checked prefix is retained"
    );
    assert!(
        matches!(
            chunks.next(),
            Some(Err(WemReadError::Format(AudioError::Truncated { len, needed })))
                if len == declared_end && needed == next_header + 8
        ),
        "the checked directory identifies the incomplete terminal header"
    );
    assert!(
        chunks.next().is_none(),
        "directory stops after its terminal error"
    );
    Ok(())
}

#[test]
fn wem_source_directory_rejects_a_body_outside_riff_even_when_raw_bytes_exist() -> TestResult {
    let mut bytes = TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[0, 1])
        .tail_chunk(*b"LIST", [7; 4])
        .bytes();
    let declared_end = bytes.len() as u64;
    write_size(&mut bytes, usize::try_from(declared_end - 8)?, 8)?;
    bytes.extend_from_slice(&[0; 8]);
    let raw_len = bytes.len() as u64;
    let legacy = Wem::parse(bytes.as_slice())?;
    assert_eq!(
        legacy.chunks().count(),
        2,
        "legacy iteration still omits the offending chunk"
    );
    let mut stream = WemSource::open(Cursor::new(bytes), raw_len)?;
    let mut chunks = stream.chunks();
    assert_eq!(
        chunks
            .by_ref()
            .take(2)
            .collect::<Result<Vec<_>, _>>()?
            .len(),
        2,
        "two admitted chunks"
    );
    assert!(
        matches!(
            chunks.next(),
            Some(Err(WemReadError::Format(AudioError::Truncated { len, needed })))
                if len == declared_end && needed == declared_end + 4
        ),
        "checked availability is the declared RIFF extent, not raw bytes outside it"
    );
    Ok(())
}

#[test]
fn wem_source_distinguishes_known_format_truncation_from_actual_reader_failure() {
    assert!(
        matches!(
            WemSource::open(Cursor::new(*b"RIFF"), 4),
            Err(WemReadError::Format(AudioError::Truncated {
                len: 4,
                needed: 12
            }))
        ),
        "known short declared input is a format error"
    );
    assert!(
        matches!(
            WemSource::open(Cursor::new(*b"RIFF"), 12),
            Err(WemReadError::Io(cause)) if cause.kind() == io::ErrorKind::UnexpectedEof
        ),
        "premature EOF inside the supplied verified length remains an input failure"
    );
}

fn write_size(bytes: &mut [u8], offset: usize, size: u32) -> TestResult {
    let field = bytes
        .get_mut(offset..offset + 4)
        .ok_or("four original size bytes")?;
    field.copy_from_slice(&size.to_le_bytes());
    Ok(())
}

struct HeaderOnly(Cursor<Vec<u8>>);

impl Read for HeaderOnly {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.0.position().saturating_add(output.len() as u64) > self.0.get_ref().len() as u64 {
            return Err(io::Error::other(
                "metadata reader tried to read the virtual DATA body",
            ));
        }
        self.0.read(output)
    }
}

impl Seek for HeaderOnly {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}
