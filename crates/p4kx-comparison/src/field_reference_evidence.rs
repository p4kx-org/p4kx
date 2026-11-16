// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{
    PositionedReferenceSources, PositionedSources, RecordJsonBytes, RecordJsonSource,
};
use p4kx_extraction::SourceUseRole;

use crate::RecordSide;
use crate::field_change_evidence::{Coordinates, selection_role};

#[derive(Clone, Copy)]
pub(crate) enum ReferenceMember {
    Guid,
    RecordPath,
    Type,
    At,
}

impl ReferenceMember {
    pub(crate) const fn field(self) -> &'static str {
        match self {
            Self::Guid => "guid",
            Self::RecordPath => "record_path",
            Self::Type => "type",
            Self::At => "at",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReferenceEvidence {
    caller: Coordinates,
    selector: Coordinates,
    guid: Coordinates,
    record_path: Coordinates,
    target_type: Coordinates,
    target_route: Coordinates,
    at: Coordinates,
    stored_guid: bool,
}

impl ReferenceEvidence {
    pub(crate) fn capture(side: RecordSide, input: &PositionedReferenceSources) -> Self {
        let caller = observed(side, input.caller(), selection_role);
        let mut selector = Coordinates::complete();
        selector.require(side, input.slot(), SourceUseRole::Selector);
        let mut guid = Coordinates::complete();
        let stored_guid = input.has_stored_guid();
        let guid_subject = if stored_guid {
            input.slot()
        } else {
            input.target_guid()
        };
        guid.require(side, guid_subject, SourceUseRole::Payload);
        let record_path = observed(side, input.record_path(), pathname_role);
        let target_type = observed(side, input.target_type(), type_role);
        let target_route = observed(side, input.target_route(), selection_role);
        let mut at = Coordinates::complete();
        at.observe(side, input.instance_path(), owned_path_role);
        Self {
            caller,
            selector,
            guid,
            record_path,
            target_type,
            target_route,
            at,
            stored_guid,
        }
    }

    pub(crate) fn members(&self, member: ReferenceMember) -> impl Iterator<Item = &Coordinates> {
        let contributions = match member {
            ReferenceMember::Guid if self.stored_guid => [None, None, Some(&self.guid)],
            ReferenceMember::Guid => [Some(&self.selector), None, Some(&self.guid)],
            ReferenceMember::RecordPath => [Some(&self.selector), None, Some(&self.record_path)],
            ReferenceMember::Type => [
                Some(&self.selector),
                Some(&self.target_route),
                Some(&self.target_type),
            ],
            ReferenceMember::At => [
                Some(&self.selector),
                Some(&self.target_route),
                Some(&self.at),
            ],
        };
        std::iter::once(&self.caller).chain(contributions.into_iter().flatten())
    }

    pub(crate) fn kind(&self) -> impl Iterator<Item = &Coordinates> {
        [
            &self.caller,
            &self.selector,
            &self.record_path,
            &self.target_route,
            &self.target_type,
        ]
        .into_iter()
    }
}

fn observed(
    side: RecordSide,
    sources: &PositionedSources,
    role: impl Fn(RecordJsonSource<'_>) -> SourceUseRole,
) -> Coordinates {
    let mut result = Coordinates::complete();
    result.observe(side, Some(sources), role);
    result
}

const fn pathname_role(coordinate: RecordJsonSource<'_>) -> SourceUseRole {
    match coordinate {
        RecordJsonSource::Bytes {
            kind: RecordJsonBytes::Text,
            ..
        } => SourceUseRole::Payload,
        _ => SourceUseRole::Conversion,
    }
}

const fn type_role(coordinate: RecordJsonSource<'_>) -> SourceUseRole {
    match coordinate {
        RecordJsonSource::StructName { .. } => SourceUseRole::Payload,
        _ => SourceUseRole::Selector,
    }
}

const fn owned_path_role(coordinate: RecordJsonSource<'_>) -> SourceUseRole {
    match coordinate {
        RecordJsonSource::Bytes {
            kind: RecordJsonBytes::Text,
            ..
        } => SourceUseRole::Conversion,
        _ => SourceUseRole::Selector,
    }
}
