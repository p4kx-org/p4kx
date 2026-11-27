// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, Value};

#[cfg(test)]
use super::frame::Frame;
use super::frame::OwnerPosition;
use super::path::OriginalPath;
#[cfg(test)]
use super::tokens::OriginalContainerKind;
use super::tokens::OriginalValueEdge;

/// One borrowed original value visit without any source-use registration capability.
pub(crate) struct OwnedValueVisit<'visit, 'read, 'source> {
    pub(super) field: &'read Field<'source>,
    pub(super) value: &'read Value<'source>,
    pub(super) ordinal: u64,
    pub(super) parent: Option<u64>,
    pub(super) edge: OriginalValueEdge,
    pub(super) owner: OwnerPosition<'source>,
    #[cfg(test)]
    pub(super) frames: &'visit [Frame<'read, 'source>],
    pub(super) path: &'visit OriginalPath,
    pub(super) declaration_ordinal: u64,
}

impl<'read, 'source> OwnedValueVisit<'_, 'read, 'source> {
    pub(crate) const fn value(&self) -> &'read Value<'source> {
        self.value
    }
    pub(crate) const fn field(&self) -> &'read Field<'source> {
        self.field
    }
    pub(crate) const fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub(crate) const fn parent_ordinal(&self) -> Option<u64> {
        self.parent
    }
    pub(crate) const fn edge(&self) -> OriginalValueEdge {
        self.edge
    }
    pub(crate) const fn owner_type(&self) -> &'source str {
        self.owner.type_name
    }
    pub(crate) const fn parent_type(&self) -> Option<&'source str> {
        self.owner.parent_type
    }
    #[cfg(test)]
    pub(crate) fn struct_depth(&self) -> usize {
        self.frames
            .iter()
            .skip(1)
            .filter(|frame| matches!(frame.kind(), OriginalContainerKind::Struct))
            .count()
    }

    pub(crate) fn logical_path(&self) -> &str {
        self.path.text()
    }
    pub(super) const fn declaration_ordinal(&self) -> u64 {
        self.declaration_ordinal
    }
}
