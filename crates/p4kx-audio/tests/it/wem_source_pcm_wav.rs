// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use p4kx_audio::{AudioError, ByteOrder, Codec, Loop, WavReport, Wem, WemPcmWriteError, WemSource};
use p4kx_testkit::{TestCodec, TestWem};

use super::TestResult;

const MAX_EXPECTED_READ: usize = 64 * 1024;
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const FRAMES: u64 = 20_002;

#[test]
fn seekable_pcm_wav_writes_exact_complete_frames_in_both_byte_orders() -> TestResult {
    let sample_count = usize::try_from(FRAMES)? * usize::from(CHANNELS);
    let expected: Vec<_> = [1_i16, -2, 3, -4]
        .into_iter()
        .cycle()
        .take(sample_count)
        .flat_map(i16::to_le_bytes)
        .collect();
    let mut raw_data = expected.clone();
    raw_data.push(0xdf);
    let fixture = TestWem::pcm(TestCodec::WwisePcm, CHANNELS, SAMPLE_RATE, &[])
        .smpl_loop(0, 1, 2)
        .data(raw_data)
        .tail_chunk(*b"data", [0xca, 0xfe]);
    let inputs = [
        (fixture.bytes(), ByteOrder::Little),
        (fixture.big_endian().bytes(), ByteOrder::Big),
    ];
    for (bytes, byte_order) in inputs {
        check_samples(&bytes, byte_order, &expected)?;
    }
    Ok(())
}

fn check_samples(bytes: &[u8], byte_order: ByteOrder, expected: &[u8]) -> TestResult {
    let original = Wem::parse(bytes)?;
    assert_eq!(
        original.byte_order(),
        byte_order,
        "actual original byte order"
    );
    assert_eq!(
        original.data().len(),
        expected.len() + 1,
        "source has one incomplete trailing byte before its second data chunk"
    );
    let source_start = original.metadata().data_chunk().body_offset;
    let reader = MaxRead(Cursor::new(bytes));
    let mut source = WemSource::open(reader, u64::try_from(bytes.len())?)?;
    let mut inspected = source.inspect()?;
    assert_eq!(
        inspected.metadata().sample_count(),
        Some(FRAMES),
        "original complete frames"
    );
    let planned = inspected.pcm_wav()?;
    assert_eq!(
        planned.source_range(),
        source_start..source_start + u64::try_from(expected.len())?,
        "the exact first data sample range is selected before writing"
    );
    let report = planned.report();
    let mut actual = Vec::new();
    planned.write(&mut actual)?;
    check_output(report, &actual, expected, &original)
}

fn check_output(
    report: WavReport,
    actual: &[u8],
    expected: &[u8],
    original: &Wem<&[u8]>,
) -> TestResult {
    assert_eq!(
        report.size,
        u64::try_from(actual.len())?,
        "actual WAV output bytes"
    );
    assert_eq!(
        (report.channels, report.sample_rate, report.sample_count),
        (CHANNELS, SAMPLE_RATE, FRAMES),
        "actual WAV audio dimensions"
    );
    assert_eq!(
        report.loop_points,
        Some(Loop { start: 1, end: 3 }),
        "exclusive output loop"
    );
    assert_eq!(
        actual.get(..4),
        Some(b"RIFF".as_slice()),
        "output is little-endian RIFF"
    );
    assert_eq!(
        Wem::parse(actual)?.data(),
        expected,
        "known PCM samples are exact"
    );
    let mut legacy = Vec::new();
    assert_eq!(
        original.write_wav(&mut legacy)?,
        report,
        "legacy report agrees"
    );
    assert_eq!(
        actual, legacy,
        "the previous canonical header and bytes stay compatible"
    );
    Ok(())
}

#[test]
fn seekable_pcm_wav_keeps_codec_and_physical_io_failures_distinct() -> TestResult {
    let unsupported = TestWem::encoded(TestCodec::Vorbis, 1, SAMPLE_RATE, 2).bytes();
    let mut source = WemSource::open(Cursor::new(&unsupported), u64::try_from(unsupported.len())?)?;
    let mut inspected = source.inspect()?;
    assert!(
        matches!(
            inspected.pcm_wav(),
            Err(AudioError::UnsupportedCodec(Codec::Vorbis))
        ),
        "preparation refuses a non-PCM codec"
    );

    let mut pcm = TestWem::pcm(TestCodec::Pcm, 1, SAMPLE_RATE, &[1, 2]).bytes();
    let original_length = u64::try_from(pcm.len())?;
    let mut source = WemSource::open(Cursor::new(pcm.as_slice()), original_length)?;
    let mut inspected = source.inspect()?;
    assert!(
        matches!(inspected.pcm_wav()?.write(&mut RejectOutput),
            Err(WemPcmWriteError::Output(cause)) if cause.kind() == io::ErrorKind::BrokenPipe),
        "the original output fault remains typed"
    );
    assert!(pcm.pop().is_some(), "one actual input byte is removed");
    let mut source = WemSource::open(Cursor::new(pcm.as_slice()), original_length)?;
    let mut inspected = source.inspect()?;
    assert!(
        matches!(inspected.pcm_wav()?.write(&mut Vec::new()),
            Err(WemPcmWriteError::Input(cause)) if cause.kind() == io::ErrorKind::UnexpectedEof),
        "a physical input fault inside the admitted range remains typed"
    );
    Ok(())
}

struct MaxRead<R>(R);

impl<R: Read> Read for MaxRead<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.len() > MAX_EXPECTED_READ {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "single read exceeds the transfer buffer",
            ));
        }
        self.0.read(output)
    }
}

impl<R: Seek> Seek for MaxRead<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.0.seek(from)
    }
}

struct RejectOutput;

impl Write for RejectOutput {
    fn write(&mut self, _output: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "synthetic output refusal",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
