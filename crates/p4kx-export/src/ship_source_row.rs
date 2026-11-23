// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projections::ship_base_facts::ShipBaseFacts;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::catalog_v3::authored_fields::ship_presentation::{
    ShipAttachmentManufacturer, ShipVehicleManufacturer,
};
use crate::catalog_v3::authored_fields::ship_source_read::{ShipPlayabilityInputs, ShipSourceRead};
use crate::contract_insurance::ShipInsurance;
use crate::loadout::ledger::ShipAggregateValues;
use crate::resource_number::ResourceNumber;
use crate::ship_base::ShipCargoSource;
use crate::{MassBasis, Playability, VehicleKind};

/// Canonical source-backed fields of one physical Ship, including resolved mass and hull.
#[derive(Debug)]
pub struct ShipSource<'source> {
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) role: SourcedValue<PublicNameRef>,
    pub(crate) career: SourcedValue<PublicNameRef>,
    pub(crate) crew_size: SourcedValue<ResourceNumber>,
    pub(crate) vehicle_manufacturer: ShipVehicleManufacturer<'source>,
    pub(crate) attachment_manufacturer: ShipAttachmentManufacturer<'source>,
    pub(crate) vehicle_definition: SourcedValue<&'source str>,
    pub(crate) vehicle_kind: SourcedValue<VehicleKind>,
    pub(crate) size: SourcedValue<ResourceNumber>,
    pub(crate) mass: SourcedValue<f64>,
    pub(crate) mass_basis: SourcedValue<MassBasis>,
    pub(crate) hull_health: SourcedValue<f64>,
    pub(crate) base_facts: ShipBaseFacts,
    pub(crate) insurance: SourcedValue<Box<ShipInsurance<'source>>>,
    pub(crate) playability_inputs: ShipPlayabilityInputs<'source>,
    pub(crate) playability: SourcedValue<Playability>,
}

impl<'source> ShipSource<'source> {
    pub(crate) fn from_read(
        source: ShipSourceRead<'_, 'source>,
        aggregates: ShipAggregateValues,
        playability: SourcedValue<Playability>,
    ) -> Self {
        let presentation = source.presentation;
        let base_facts = ShipBaseFacts::new(&source.header, aggregates.cargo);
        let vehicle_kind = source
            .playability_proof
            .exosuit_kind()
            .unwrap_or(aggregates.vehicle_kind);
        Self {
            header: source.header,
            name: presentation.name,
            description: presentation.description,
            role: presentation.role,
            career: presentation.career,
            crew_size: source.crew_size,
            vehicle_manufacturer: presentation.vehicle_manufacturer,
            attachment_manufacturer: presentation.attachment_manufacturer,
            vehicle_definition: source.vehicle_definition,
            vehicle_kind,
            size: presentation.size,
            mass: aggregates.mass,
            mass_basis: aggregates.mass_basis,
            hull_health: aggregates.hull_health,
            base_facts,
            insurance: source.insurance,
            playability_inputs: source.playability_inputs,
            playability,
        }
    }

    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// Borrows the original vehicle-name reference.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<PublicNameRef> {
        &self.name
    }

    /// Borrows the protected vehicle-description reference.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }

    /// Borrows the authored vehicle-role reference.
    #[must_use]
    pub const fn role(&self) -> &SourcedValue<PublicNameRef> {
        &self.role
    }

    /// Borrows the authored career reference.
    #[must_use]
    pub const fn career(&self) -> &SourcedValue<PublicNameRef> {
        &self.career
    }

    /// Borrows the original exact crew size.
    #[must_use]
    pub const fn crew_size(&self) -> &SourcedValue<ResourceNumber> {
        &self.crew_size
    }

    /// Borrows the selected vehicle manufacturer.
    #[must_use]
    pub const fn vehicle_manufacturer(&self) -> &ShipVehicleManufacturer<'source> {
        &self.vehicle_manufacturer
    }

    /// Borrows the independent attachment manufacturer.
    #[must_use]
    pub const fn attachment_manufacturer(&self) -> &ShipAttachmentManufacturer<'source> {
        &self.attachment_manufacturer
    }

    /// Borrows the unnormalized authored vehicle selector.
    #[must_use]
    pub const fn vehicle_definition(&self) -> &SourcedValue<&'source str> {
        &self.vehicle_definition
    }

    /// Borrows the vehicle kind with its original classification evidence.
    #[must_use]
    pub const fn vehicle_kind(&self) -> &SourcedValue<VehicleKind> {
        &self.vehicle_kind
    }

    /// Borrows the original exact attachment size.
    #[must_use]
    pub const fn size(&self) -> &SourcedValue<ResourceNumber> {
        &self.size
    }

    /// Borrows the canonical hull mass in kilograms.
    #[must_use]
    pub const fn mass(&self) -> &SourcedValue<f64> {
        &self.mass
    }

    /// Borrows the original mass-selection basis.
    #[must_use]
    pub const fn mass_basis(&self) -> &SourcedValue<MassBasis> {
        &self.mass_basis
    }

    /// Borrows the canonical structural hull health.
    #[must_use]
    pub const fn hull_health(&self) -> &SourcedValue<f64> {
        &self.hull_health
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
        &self.insurance
    }

    /// Borrows the canonical classification and its immediate source evidence.
    #[must_use]
    pub const fn playability(&self) -> &SourcedValue<Playability> {
        &self.playability
    }

    /// Borrows the authored crew-manifest inputs.
    #[must_use]
    pub const fn playability_inputs(&self) -> &ShipPlayabilityInputs<'source> {
        &self.playability_inputs
    }
}
