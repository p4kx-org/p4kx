// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufReader, PipeWriter};

use p4kx_export::CatalogError;
use serde_json::Value;

use super::{TestResult, TextPaths};
use crate::real_archive_catalog_pipe::{self, CountingReader};

#[derive(Debug)]
pub(super) struct Capture {
    pub(super) document: Value,
    pub(super) bytes: u64,
}

pub(super) fn transfer<T>(
    paths: &TextPaths,
    produce: impl FnOnce(PipeWriter) -> Result<(T, PipeWriter), CatalogError>,
) -> TestResult<(T, Capture)> {
    real_archive_catalog_pipe::transfer(
        |reader| {
            let mut input = BufReader::new(CountingReader::new(reader));
            let document = paths.read(&mut input)?;
            Ok::<_, serde_json::Error>(Capture {
                document,
                bytes: input.into_inner().byte_count(),
            })
        },
        produce,
    )
}
