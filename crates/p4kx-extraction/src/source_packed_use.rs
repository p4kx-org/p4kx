// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::field_origin::OriginContribution;
use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};
use crate::source_use::storage::PackedDataForgeLocation;
use crate::source_use::{InputScope, SourceUseDescription, SourceUseRef, SourceUseRole};

/// An unheld DataForge use stored without the live handle or its location cache.
#[derive(Clone)]
pub(crate) struct PackedSourceUse {
    source: PhysicalSourceRef,
    location: PackedDataForgeLocation,
    scope: InputScope,
    role: SourceUseRole,
    encryption: SourceEncryptionState,
}

impl PackedSourceUse {
    pub(crate) fn pack(row: &SourceUseDescription) -> Option<Self> {
        Some(Self {
            source: row.source().clone(),
            location: row.location.pack_dataforge()?,
            scope: row.scope(),
            role: row.role(),
            encryption: row.effective_encryption(),
        })
    }

    pub(crate) fn restore(&self, id: &SourceUseRef) -> SourceUseDescription {
        SourceUseDescription {
            id: id.clone(),
            source: self.source.clone(),
            scope: self.scope,
            role: self.role,
            location: self.location.retain(id),
            effective_encryption: self.encryption,
        }
    }

    pub(crate) const fn contribution<'a>(&self, id: &'a SourceUseRef) -> OriginContribution<'a> {
        OriginContribution {
            id,
            role: self.role,
            encryption: self.encryption,
            opaque: false,
        }
    }
}
