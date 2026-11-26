// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;

use thiserror::Error;

use super::{IndexedReference, ReferenceIndex};
use crate::{DataForge, DataForgeError, DataType, ValueError};

/// A gap in the actual canonical Reference scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReferenceScanGap {
    /// The canonical tree could not be read.
    Record {
        /// The original physical Record position.
        record_index: u32,
    },
    /// An unreadable value could conceal an original Reference.
    Value {
        /// The original physical Record position.
        record_index: u32,
        /// The original canonical preorder position, excluding the root Struct.
        value_ordinal: u64,
    },
}

impl ReferenceScanGap {
    /// Returns the physical Record whose scan has the gap.
    #[must_use]
    pub const fn record_index(self) -> u32 {
        match self {
            Self::Record { record_index } | Self::Value { record_index, .. } => record_index,
        }
    }
}

/// A failure to prepare original Reference evidence.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReferenceIndexError {
    /// Addressable original-evidence storage could not be allocated.
    #[error("cannot reserve original Reference evidence")]
    Allocation(#[from] TryReserveError),
    /// The canonical read failed before source evidence could be completed.
    #[error(transparent)]
    Read(#[from] DataForgeError),
    /// The private index and original-evidence columns disagree.
    #[error("the Reference index and its original observations are inconsistent")]
    Inconsistent,
}

/// One index kept with its actual original DataForge and reference occurrences.
#[derive(Debug)]
pub struct ObservedReferenceIndex<'source> {
    forge: &'source DataForge<'source>,
    index: ReferenceIndex,
    evidence: Evidence,
}

impl<'source> ObservedReferenceIndex<'source> {
    /// Builds the index and its original occurrence evidence in one canonical scan.
    ///
    /// # Errors
    ///
    /// Returns an error when the evidence cannot be allocated or the operation cannot finish.
    pub fn build(forge: &'source DataForge<'source>) -> Result<Self, ReferenceIndexError> {
        let (index, evidence) =
            ReferenceIndex::scan::<true>(forge, usize::MAX, &|| false, EvidencePolicy::Required)
                .ok_or(ReferenceIndexError::Inconsistent)?;
        let mut evidence = evidence.ok_or(ReferenceIndexError::Inconsistent)?;
        if let Some(error) = evidence.fault.take() {
            return Err(error);
        }
        if index.is_truncated() || evidence.ordinals.len() != index.len() {
            return Err(ReferenceIndexError::Inconsistent);
        }
        Ok(Self {
            forge,
            index,
            evidence,
        })
    }

    /// Borrows the actual original DataForge.
    #[must_use]
    pub const fn forge(&self) -> &'source DataForge<'source> {
        self.forge
    }

    /// Borrows the index paired with these original observations.
    #[must_use]
    pub const fn index(&self) -> &ReferenceIndex {
        &self.index
    }

    /// Returns original gaps in Record and value order.
    #[must_use]
    pub fn gaps(&self) -> &[ReferenceScanGap] {
        &self.evidence.gaps
    }

    /// Returns the actual original position when the occurrence belongs to this index.
    #[must_use]
    pub fn origin(&self, reference: IndexedReference<'_>) -> Option<IndexedReferenceOrigin<'_>> {
        self.evidence.origin(&self.index, reference)
    }
}

/// A retained Reference with its exact original value position.
#[derive(Debug, Clone, Copy)]
pub struct IndexedReferenceOrigin<'index> {
    reference: IndexedReference<'index>,
    value_ordinal: u64,
}

impl<'index> IndexedReferenceOrigin<'index> {
    /// Returns the actual indexed Reference.
    #[must_use]
    pub const fn reference(&self) -> IndexedReference<'index> {
        self.reference
    }

    /// Returns the original value's canonical preorder ordinal.
    #[must_use]
    pub const fn value_ordinal(&self) -> u64 {
        self.value_ordinal
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum EvidencePolicy {
    Required,
    Optional,
}

#[derive(Debug)]
pub(super) struct Evidence {
    policy: EvidencePolicy,
    ordinals: Vec<u64>,
    gaps: Vec<ReferenceScanGap>,
    fault: Option<ReferenceIndexError>,
}

impl Evidence {
    pub(super) const fn new(policy: EvidencePolicy) -> Self {
        Self {
            policy,
            ordinals: Vec::new(),
            gaps: Vec::new(),
            fault: None,
        }
    }

    pub(super) fn origin<'index>(
        &self,
        index: &'index ReferenceIndex,
        reference: IndexedReference<'_>,
    ) -> Option<IndexedReferenceOrigin<'index>> {
        if !std::ptr::eq(reference.index, index) {
            return None;
        }
        Some(IndexedReferenceOrigin {
            reference: IndexedReference {
                index,
                entry: reference.entry,
            },
            value_ordinal: *self.ordinals.get(reference.entry)?,
        })
    }

    pub(super) fn gaps(&self) -> &[ReferenceScanGap] {
        &self.gaps
    }

    pub(super) const fn covers(&self, length: usize) -> bool {
        !self.failed() && self.ordinals.len() == length
    }

    pub(super) const fn aborts_index(&self) -> bool {
        self.failed() && matches!(self.policy, EvidencePolicy::Required)
    }

    pub(super) const fn failed(&self) -> bool {
        self.fault.is_some()
    }

    pub(super) fn fail(&mut self, error: ReferenceIndexError) {
        self.fault.get_or_insert(error);
        self.ordinals = Vec::new();
        self.gaps = Vec::new();
    }

    pub(super) fn retain(&mut self, ordinal: u64) -> Result<(), ReferenceIndexError> {
        self.ordinals.try_reserve(1)?;
        self.ordinals.push(ordinal);
        Ok(())
    }

    pub(super) fn tree_error(&mut self, record_index: u32, error: DataForgeError) {
        if matches!(
            error,
            DataForgeError::ObservationStopped
                | DataForgeError::TooLarge { .. }
                | DataForgeError::MaterializationBudgetExceeded { .. }
                | DataForgeError::MaterializationAllocationRefused { .. }
                | DataForgeError::LayoutAllocationRefused { .. }
                | DataForgeError::LayoutInvariant
                | DataForgeError::CanonicalTreeInvariant
        ) {
            self.fail(error.into());
        } else {
            self.gap(ReferenceScanGap::Record { record_index });
        }
    }

    pub(super) fn value_error(
        &mut self,
        record_index: u32,
        value_ordinal: u64,
        data_type: DataType,
        error: ValueError,
    ) {
        let may_hold_references = matches!(
            data_type,
            DataType::Reference | DataType::Class | DataType::StrongPointer | DataType::WeakPointer
        );
        if may_hold_references
            && !matches!(
                error,
                ValueError::DanglingReference { .. } | ValueError::OutsideRecord { .. }
            )
        {
            self.gap(ReferenceScanGap::Value {
                record_index,
                value_ordinal,
            });
        }
    }

    fn gap(&mut self, gap: ReferenceScanGap) {
        if let Err(error) = self.gaps.try_reserve(1) {
            self.fail(error.into());
        } else {
            self.gaps.push(gap);
        }
    }

    pub(super) fn append(&mut self, chunk: Self, kept: usize) -> Result<(), ReferenceIndexError> {
        if self.failed() {
            return Ok(());
        }
        if let Some(error) = chunk.fault {
            return Err(error);
        }
        let ordinals = chunk
            .ordinals
            .get(..kept)
            .ok_or(ReferenceIndexError::Inconsistent)?;
        self.ordinals.try_reserve(ordinals.len())?;
        self.gaps.try_reserve(chunk.gaps.len())?;
        self.ordinals.extend_from_slice(ordinals);
        self.gaps.extend(chunk.gaps);
        Ok(())
    }
}
