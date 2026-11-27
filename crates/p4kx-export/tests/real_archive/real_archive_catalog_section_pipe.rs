// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, BufReader, PipeWriter, Read, Write};

use p4kx_export::{CatalogError, CatalogSummary, ZSTD_WINDOW_LOG};
use serde::Serialize;

use crate::real_archive_catalog_pipe::{self, CountingReader};

#[path = "real_archive_catalog_section_profile.rs"]
mod profile;
#[path = "real_archive_catalog_section_scan.rs"]
mod scan;

pub(super) use profile::Report as SectionReport;

#[cfg(test)]
#[path = "real_archive_catalog_section_tests.rs"]
mod tests;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Census {
    pub(super) encoded_bytes: u64,
    #[serde(flatten)]
    decoded: profile::Report,
}

impl Census {
    pub(super) fn reconcile_sizes(&self, encoded: u64, decoded: u64) -> io::Result<()> {
        if self.encoded_bytes != encoded || self.decoded.decoded_bytes != decoded {
            return Err(profile::invalid(
                "producer and census encoded/decoded byte counts differ",
            ));
        }
        Ok(())
    }

    pub(super) fn reconcile(&self, summary: &CatalogSummary) -> io::Result<()> {
        let encoded = summary
            .compressed_size()
            .ok_or_else(|| profile::invalid("producer did not write a compressed catalog"))?;
        self.reconcile_sizes(encoded, summary.size())?;
        let inventory = summary.source_inventory();
        if self.decoded.physical_sources != inventory.source_count() as u64
            || self.decoded.source_uses != inventory.use_count() as u64
        {
            return Err(profile::invalid(
                "serialized and registered provenance inventory counts differ",
            ));
        }
        Ok(())
    }
}

type TestResult<T> = Result<T, Box<dyn Error>>;

pub(super) fn transfer<T>(
    produce: impl FnOnce(PipeWriter) -> Result<(T, PipeWriter), CatalogError>,
) -> TestResult<(T, Census)> {
    real_archive_catalog_pipe::transfer(decode, produce)
}

fn decode(input: impl Read) -> io::Result<Census> {
    decode_to(input, io::sink())
}

pub(super) fn decode_to(input: impl Read, mut output: impl Write) -> io::Result<Census> {
    let mut decoder = zstd::Decoder::new(CountingReader::new(input))?;
    decoder.window_log_max(ZSTD_WINDOW_LOG)?;
    let report = scan::read(BufReader::new(CopyingReader {
        decoded: &mut decoder,
        output: &mut output,
    }))?;
    output.flush()?;
    Ok(Census {
        encoded_bytes: decoder.finish().into_inner().byte_count(),
        decoded: report,
    })
}

pub(super) struct SectionReader<R> {
    input: R,
    scanner: scan::Scanner,
}

impl<R> SectionReader<R> {
    pub(super) fn new(input: R) -> Self {
        Self {
            input,
            scanner: scan::Scanner::default(),
        }
    }

    pub(super) fn finish(self) -> io::Result<SectionReport> {
        self.scanner.finish()
    }
}

impl<R: Read> Read for SectionReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.input.read(bytes)?;
        let decoded = bytes
            .get(..count)
            .ok_or_else(|| profile::invalid("decoded read exceeded its buffer"))?;
        self.scanner.push(decoded)?;
        Ok(count)
    }
}

struct CopyingReader<R, W> {
    decoded: R,
    output: W,
}

impl<R: Read, W: Write> Read for CopyingReader<R, W> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.decoded.read(buffer)?;
        let bytes = buffer
            .get(..count)
            .ok_or_else(|| profile::invalid("decoded read exceeded its buffer"))?;
        self.output.write_all(bytes)?;
        Ok(count)
    }
}
