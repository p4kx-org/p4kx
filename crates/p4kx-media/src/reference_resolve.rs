// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::{
    CandidateInventory, CandidateState, Identity, MediaError, MediaWriter, OriginalIdentifier,
    Reference, ReferenceCandidate, ReferenceResolution, ReferenceTarget, ReferenceUnavailable,
};

impl MediaWriter<'_> {
    /// Classifies actual domain-reader candidates without performing another lookup or read.
    ///
    /// Resolved proves target identity only; coverage and raw preservation remain independent.
    ///
    /// # Errors
    /// Returns invalid target provenance, cancellation or the global reference-work limit.
    pub fn resolve_reference(
        &mut self,
        original: OriginalIdentifier,
        candidates: impl IntoIterator<Item = ReferenceCandidate>,
        inventory: CandidateInventory,
    ) -> Result<Reference, MediaError> {
        self.try_resolve_reference(original, candidates.into_iter().map(Ok), inventory)
    }

    /// Classifies a fallible source-bound candidate scan without hiding interrupted inventory.
    ///
    /// # Errors
    /// Returns the original scan error, invalid target provenance or reference-work failure.
    pub fn try_resolve_reference(
        &mut self,
        original: OriginalIdentifier,
        candidates: impl IntoIterator<Item = Result<ReferenceCandidate, MediaError>>,
        inventory: CandidateInventory,
    ) -> Result<Reference, MediaError> {
        self.try_resolve_with(original, candidates, inventory, |candidate| (candidate, ()))
            .map(|(reference, _)| reference)
    }

    /// Retains caller-owned evidence for the admitted candidate prefix and one cutoff item.
    ///
    /// # Errors
    /// Returns the original scan error, invalid target provenance or reference-work failure.
    pub(crate) fn try_resolve_reference_observed<E>(
        &mut self,
        original: OriginalIdentifier,
        candidates: impl IntoIterator<Item = Result<(ReferenceCandidate, E), MediaError>>,
        inventory: CandidateInventory,
    ) -> Result<(Reference, ReferenceObservations<E>), MediaError> {
        self.try_resolve_with(original, candidates, inventory, |candidate| candidate)
    }

    fn try_resolve_with<E, C>(
        &mut self,
        original: OriginalIdentifier,
        candidates: impl IntoIterator<Item = Result<C, MediaError>>,
        inventory: CandidateInventory,
        split: impl Fn(C) -> (ReferenceCandidate, E),
    ) -> Result<(Reference, ReferenceObservations<E>), MediaError> {
        self.start_reference()?;
        let mut counter = Counter {
            remaining: self.limits().record_bytes,
            exceeded: false,
        };
        if !counter.include(&original)? {
            return Ok((
                unavailable(
                    original,
                    ReferenceUnavailable::CandidateBytes {
                        limit: self.limits().record_bytes,
                    },
                    Vec::new(),
                    0,
                ),
                ReferenceObservations::empty(),
            ));
        }
        let observed = collect(self, candidates, &mut counter, split)?;
        if let Some(reason) = observed.unavailable {
            return Ok((
                unavailable(original, reason, observed.known, observed.count),
                observed.evidence,
            ));
        }
        let mut evidence = observed.evidence;
        evidence.inventory_used = true;
        let reference = match inventory {
            CandidateInventory::Unavailable(reason) => {
                unavailable(original, reason, observed.known, observed.count)
            }
            CandidateInventory::Complete => Reference {
                original,
                resolution: classify(observed.known),
                observed_candidates: observed.count,
                candidates_complete: true,
            },
        };
        Ok((reference, evidence))
    }
}

/// Evidence admitted with candidate descriptors, without serialized-identity reconstruction.
pub(crate) struct ReferenceObservations<E> {
    /// Evidence in the same order as the retained candidate prefix.
    pub(crate) retained: Vec<E>,
    /// Evidence for the single observed candidate refused by a count or byte cutoff.
    pub(crate) discarded: Option<E>,
    /// Whether complete scanning reached the supplied inventory disposition.
    pub(crate) inventory_used: bool,
}

impl<E> ReferenceObservations<E> {
    const fn empty() -> Self {
        Self {
            retained: Vec::new(),
            discarded: None,
            inventory_used: false,
        }
    }
}

struct Observed<E> {
    known: Vec<ReferenceCandidate>,
    count: u64,
    unavailable: Option<ReferenceUnavailable>,
    evidence: ReferenceObservations<E>,
}

fn collect<E, C>(
    writer: &MediaWriter<'_>,
    candidates: impl IntoIterator<Item = Result<C, MediaError>>,
    counter: &mut Counter,
    split: impl Fn(C) -> (ReferenceCandidate, E),
) -> Result<Observed<E>, MediaError> {
    let limits = writer.limits();
    let mut observed = Observed {
        known: Vec::new(),
        count: 0,
        unavailable: None,
        evidence: ReferenceObservations::empty(),
    };
    for candidate in candidates {
        let (candidate, evidence) = split(candidate?);
        writer.checkpoint()?;
        observed.count += 1;
        if observed.count > u64::from(limits.reference_candidates) {
            observed.unavailable = Some(ReferenceUnavailable::CandidateLimit {
                limit: limits.reference_candidates,
            });
            observed.evidence.discarded = Some(evidence);
            break;
        }
        if &candidate.target.root != writer.reference_root()? {
            return Err(MediaError::Contract(
                "reference target belongs to a different root Archive".into(),
            ));
        }
        validate_target(&candidate.target, limits.depth)?;
        if !counter.include(&candidate)? {
            observed.unavailable = Some(ReferenceUnavailable::CandidateBytes {
                limit: limits.record_bytes,
            });
            observed.evidence.discarded = Some(evidence);
            break;
        }
        observed.known.push(candidate);
        observed.evidence.retained.push(evidence);
    }
    Ok(observed)
}
fn validate_target(target: &ReferenceTarget, depth: u32) -> Result<(), MediaError> {
    if target.subresource.is_some() && target.kind.is_none() {
        return Err(MediaError::Contract(
            "reference subresource is not qualified by its provider kind".into(),
        ));
    }
    if let Identity::Entry { containers, entry } = &target.source {
        if containers.len() as u64 > u64::from(depth) {
            return Err(MediaError::Contract(
                "reference target container chain exceeds its source limit".into(),
            ));
        }
        for identity in containers.iter().chain(std::iter::once(entry)) {
            if u32::try_from(identity.entry_index).is_err() {
                return Err(MediaError::Contract(
                    "reference target index is outside the Archive reader's physical index range"
                        .into(),
                ));
            }
            if identity.raw_path_hex.as_ref().is_some_and(|hex| {
                !hex.len().is_multiple_of(2) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            }) {
                return Err(MediaError::Contract(
                    "reference target has malformed retained path bytes".into(),
                ));
            }
        }
    }
    if target
        .subresource
        .as_ref()
        .is_some_and(|subresource| subresource.namespace.is_empty())
    {
        return Err(MediaError::Contract(
            "reference subresource has no namespace".into(),
        ));
    }
    Ok(())
}

fn classify(mut candidates: Vec<ReferenceCandidate>) -> ReferenceResolution {
    if candidates.len() > 1 {
        return ReferenceResolution::Ambiguous { candidates };
    }
    let Some(candidate) = candidates.pop() else {
        return ReferenceResolution::Missing;
    };
    match candidate.state {
        CandidateState::Selected => ReferenceResolution::Resolved {
            target: candidate.target,
        },
        CandidateState::NotSelected { reason } => ReferenceResolution::NotSelected {
            target: candidate.target,
            reason,
        },
        CandidateState::Unavailable { reason } => ReferenceResolution::Unavailable {
            reason,
            known_candidates: vec![candidate],
        },
    }
}

const fn unavailable(
    original: OriginalIdentifier,
    reason: ReferenceUnavailable,
    known_candidates: Vec<ReferenceCandidate>,
    observed_candidates: u64,
) -> Reference {
    Reference {
        original,
        resolution: ReferenceResolution::Unavailable {
            reason,
            known_candidates,
        },
        observed_candidates,
        candidates_complete: false,
    }
}

struct Counter {
    remaining: u64,
    exceeded: bool,
}

impl Counter {
    fn include<T: serde::Serialize>(&mut self, value: &T) -> Result<bool, MediaError> {
        match serde_json::to_writer(&mut *self, value) {
            Ok(()) => Ok(true),
            Err(_) if self.exceeded => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            self.exceeded = true;
            return Err(io::Error::other("reference candidate bytes exceeded"));
        }
        self.remaining -= bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "reference_admission_controls.rs"]
mod admission_controls;
#[cfg(test)]
#[path = "reference_admission_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "reference_admission_observed_tests.rs"]
mod observed_tests;
