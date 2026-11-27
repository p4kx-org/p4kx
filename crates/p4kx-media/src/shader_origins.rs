// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ReferenceCandidate;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::reference_resolve::ReferenceObservations;

#[derive(Clone)]
pub(crate) struct ShaderCandidateInput {
    pub(crate) input: MediaOriginInput,
    pub(crate) property: Option<usize>,
    pub(crate) site: Option<crate::origin_reference::PropertySite>,
}

pub(crate) struct ShaderCandidate {
    pub(crate) value: ReferenceCandidate,
    pub(crate) evidence: ShaderCandidateInput,
}

pub(crate) struct ShaderReferenceOrigins {
    pub(crate) candidates: ReferenceObservations<ShaderCandidateInput>,
    pub(crate) inventory: OriginContributions,
    pub(crate) semantic: OriginContributions,
    pub(crate) read_selected: bool,
}

pub(crate) struct ShaderFlagBinding {
    pub(crate) reference: crate::Reference,
    pub(crate) input: Option<crate::InputReceipt>,
    pub(crate) origins: ShaderReferenceOrigins,
}
impl crate::origin_reference::CandidateInput for ShaderCandidateInput {
    fn original(&self) -> &MediaOriginInput {
        &self.input
    }
    fn property_site(&self) -> Option<crate::origin_reference::PropertySite> {
        self.site
    }
}
pub(crate) fn unavailable(
    original: crate::OriginalIdentifier,
    reason: crate::ReferenceUnavailable,
    writer: &mut crate::MediaWriter<'_>,
) -> Result<ShaderFlagBinding, crate::MediaError> {
    let (reference, candidates) = writer.try_resolve_reference_observed::<ShaderCandidateInput>(
        original,
        std::iter::empty(),
        crate::CandidateInventory::Unavailable(reason),
    )?;
    Ok(ShaderFlagBinding {
        reference,
        input: None,
        origins: ShaderReferenceOrigins {
            candidates,
            inventory: OriginContributions::independent(&writer.origins.owner),
            semantic: OriginContributions::independent(&writer.origins.owner),
            read_selected: false,
        },
    })
}
