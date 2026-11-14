// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::query::{QueryByteUse, QuerySource};
use p4kx_extraction::{InputScope, SourceUseRole};

use super::origins::ModelOrigins;
use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter};

pub(super) fn path_evidence(
    originals: &ModelOrigins<'_>,
    record: u64,
    field: &str,
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
    for &source in sites {
        capture_query_source(&mut capture, originals, source, field, writer)?;
    }
    Ok(capture.finish())
}

fn capture_query_source(
    capture: &mut OriginCapture,
    originals: &ModelOrigins<'_>,
    source: QuerySource,
    field: &str,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let role = SourceUseRole::Selector;
    match source {
        QuerySource::Bytes { start, end, kind } => {
            let role = if matches!(kind, QueryByteUse::Value | QueryByteUse::Text) {
                SourceUseRole::Payload
            } else {
                role
            };
            capture.add(
                originals.input,
                InputScope::Payload,
                OriginSite::new("dataforgeDecodedBytes", &[start, end], field),
                role,
                writer,
            )?;
        }
        QuerySource::ArrayElement {
            header_start,
            index,
        } => capture.add(
            originals.input,
            InputScope::Payload,
            OriginSite::new(
                "dataforgeArrayElement",
                &[header_start, u64::from(index)],
                field,
            ),
            role,
            writer,
        )?,
        QuerySource::Instance {
            structure,
            instance,
        } => capture.add(
            originals.input,
            InputScope::Payload,
            OriginSite::new(
                "dataforgeInstance",
                &[u64::from(structure), u64::from(instance)],
                field,
            ),
            role,
            writer,
        )?,
        QuerySource::Structure { index } => capture.add(
            originals.input,
            InputScope::Payload,
            OriginSite::new("dataforgeStructDefinition", &[u64::from(index)], field),
            role,
            writer,
        )?,
        _ => {
            return Err(MediaError::Contract(
                "unexpected original vehicle-grid query route".into(),
            ));
        }
    }
    Ok(())
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
