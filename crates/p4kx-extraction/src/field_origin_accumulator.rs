// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::ProvenanceMode;
use crate::source_inventory::SourceEncryptionState;
use crate::source_use::{OriginUseId, SourceUseDescription, SourceUseRef, SourceUseRole};

use super::{FieldOrigin, SharedFieldOrigin};

pub(super) struct OriginFacts {
    pub(super) contains_encrypted_source: Option<bool>,
    pub(super) references: Vec<SourceUseRef>,
    pub(super) source_set_complete: bool,
    pub(super) mode: ProvenanceMode,
}

impl OriginFacts {
    pub(super) fn measure<'a>(
        uses: impl IntoIterator<Item = &'a SourceUseDescription>,
        complete: bool,
        mode: ProvenanceMode,
    ) -> Self {
        let mut accumulator = OriginAccumulator::new(complete, mode);
        for description in uses {
            accumulator.observe(OriginContribution::from(description));
        }
        accumulator.into_facts()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OriginContribution<'a> {
    pub(crate) id: OriginUseId<'a>,
    pub(crate) role: SourceUseRole,
    pub(crate) encryption: SourceEncryptionState,
    pub(crate) opaque: bool,
}

impl<'a> From<&'a SourceUseDescription> for OriginContribution<'a> {
    fn from(description: &'a SourceUseDescription) -> Self {
        Self {
            id: OriginUseId::Description(description.id()),
            role: description.role(),
            encryption: description.effective_encryption(),
            opaque: description.is_opaque(),
        }
    }
}

pub(crate) struct OriginAccumulator {
    references: Vec<SourceUseRef>,
    encryption: SourceEncryptionState,
    complete: bool,
    mode: ProvenanceMode,
}

impl OriginAccumulator {
    pub(crate) const fn new(complete: bool, mode: ProvenanceMode) -> Self {
        Self {
            references: Vec::new(),
            encryption: SourceEncryptionState::Unencrypted,
            complete,
            mode,
        }
    }

    pub(crate) const fn include_completeness(&mut self, complete: bool) {
        self.complete &= complete;
    }

    pub(crate) fn include_marker(&mut self, origin: &FieldOrigin) {
        self.include_completeness(origin.source_set_complete());
        let encryption = match origin.contains_encrypted_source() {
            Some(true) => SourceEncryptionState::Encrypted,
            Some(false) => SourceEncryptionState::Unencrypted,
            None => SourceEncryptionState::Unknown,
        };
        self.encryption = self.encryption.combine(encryption);
    }

    pub(crate) fn observe(&mut self, contribution: OriginContribution<'_>) {
        if contribution.role.contributes() {
            self.include_source(contribution.encryption, contribution.opaque);
            if self.mode == ProvenanceMode::Full {
                self.references.push(contribution.id.retain());
            }
        }
    }

    pub(crate) const fn include_source(&mut self, encryption: SourceEncryptionState, opaque: bool) {
        self.include_completeness(!opaque);
        self.encryption = self.encryption.combine(encryption);
    }

    pub(crate) fn finish(self, identity: Arc<()>) -> FieldOrigin {
        FieldOrigin::from_facts(identity, self.into_facts())
    }

    pub(crate) fn finish_shared(self, identity: Arc<()>) -> SharedFieldOrigin {
        SharedFieldOrigin::from_facts(identity, self.into_facts())
    }

    pub(crate) fn finish_with_shared(self, shared: &SharedFieldOrigin) -> FieldOrigin {
        FieldOrigin::from_shared_facts(shared, self.into_facts())
    }

    fn into_facts(mut self) -> OriginFacts {
        self.references.sort();
        self.references.dedup();
        let contains_encrypted_source = match self.encryption {
            SourceEncryptionState::Encrypted => Some(true),
            SourceEncryptionState::Unencrypted if self.complete => Some(false),
            _ => None,
        };
        OriginFacts {
            contains_encrypted_source,
            references: self.references,
            source_set_complete: self.complete,
            mode: self.mode,
        }
    }
}
