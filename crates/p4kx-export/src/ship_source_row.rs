// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRef, SourcedValue};

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projections::ship_base_facts::ShipBaseFacts;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::catalog_v3::authored_fields::ship_presentation::{
    ShipAttachmentManufacturer, ShipVehicleManufacturer,
};
use crate::catalog_v3::authored_fields::ship_source_read::{ShipPlayabilityInputs, ShipSourceRead};
use crate::contract::Ship;
use crate::contract_insurance::ShipInsurance;
use crate::loadout::ledger::ShipAggregateValues;
use crate::resource_number::ResourceNumber;
use crate::ship_base::ShipCargoSource;
use crate::variant_decision::VariantDecision;
use crate::variant_target::VariantTarget;
use crate::{CatalogError, MassBasis, Playability, VehicleKind};

/// Canonical source-backed fields of one physical Ship, including resolved mass and hull.
#[derive(Debug)]
pub struct ShipSource<'source> {
    ship: Ship<'source>,
    base_facts: ShipBaseFacts,
    has_variant: bool,
}

impl<'source> ShipSource<'source> {
    pub(crate) fn from_read(
        mut source: ShipSourceRead<'_, 'source>,
        aggregates: ShipAggregateValues,
        playability: SourcedValue<Playability>,
    ) -> Result<Self, CatalogError> {
        let base_facts = ShipBaseFacts::new(&source.header, aggregates.cargo);
        source.numbers = Vec::new();
        Ok(Self {
            ship: Ship::from_read(source, aggregates.vehicle, playability)?,
            base_facts,
            has_variant: false,
        })
    }

    #[cfg(test)]
    pub(crate) const fn direct_row(&self) -> &Ship<'source> {
        &self.ship
    }

    pub(crate) fn set_variant(&mut self, decision: VariantDecision) {
        self.ship.variant_of = decision;
        self.has_variant = true;
    }

    /// Borrows the serializable Catalog row when this request compared Ship variants.
    #[must_use]
    pub const fn catalog_row(&self) -> Option<&Ship<'source>> {
        if self.has_variant {
            Some(&self.ship)
        } else {
            None
        }
    }

    pub(crate) const fn variant_decision(&self) -> &VariantDecision {
        &self.ship.variant_of
    }

    /// Borrows the physical variant decision when this is a canonical Ship query.
    #[must_use]
    pub fn variant_of(&self) -> Option<&SourcedValue<VariantTarget>> {
        self.has_variant.then(|| self.ship.variant_of.expanded())
    }

    /// Returns the provenance set referenced by the canonical variant decision.
    #[must_use]
    pub fn variant_source_use_set_ref(&self) -> Option<u32> {
        self.ship.variant_of.source_use_set_ref()
    }

    /// Borrows failed operations of the canonical variant decision apart from contributors.
    #[must_use]
    pub fn variant_failed_source_uses(&self) -> &[SourceUseRef] {
        self.ship.variant_of.failed_source_uses()
    }

    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.ship.header
    }

    /// Borrows the original vehicle-name reference.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<PublicNameRef> {
        &self.ship.name
    }

    /// Borrows the protected vehicle-description reference.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.ship.description
    }

    /// Borrows the authored vehicle-role reference.
    #[must_use]
    pub const fn role(&self) -> &SourcedValue<PublicNameRef> {
        &self.ship.role
    }

    /// Borrows the authored career reference.
    #[must_use]
    pub const fn career(&self) -> &SourcedValue<PublicNameRef> {
        &self.ship.career
    }

    /// Borrows the original exact crew size.
    #[must_use]
    pub const fn crew_size(&self) -> &SourcedValue<ResourceNumber> {
        &self.ship.crew_size
    }

    /// Borrows the selected vehicle manufacturer.
    #[must_use]
    pub const fn vehicle_manufacturer(&self) -> &ShipVehicleManufacturer<'source> {
        &self.ship.vehicle_manufacturer
    }

    /// Borrows the independent attachment manufacturer.
    #[must_use]
    pub const fn attachment_manufacturer(&self) -> &ShipAttachmentManufacturer<'source> {
        &self.ship.attachment_manufacturer
    }

    /// Borrows the unnormalized authored vehicle selector.
    #[must_use]
    pub const fn vehicle_definition(&self) -> &SourcedValue<&'source str> {
        &self.ship.vehicle_definition
    }

    /// Borrows the vehicle kind with its original path and rolling-part decision.
    #[must_use]
    pub const fn vehicle_kind(&self) -> &SourcedValue<VehicleKind> {
        &self.ship.vehicle_kind
    }

    /// Borrows the original exact attachment size.
    #[must_use]
    pub const fn size(&self) -> &SourcedValue<ResourceNumber> {
        &self.ship.size
    }

    /// Borrows the canonical hull mass in kilograms.
    #[must_use]
    pub const fn mass(&self) -> &SourcedValue<f64> {
        &self.ship.mass
    }

    /// Borrows the original mass-selection basis.
    #[must_use]
    pub const fn mass_basis(&self) -> &SourcedValue<MassBasis> {
        &self.ship.mass_basis
    }

    /// Borrows the canonical structural hull health.
    #[must_use]
    pub const fn hull_health(&self) -> &SourcedValue<f64> {
        &self.ship.hull_health
    }

    /// Borrows the canonical cargo projection and its positive mount evidence.
    #[must_use]
    pub const fn cargo(&self) -> &ShipCargoSource {
        self.base_facts.cargo()
    }

    /// Borrows the canonical Ship base facts projection and its sourced cargo operands.
    #[must_use]
    pub const fn base_facts(&self) -> &ShipBaseFacts {
        &self.base_facts
    }

    /// Borrows the original insurance declaration and parameter states.
    #[must_use]
    pub const fn insurance(&self) -> &SourcedValue<Box<ShipInsurance<'source>>> {
        &self.ship.insurance
    }

    /// Borrows the canonical classification and its immediate source evidence.
    #[must_use]
    pub const fn playability(&self) -> &SourcedValue<Playability> {
        &self.ship.playability
    }

    /// Borrows the authored crew-manifest inputs.
    #[must_use]
    pub const fn playability_inputs(&self) -> &ShipPlayabilityInputs<'source> {
        &self.ship.playability_inputs
    }
}
