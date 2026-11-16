// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::RecordJsonUseRole::{Conversion, Payload, Selector};
use crate::positioned_record::ValueSlot;
use crate::{
    DataType, PositionedRecord, PositionedReferenceSources, PositionedSources, RecordJsonBytes,
    RecordJsonSource, RecordJsonUse, RecordJsonUseRole,
};

/// An emitted member of a canonical Record Reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordJsonReferenceMember {
    /// The GUID of the selected Record or the owner of the selected instance.
    Guid,
    /// The Record filename with its optional suffix removed.
    Path,
    /// The struct name of the selected Record or instance.
    Type,
    /// The selection path within the owning Record.
    InstancePath,
}

#[derive(Clone, Copy)]
pub(crate) enum DanglingReferenceMember {
    Error,
    Guid,
}

/// The local contributors and completeness of one Reference observation.
#[derive(Debug)]
pub struct RecordJsonReferenceSources {
    sources: Vec<RecordJsonUse>,
    complete: bool,
}

impl RecordJsonReferenceSources {
    const fn new() -> Self {
        Self {
            sources: Vec::new(),
            complete: true,
        }
    }

    pub(crate) const fn unavailable() -> Self {
        Self {
            sources: Vec::new(),
            complete: false,
        }
    }

    pub(crate) fn dangling(
        input: &PositionedRecord<'_>,
        slot: Option<ValueSlot<'_>>,
        member: DanglingReferenceMember,
    ) -> Self {
        let Some(slot) = slot.filter(|slot| slot.data_type == DataType::Reference) else {
            return Self::unavailable();
        };
        let mut result = Self::new();
        let role = match member {
            DanglingReferenceMember::Error => {
                result.complete = false;
                Conversion
            }
            DanglingReferenceMember::Guid => Payload,
        };
        let stored = PositionedReferenceSources::caller_storage(input, slot);
        result.append(&stored, |source| match source {
            RecordJsonSource::Bytes {
                kind: RecordJsonBytes::Slot | RecordJsonBytes::ArrayElement,
                ..
            } => RecordJsonUse { source, role },
            _ => RecordJsonUse::selection(source),
        });
        result
    }

    /// Returns the original coordinates and contribution roles.
    #[must_use]
    pub fn sources(&self) -> &[RecordJsonUse] {
        &self.sources
    }

    /// Returns whether all required local contributions were observed.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.complete
    }

    fn require(&mut self, source: Option<RecordJsonSource<'static>>, role: RecordJsonUseRole) {
        self.complete &= source.is_some();
        self.sources
            .extend(source.map(|source| RecordJsonUse { source, role }));
    }

    fn append(
        &mut self,
        sources: &PositionedSources,
        use_source: impl Fn(RecordJsonSource<'static>) -> RecordJsonUse,
    ) {
        self.complete &= sources.complete();
        self.sources
            .extend(sources.sources().iter().copied().map(use_source));
    }
}

impl PositionedReferenceSources {
    /// Observes the contributors of one emitted member, excluding the caller's ancestors.
    #[must_use]
    pub fn member(&self, member: RecordJsonReferenceMember) -> RecordJsonReferenceSources {
        let mut result = RecordJsonReferenceSources::new();
        result.append(self.caller(), RecordJsonUse::selection);
        if member == RecordJsonReferenceMember::Guid && self.has_stored_guid() {
            result.require(self.slot(), Payload);
            return result;
        }
        result.require(self.slot(), Selector);
        result.append(self.target_route(), RecordJsonUse::selection);
        match member {
            RecordJsonReferenceMember::Guid => result.require(self.target_guid(), Payload),
            RecordJsonReferenceMember::Path => result.append(self.record_path(), record_path_use),
            RecordJsonReferenceMember::Type => result.append(self.target_type(), target_type_use),
            RecordJsonReferenceMember::InstancePath => {
                if let Some(path) = self.instance_path() {
                    result.append(path, instance_path_use);
                } else {
                    result.complete = false;
                }
            }
        }
        result
    }

    /// Observes the source selection that admitted the decoded Reference kind.
    #[must_use]
    pub fn kind(&self) -> RecordJsonReferenceSources {
        let mut result = RecordJsonReferenceSources::new();
        result.append(self.caller(), RecordJsonUse::selection);
        result.require(self.slot(), Selector);
        result.append(self.record_path(), record_path_use);
        result.append(self.target_route(), RecordJsonUse::selection);
        result.append(self.target_type(), target_type_use);
        result
    }
}

const fn record_path_use(source: RecordJsonSource<'static>) -> RecordJsonUse {
    let role = match source {
        RecordJsonSource::Bytes {
            kind: RecordJsonBytes::Text,
            ..
        } => Payload,
        _ => Conversion,
    };
    RecordJsonUse { source, role }
}

const fn target_type_use(source: RecordJsonSource<'static>) -> RecordJsonUse {
    let role = match source {
        RecordJsonSource::StructName { .. } => Payload,
        _ => Selector,
    };
    RecordJsonUse { source, role }
}

const fn instance_path_use(source: RecordJsonSource<'static>) -> RecordJsonUse {
    let role = match source {
        RecordJsonSource::Bytes {
            kind: RecordJsonBytes::Text,
            ..
        } => Conversion,
        _ => Selector,
    };
    RecordJsonUse { source, role }
}
