// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::query::QuerySource;
use p4kx_extraction::{DataForge, InputScope, SourceUseRole};

use super::origins::ModelOrigins;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter};

pub(super) fn path_evidence(
    originals: &ModelOrigins<'_>,
    record: u64,
    field: &str,
    forge: &DataForge<'_>,
    sites: &[QuerySource],
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        originals.input,
        InputScope::Payload,
        OriginSite::new("dataforge-record", &[record], field),
        SourceUseRole::Selector,
        writer,
    )?;
    capture.add_query_inputs(originals.input, forge, sites, writer)?;
    Ok(capture.finish())
}

pub(super) fn record_field(
    originals: &ModelOrigins<'_>,
    record: u64,
    field: &str,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        originals.input,
        InputScope::Payload,
        OriginSite::new("dataforge-record", &[record], field),
        SourceUseRole::Payload,
        writer,
    )?;
    Ok(capture.finish())
}
