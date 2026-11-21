// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{PositionedReferenceSources, RecordJsonReferenceMember};

use crate::RecordSide;
use crate::field_change_evidence::Coordinates;

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
    guid: Coordinates,
    record_path: Coordinates,
    target_type: Coordinates,
    at: Coordinates,
    kind: Coordinates,
}

impl ReferenceEvidence {
    pub(crate) fn capture(side: RecordSide, input: &PositionedReferenceSources) -> Self {
        Self {
            guid: Coordinates::from_reference(side, &input.member(RecordJsonReferenceMember::Guid)),
            record_path: Coordinates::from_reference(
                side,
                &input.member(RecordJsonReferenceMember::Path),
            ),
            target_type: Coordinates::from_reference(
                side,
                &input.member(RecordJsonReferenceMember::Type),
            ),
            at: Coordinates::from_reference(
                side,
                &input.member(RecordJsonReferenceMember::InstancePath),
            ),
            kind: Coordinates::from_reference(side, &input.kind()),
        }
    }

    pub(crate) const fn member(&self, member: ReferenceMember) -> &Coordinates {
        match member {
            ReferenceMember::Guid => &self.guid,
            ReferenceMember::RecordPath => &self.record_path,
            ReferenceMember::Type => &self.target_type,
            ReferenceMember::At => &self.at,
        }
    }

    pub(crate) const fn kind(&self) -> &Coordinates {
        &self.kind
    }
}
