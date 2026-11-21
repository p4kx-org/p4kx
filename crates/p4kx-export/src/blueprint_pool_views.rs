// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{PhysicalSourceRef, SourcedValue};

use super::{BlueprintPoolDefinition, BlueprintPoolEntry, BlueprintPoolRef, BlueprintPoolWeights};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

impl<'source> BlueprintPoolDefinition<'source> {
    /// Borrows the original independently sourced physical pool header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }
    /// Borrows all original ordered reward slots and their retained states.
    #[must_use]
    pub const fn blueprint_rewards(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<BlueprintPoolEntry<'source>>>> {
        &self.blueprint_rewards
    }
    /// Borrows unprojected and rejected original pool members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}

impl<'source> BlueprintPoolEntry<'source> {
    /// Borrows the original concrete entry type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }
    /// Borrows the original Blueprint link, including unresolved target states.
    #[must_use]
    pub const fn blueprint_record(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.blueprint_record
    }
    /// Borrows the original numeric weight without narrowing its storage.
    #[must_use]
    pub const fn weight(&self) -> &SourcedValue<ResourceNumber> {
        &self.weight
    }
    /// Borrows unprojected and rejected original entry members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}

impl BlueprintPoolRef {
    /// Borrows the original DataForge physical source identity.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }
    /// Borrows the pool's physical Record index in that DataForge.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        &self.record_index
    }
    /// Borrows the nominal pool GUID from its original header.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }
}

impl BlueprintPoolWeights {
    /// Borrows the physical original definition identity.
    #[must_use]
    pub const fn pool(&self) -> &BlueprintPoolRef {
        &self.pool
    }
    /// Borrows the derived sum with its actual source contributions and completeness.
    #[must_use]
    pub const fn total_weight(&self) -> &SourcedValue<ResourceFloat32> {
        &self.total_weight
    }
}
