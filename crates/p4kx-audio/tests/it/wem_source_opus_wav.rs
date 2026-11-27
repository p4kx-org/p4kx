// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use p4kx_audio::{Wem, WemOpusWriteError, WemReadError, WemSource};
use p4kx_testkit::{TestOpus, TestOpusFraming};

use super::wav::assert_output_ranges;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn seekable_opus_wav_matches_all_framings_and_reads_only_the_planned_packet_prefix() -> TestResult {
    for (framing, channels) in [
        (TestOpusFraming::Wwise, 6),
        (TestOpusFraming::Switch, 2),
        (TestOpusFraming::Ogg, 6),
    ] {
        let bytes = fixture(framing, channels)?;
        check_replay(&bytes)?;
    }
    Ok(())
}

fn fixture(framing: TestOpusFraming, channels: u16) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(TestOpus::new(framing, channels, 9_600)
        .declared_sample_count(1_500)
        .output_gain(512)
        .wem()?
        .smpl_loop(0, 1, 2)
        .tail_chunk(*b"data", [0xa3, 0xd0])
        .bytes())
}

fn check_replay(bytes: &[u8]) -> TestResult {
    let original = Wem::parse(bytes)?;
    let mut expected = Vec::new();
    let expected_report = original.write_wav(&mut expected)?;
    let trace = Trace::new(bytes.len() as u64);
    let mut source = WemSource::open(trace.reader(bytes), bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    let plan = inspected.opus_wav()?;
    let ranges = plan.ranges().clone();
    assert_eq!(
        plan.report(),
        expected_report,
        "original PCM layout, trim and loop"
    );
    assert_eq!(plan.report().sample_count, 1_500, "authored frame trim");
    let data = original.metadata().data_chunk();
    assert!(
        ranges.emitted_data.start >= data.body_offset
            && ranges.emitted_data.end <= ranges.checked_data.end
            && ranges.checked_data.end <= data.body_end_exclusive(),
        "the emitted packet prefix belongs to the validated first DATA extent"
    );
    trace.last_byte.set(0);
    trace.readable_end.set(ranges.emitted_data.end);
    trace
        .table_end
        .set(ranges.seek_table.as_ref().map(|table| table.end));
    let mut actual = Vec::new();
    let evidence = plan.write_with_evidence(&mut actual)?;
    assert_eq!(evidence.report, expected_report, "successful write report");
    assert_eq!(evidence.ranges, ranges, "same validated source ranges");
    assert_eq!(
        evidence.replayed_seek_table,
        ranges
            .seek_table
            .as_ref()
            .map(|table| table.start..trace.last_table_byte.get()),
        "the successful conversion reports only the seek words read during replay"
    );
    assert_output_ranges(&actual, &evidence.output, evidence.report)?;
    assert_eq!(
        actual, expected,
        "streamed WAV matches the standalone decoded bytes"
    );
    assert_eq!(
        trace.last_byte.get(),
        ranges.emitted_data.end,
        "exact physical replay extent"
    );
    Ok(())
}

#[test]
fn seekable_opus_wav_reports_read_and_write_failures_independently() -> TestResult {
    let bytes = fixture(TestOpusFraming::Wwise, 1)?;
    let trace = Trace::new(bytes.len() as u64);
    let mut source = WemSource::open(trace.reader(&bytes), bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    let prepared = inspected.opus_wav()?;
    let end = prepared.ranges().emitted_data.end;
    trace.readable_end.set(end - 1);
    assert!(
        matches!(prepared.write(&mut io::sink()), Err(WemOpusWriteError::Input(WemReadError::Io(cause)))
            if cause.kind() == io::ErrorKind::PermissionDenied),
        "a failed packet read keeps its physical error"
    );
    trace.readable_end.set(bytes.len() as u64);
    let prepared = inspected.opus_wav()?;
    assert!(
        matches!(prepared.write(&mut ClosedSink), Err(WemOpusWriteError::Output(cause))
            if cause.kind() == io::ErrorKind::BrokenPipe),
        "an output failure remains an output error"
    );
    Ok(())
}

struct Trace {
    last_byte: Cell<u64>,
    readable_end: Cell<u64>,
    table_end: Cell<Option<u64>>,
    last_table_byte: Cell<u64>,
}

impl Trace {
    const fn new(readable_end: u64) -> Self {
        Self {
            last_byte: Cell::new(0),
            readable_end: Cell::new(readable_end),
            table_end: Cell::new(None),
            last_table_byte: Cell::new(0),
        }
    }

    const fn reader<'a>(&'a self, bytes: &'a [u8]) -> BoundedRead<'a> {
        BoundedRead {
            cursor: Cursor::new(bytes),
            trace: self,
        }
    }
}

struct BoundedRead<'a> {
    cursor: Cursor<&'a [u8]>,
    trace: &'a Trace,
}

impl Read for BoundedRead<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.len() > 1 << 16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unbounded input read",
            ));
        }
        let end = self.cursor.position() + output.len() as u64;
        if end > self.trace.readable_end.get() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "outside selected extent",
            ));
        }
        let read = self.cursor.read(output)?;
        if self
            .trace
            .table_end
            .get()
            .is_some_and(|table_end| end <= table_end)
        {
            self.trace
                .last_table_byte
                .set(self.trace.last_table_byte.get().max(self.cursor.position()));
        }
        self.trace
            .last_byte
            .set(self.trace.last_byte.get().max(self.cursor.position()));
        Ok(read)
    }
}

impl Seek for BoundedRead<'_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(to)
    }
}

struct ClosedSink;

impl Write for ClosedSink {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "closed test sink",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
