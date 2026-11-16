// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, InputScope, SourceCoordinateOmission, SourceDetails, SourceEncryption,
    SourceEvidenceError, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

use super::{AudioCandidate, AudioCandidateState, AudioSurvey};

/// The amount of provenance retained from an observed audio index search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSurveyProof {
    /// Retain a representative contributor, preferring known encryption.
    Witness,
    /// Register every observed candidate and its actual admission state.
    Complete,
}

impl AudioSurvey<'_, '_> {
    /// Registers the actual inputs used in the bound index search.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain a source observation.
    pub fn origins(
        &self,
        uses: &SourceUses,
        mode: AudioSurveyProof,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let directory = uses.observe_root_directory_paths(self.archive)?;
        let complete = self.complete() && directory.is_some();
        let mut facts: Vec<_> = directory.into_iter().collect();
        let mut candidates = self.candidates();
        let all_candidates = match mode {
            AudioSurveyProof::Complete => {
                for candidate in candidates {
                    facts.push(candidate_fact(uses, candidate)?);
                }
                true
            }
            AudioSurveyProof::Witness => {
                let first = candidates.next();
                let second = candidates.next();
                let all_candidates = second.is_none();
                let witness = first
                    .filter(encrypted_contributor)
                    .or_else(|| {
                        second
                            .into_iter()
                            .chain(candidates)
                            .find(encrypted_contributor)
                    })
                    .or(first);
                if let Some(candidate) = witness {
                    facts.push(candidate_fact(uses, candidate)?);
                }
                all_candidates
            }
        };
        Ok(uses.origin(&facts, complete && all_candidates))
    }
}

fn encrypted_contributor(candidate: &AudioCandidate<'_>) -> bool {
    candidate.has_retained_input()
        && candidate.entry().stored_encryption() == SourceEncryption::Encrypted
}

fn candidate_fact(
    uses: &SourceUses,
    candidate: AudioCandidate<'_>,
) -> Result<SourceUse, SourceEvidenceError> {
    let source = uses.observe_with_details(
        SourceInput::ArchiveView(candidate.entry()),
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    let role = if candidate.has_retained_input() {
        SourceUseRole::Selector
    } else {
        SourceUseRole::Attempted
    };
    if candidate.state() == AudioCandidateState::CapacityLimited && candidate.has_retained_input() {
        return source.at_opaque(role, SourceCoordinateOmission::CaptureLimit);
    }
    candidate.decoded_bytes().map_or_else(
        || {
            source.at_opaque(
                SourceUseRole::Attempted,
                SourceCoordinateOmission::Unavailable,
            )
        },
        |len| {
            source.at(
                SourceLocation::Subresource {
                    namespace: "entryDecodedBytes".into(),
                    indices: vec![0, len],
                    field: "audioIndexSurvey".into(),
                },
                role,
            )
        },
    )
}
