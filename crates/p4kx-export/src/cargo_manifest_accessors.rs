// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::{
    CargoFillCapacity, CargoManifestDefinition, CargoQuantityAlgorithm, CargoResource,
    CargoResourceSlot, CargoSpawnRule,
};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

impl<'source> CargoManifestDefinition<'source> {
    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the authored lock flag.
    #[must_use]
    pub const fn lock_cargo_grids_on_spawn(&self) -> &SourcedValue<bool> {
        &self.lock_cargo_grids_on_spawn
    }

    /// Borrows the original cargo fill definition.
    #[must_use]
    pub const fn cargo_fill_capacity(&self) -> &SourcedValue<CargoFillCapacity<'source>> {
        &self.cargo_fill_capacity
    }

    /// Borrows the original ordered spawn rules.
    #[must_use]
    pub const fn cargo_spawn_rules(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<CargoSpawnRule<'source>>>> {
        &self.cargo_spawn_rules
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> CargoFillCapacity<'source> {
    /// Borrows the original concrete source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the lower fill fraction.
    #[must_use]
    pub const fn min_capacity_range(&self) -> &SourcedValue<ResourceNumber> {
        &self.min_capacity_range
    }

    /// Borrows the upper fill fraction.
    #[must_use]
    pub const fn max_capacity_range(&self) -> &SourcedValue<ResourceNumber> {
        &self.max_capacity_range
    }

    /// Borrows the resource and allocation occurrences in source order.
    #[must_use]
    pub const fn resources(&self) -> &SourcedValue<Vec<SourcedValue<CargoResourceSlot<'source>>>> {
        &self.resources
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> CargoResourceSlot<'source> {
    /// Borrows the original concrete source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the direct resource reference when declared.
    #[must_use]
    pub const fn resource(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.resource
    }

    /// Borrows the direct resource probability when declared.
    #[must_use]
    pub const fn probability(&self) -> &SourcedValue<ResourceNumber> {
        &self.probability
    }

    /// Borrows the lower allocation bound.
    #[must_use]
    pub const fn min_resource_allocation_percentage(&self) -> &SourcedValue<ResourceNumber> {
        &self.min_resource_allocation_percentage
    }

    /// Borrows the upper allocation bound.
    #[must_use]
    pub const fn max_resource_allocation_percentage(&self) -> &SourcedValue<ResourceNumber> {
        &self.max_resource_allocation_percentage
    }

    /// Borrows the resource owned by the allocation.
    #[must_use]
    pub const fn resources(&self) -> &SourcedValue<CargoResource<'source>> {
        &self.resources
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> CargoResource<'source> {
    /// Borrows the original concrete source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored resource reference.
    #[must_use]
    pub const fn resource(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.resource
    }

    /// Borrows the authored resource probability.
    #[must_use]
    pub const fn probability(&self) -> &SourcedValue<ResourceNumber> {
        &self.probability
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> CargoSpawnRule<'source> {
    /// Borrows the original concrete source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored spawn probability.
    #[must_use]
    pub const fn probability(&self) -> &SourcedValue<ResourceNumber> {
        &self.probability
    }

    /// Borrows the ordered entity references.
    #[must_use]
    pub const fn entities(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.entities
    }

    /// Borrows the ordered resource references.
    #[must_use]
    pub const fn resources(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.resources
    }

    /// Borrows the original quantity algorithm selection.
    #[must_use]
    pub const fn quantity_algorithm_type(&self) -> &SourcedValue<CargoQuantityAlgorithm<'source>> {
        &self.quantity_algorithm_type
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> CargoQuantityAlgorithm<'source> {
    /// Borrows the original concrete source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the original fixed integer quantity.
    #[must_use]
    pub const fn quantity(&self) -> &SourcedValue<ResourceNumber> {
        &self.quantity
    }

    /// Borrows the original proportional quantity.
    #[must_use]
    pub const fn proportion(&self) -> &SourcedValue<ResourceNumber> {
        &self.proportion
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}
