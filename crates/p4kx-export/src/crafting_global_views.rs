// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::{CraftingBlueprintSelection, CraftingGlobalConfiguration};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

impl<'source> CraftingGlobalConfiguration<'source> {
    /// Borrows the original independently sourced physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the original default-blueprint selection and its retained field state.
    #[must_use]
    pub const fn default_blueprint_selection(
        &self,
    ) -> &SourcedValue<CraftingBlueprintSelection<'source>> {
        &self.default_blueprint_selection
    }

    /// Borrows all original resource-exclusion slots in authored order.
    #[must_use]
    pub const fn dismantle_blacklist_resources(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.dismantle_blacklist_resources
    }

    /// Borrows all original item-exclusion slots in authored order.
    #[must_use]
    pub const fn dismantle_blacklist_entity_classes(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.dismantle_blacklist_entity_classes
    }

    /// Borrows additional fields and rejected original storage in this owner's arena.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}

impl<'source> CraftingBlueprintSelection<'source> {
    /// Borrows the original concrete selection type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows all original blueprint-reference slots in authored order.
    #[must_use]
    pub const fn blueprint_records(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.blueprint_records
    }

    /// Borrows additional selection fields and rejected original storage.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}
