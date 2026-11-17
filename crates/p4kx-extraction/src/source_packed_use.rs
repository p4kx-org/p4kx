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
    location: PackedDataForgeLocation,
    scope: InputScope,
    role: SourceUseRole,
    encryption: SourceEncryptionState,
}

const _: () = assert!(
    size_of::<(Arc<str>, PackedSourceUse)>() <= 6 * size_of::<u64>(),
    "the packed source-use entry fits in 48 bytes"
);

impl PackedSourceUse {
    pub(crate) const fn has_direct_field(&self) -> bool {
        self.location.has_direct_field()
    }

    pub(crate) fn pack(
        row: &SourceUseDescription,
        sources: &mut BTreeSet<Arc<PhysicalSourceRef>>,
    ) -> Option<Self> {
        let location = row.location.pack_dataforge()?;
        let source = intern_source(sources, row.source())?;
        Some(Self {
            source,
            location,
            scope: row.scope(),
            role: row.role(),
            encryption: row.effective_encryption(),
        })
    }

    pub(crate) fn restore(&self, text: &Arc<str>) -> SourceUseDescription {
        let id = SourceUseRef::from_registered(Arc::clone(text));
        let location = self.location.retain(&id);
        SourceUseDescription {
            id,
            source: self.source.as_ref().clone(),
            scope: self.scope,
            role: self.role,
            location,
            effective_encryption: self.encryption,
        }
    }

    pub(crate) const fn contribution<'a>(&self, text: &'a Arc<str>) -> OriginContribution<'a> {
        OriginContribution {
            id: OriginUseId::Registered(text),
            role: self.role,
            encryption: self.encryption,
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
