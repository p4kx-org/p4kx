// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, BufReader, PipeWriter, Read};

use p4kx_export::{CatalogError, CatalogSummary, ZSTD_WINDOW_LOG};
use serde::Serialize;

use crate::real_archive_catalog_pipe::{self, CountingReader};

#[path = "real_archive_catalog_section_profile.rs"]
mod profile;
#[path = "real_archive_catalog_section_scan.rs"]
mod scan;

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
    pub(super) fn reconcile(&self, summary: &CatalogSummary) -> io::Result<()> {
        if Some(self.encoded_bytes) != summary.compressed_size()
            || self.decoded.decoded_bytes != summary.size()
        {
            return Err(profile::invalid(
                "producer and census encoded/decoded byte counts differ",
            ));
        }
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
    let mut decoder = zstd::Decoder::new(CountingReader::new(input))?;
    decoder.window_log_max(ZSTD_WINDOW_LOG)?;
    let report = scan::read(BufReader::new(&mut decoder))?;
    Ok(Census {
        encoded_bytes: decoder.finish().into_inner().byte_count(),
        decoded: report,
    })
}
