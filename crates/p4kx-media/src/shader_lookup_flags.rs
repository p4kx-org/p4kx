// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::shader_flags::ShaderRead;
use crate::shader_lookup::ShaderLookup;
use crate::shader_origins::{ShaderCandidate, ShaderCandidateInput};
use crate::{
    CandidateInventory, CandidateState, MediaError, MediaKind, MediaWriter, ReferenceCandidate,
    ReferenceTarget, ReferenceUnavailable,
};

pub(crate) fn candidates(
    lookup: &ShaderLookup<'_>,
    read: &ShaderRead,
    name: &str,
    writer: &MediaWriter<'_>,
) -> Result<(Vec<ShaderCandidate>, CandidateInventory), MediaError> {
    let receipt = read
        .receipt
        .as_ref()
        .ok_or_else(|| MediaError::Contract("parsed shader has no input receipt".into()))?;
    let input = crate::shader_read_origins::input(read, writer)?
        .ok_or_else(|| MediaError::Contract("parsed shader has no original input".into()))?;
    let mut candidates = Vec::new();
    for (position, property) in read.properties.iter().enumerate() {
        lookup.work(
            property.name.len().min(name.len()).saturating_add(1),
            writer,
        )?;
        if property.name.eq_ignore_ascii_case(name) {
            candidates.try_reserve(1).map_err(|cause| MediaError::Io {
                path: writer.root.clone(),
                source: std::io::Error::new(std::io::ErrorKind::OutOfMemory, cause),
            })?;
            candidates.push(ShaderCandidate {
                value: ReferenceCandidate::new(
                    ReferenceTarget::new_with_subresource(
                        &receipt.source,
                        MediaKind::Shader,
                        property.target.clone(),
                    ),
                    CandidateState::Selected,
                ),
                evidence: ShaderCandidateInput {
                    input: input.clone(),
                    property: Some(position),
                    site: Some(crate::origin_reference::PropertySite {
                        ordinal: *property.target.indices.first().ok_or_else(|| {
                            MediaError::Contract("parsed Property lost its original ordinal".into())
                        })?,
                        name: property.name_ordinal,
                    }),
                },
            });
            if crate::shader_values::count(candidates.len())?
                > u64::from(writer.limits().reference_candidates)
            {
                break;
            }
        }
    }
    let inventory = if read.unknown_names || (read.common_flags && candidates.is_empty()) {
        CandidateInventory::Unavailable(ReferenceUnavailable::UnsupportedLayout)
    } else {
        CandidateInventory::Complete
    };
    Ok((candidates, inventory))
}
