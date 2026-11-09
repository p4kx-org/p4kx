// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::reference_resolve::ReferenceObservations;
use crate::{InputLookup, MediaError, MediaWriter};

pub(crate) struct ObservedInputLookup<'a> {
    pub(crate) lookup: InputLookup<'a>,
    pub(crate) evidence: ReferenceObservations<MediaOriginInput>,
    pub(crate) inventory: OriginContributions,
}

pub(crate) fn directory(
    input: Option<&MediaOriginInput>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let Some(input) = input else {
        return Ok(OriginContributions::unknown(&writer.origins.owner));
    };
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new("archive-directory", &[], "matching-inventory"),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(capture.finish())
}
