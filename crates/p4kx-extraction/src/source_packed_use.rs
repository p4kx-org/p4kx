// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::field_origin::OriginContribution;
use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};
use crate::source_use::storage::PackedDataForgeLocation;
use crate::source_use::{
    InputScope, OriginUseId, SourceUseDescription, SourceUseRef, SourceUseRole,
};

pub(crate) type PackedUseTree = BTreeMap<Arc<str>, PackedSourceUse>;

/// An unheld DataForge use stored without the live handle or its location cache.
#[derive(Clone)]
pub(crate) struct PackedSourceUse {
    source: Arc<PhysicalSourceRef>,
    // The use facts fit beside the coordinate tag before the aligned fields.
    location: PackedDataForgeLocation<PackedUseFacts>,
}

const _: () = assert!(
    size_of::<(Arc<str>, PackedSourceUse)>() <= 5 * size_of::<u64>(),
    "the packed source-use entry fits in 40 bytes"
);

#[derive(Clone, Copy)]
struct PackedUseFacts {
    scope_encryption: ScopeEncryption,
    role: SourceUseRole,
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ScopeEncryption {
    PayloadEncrypted,
    PayloadUnencrypted,
    PayloadUnknown,
    DirectoryEncrypted,
    DirectoryUnencrypted,
    DirectoryUnknown,
    FileEncrypted,
    FileUnencrypted,
    FileUnknown,
}

impl ScopeEncryption {
    const fn new(scope: InputScope, encryption: SourceEncryptionState) -> Self {
        use InputScope::{ArchiveDirectoryMetadata, FileMetadata, Payload};
        use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};

        match (scope, encryption) {
            (Payload, Encrypted) => Self::PayloadEncrypted,
            (Payload, Unencrypted) => Self::PayloadUnencrypted,
            (Payload, Unknown) => Self::PayloadUnknown,
            (ArchiveDirectoryMetadata, Encrypted) => Self::DirectoryEncrypted,
            (ArchiveDirectoryMetadata, Unencrypted) => Self::DirectoryUnencrypted,
            (ArchiveDirectoryMetadata, Unknown) => Self::DirectoryUnknown,
            (FileMetadata, Encrypted) => Self::FileEncrypted,
            (FileMetadata, Unencrypted) => Self::FileUnencrypted,
            (FileMetadata, Unknown) => Self::FileUnknown,
        }
    }

    const fn into_parts(self) -> (InputScope, SourceEncryptionState) {
        use InputScope::{ArchiveDirectoryMetadata, FileMetadata, Payload};
        use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};

        match self {
            Self::PayloadEncrypted => (Payload, Encrypted),
            Self::PayloadUnencrypted => (Payload, Unencrypted),
            Self::PayloadUnknown => (Payload, Unknown),
            Self::DirectoryEncrypted => (ArchiveDirectoryMetadata, Encrypted),
            Self::DirectoryUnencrypted => (ArchiveDirectoryMetadata, Unencrypted),
            Self::DirectoryUnknown => (ArchiveDirectoryMetadata, Unknown),
            Self::FileEncrypted => (FileMetadata, Encrypted),
            Self::FileUnencrypted => (FileMetadata, Unencrypted),
            Self::FileUnknown => (FileMetadata, Unknown),
        }
    }
}

impl PackedSourceUse {
    pub(crate) const fn has_direct_field(&self) -> bool {
        self.location.has_direct_field()
    }

    pub(crate) fn pack(
        row: &SourceUseDescription,
        sources: &mut BTreeSet<Arc<PhysicalSourceRef>>,
    ) -> Option<Self> {
        let facts = PackedUseFacts {
            scope_encryption: ScopeEncryption::new(row.scope(), row.effective_encryption()),
            role: row.role(),
        };
        let location = row.location.pack_dataforge_with(facts)?;
        let source = intern_source(sources, row.source())?;
        Some(Self { source, location })
    }

    pub(crate) fn restore(&self, text: &Arc<str>) -> SourceUseDescription {
        let id = SourceUseRef::from_registered(Arc::clone(text));
        let location = self.location.retain(&id);
        let facts = self.location.extra();
        let (scope, effective_encryption) = facts.scope_encryption.into_parts();
        SourceUseDescription {
            id,
            source: self.source.as_ref().clone(),
            scope,
            role: facts.role,
            location,
            effective_encryption,
        }
    }

    pub(crate) const fn contribution<'a>(&self, text: &'a Arc<str>) -> OriginContribution<'a> {
        let facts = self.location.extra();
        let (_, encryption) = facts.scope_encryption.into_parts();
        OriginContribution {
            id: OriginUseId::Registered(text),
            role: facts.role,
            encryption,
            opaque: false,
        }
    }
}

fn intern_source(
    sources: &mut BTreeSet<Arc<PhysicalSourceRef>>,
    source: &PhysicalSourceRef,
) -> Option<Arc<PhysicalSourceRef>> {
    if let Some(shared) = sources.get(source) {
        // Each packed row keeps the original physical reference allocation.
        return std::ptr::eq(shared.as_str(), source.as_str()).then(|| Arc::clone(shared));
    }
    let shared = Arc::new(source.clone());
    sources.insert(Arc::clone(&shared));
    Some(shared)
}

#[cfg(test)]
#[path = "source_packed_use_wire_tests.rs"]
mod wire_tests;

#[cfg(test)]
mod tests {
    use super::{Arc, BTreeSet, PhysicalSourceRef, intern_source};

    #[test]
    fn packed_physical_references_share_only_the_original_allocation() -> Result<(), &'static str> {
        let original = PhysicalSourceRef::new("synthetic-source");
        let duplicate = PhysicalSourceRef::new(original.as_str());
        let mut sources = BTreeSet::new();
        let first = intern_source(&mut sources, &original).ok_or("original source")?;
        let alias = original.clone();
        let second = intern_source(&mut sources, &alias).ok_or("repeated source")?;
        assert!(
            Arc::ptr_eq(&first, &second),
            "cold rows share the retained physical reference"
        );
        assert!(
            std::ptr::eq(first.as_str(), original.as_str()),
            "sharing preserves the original physical reference allocation"
        );
        assert_eq!(Arc::strong_count(&first), 3, "two rows and the registry");
        assert_eq!(duplicate, original, "the physical text compares equal");
        assert!(
            intern_source(&mut sources, &duplicate).is_none(),
            "equal text from a separate allocation stays live"
        );
        drop(second);
        assert_eq!(Arc::strong_count(&first), 2, "one row and the registry");
        drop(sources);
        assert_eq!(Arc::strong_count(&first), 1, "the last cold row");
        Ok(())
    }
}
