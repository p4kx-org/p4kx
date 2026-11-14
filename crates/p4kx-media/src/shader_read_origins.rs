// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::reference_resolve::ReferenceObservations;
use crate::shader_flags::ShaderRead;
use crate::shader_origins::{ShaderCandidate, ShaderCandidateInput};
use crate::{MediaError, MediaWriter, origin_error};

pub(crate) fn input<'a>(
    read: &'a ShaderRead,
    writer: &MediaWriter<'_>,
) -> Result<Option<&'a MediaOriginInput>, MediaError> {
    writer.checkpoint()?;
    let Some(receipt) = &read.receipt else {
        return Ok(None);
    };
    let input = receipt.origin.as_ref().ok_or_else(|| {
        MediaError::Contract("cached shader read lost its original input evidence".into())
    })?;
    if !input.owner.is_same_inventory(&writer.origins.owner) {
        return Err(origin_error::foreign());
    }
    Ok(Some(input))
}

pub(crate) fn layout(
    read: Option<&ShaderRead>,
    candidates: &[ShaderCandidate],
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let Some(read) = read else {
        return Ok(OriginContributions::independent(&writer.origins.owner));
    };
    let mut capture = writer.capture_origins();
    if let Some(input) = input(read, writer)? {
        capture.add(
            input,
            InputScope::Payload,
            OriginSite::new("shader-definition", &[], "layout"),
            SourceUseRole::Conversion,
            writer,
        )?;
    } else if let Some(candidate) = candidates.first() {
        capture.add(
            &candidate.evidence.input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-directory", &[], "uncompressedSize"),
            SourceUseRole::Conversion,
            writer,
        )?;
    } else {
        capture.include(&OriginContributions::unknown(&writer.origins.owner), writer)?;
    }
    Ok(capture.finish())
}

pub(crate) fn flag(
    read: &ShaderRead,
    evidence: &ReferenceObservations<ShaderCandidateInput>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let input = input(read, writer)?
        .ok_or_else(|| MediaError::Contract("parsed shader has no original read input".into()))?;
    let observed = evidence.retained.iter().chain(evidence.discarded.iter());
    let prefix = if evidence.inventory_used {
        read.properties.len()
    } else {
        observed
            .filter_map(|candidate| candidate.property)
            .max()
            .map_or(0, |position| position.saturating_add(1))
    };
    let mut capture = writer.capture_origins();
    for property in read.properties.iter().take(prefix) {
        let ordinal = *property.target.indices.first().ok_or_else(|| {
            MediaError::Contract("parsed shader Property lost its original ordinal".into())
        })?;
        let indices = [ordinal, property.name_ordinal];
        capture.add(
            input,
            InputScope::Payload,
            OriginSite::new("shader-property", &indices, "Name"),
            SourceUseRole::Selector,
            writer,
        )?;
    }
    if evidence.inventory_used {
        capture.add(
            input,
            InputScope::Payload,
            OriginSite::new("shader-definition", &[], "Property.Name"),
            SourceUseRole::Selector,
            writer,
        )?;
        if !read.unknown_names && evidence.retained.is_empty() {
            capture.add(
                input,
                InputScope::Payload,
                OriginSite::new("shader-definition", &[], "UsesCommonGlobalFlags"),
                SourceUseRole::Selector,
                writer,
            )?;
        }
    }
    Ok(capture.finish())
}
