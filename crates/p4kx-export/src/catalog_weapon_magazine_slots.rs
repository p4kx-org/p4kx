// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::item_stats::PRIMARY_MAGAZINE_FIELD;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) use crate::item_stats::WEAPON_COMPONENT as SOURCE_TYPE;
pub(super) const MAGAZINE_TARGET: &str = "EntityClassDefinition";
const SECONDARY_SLOT: &str = "SSecondaryAmmoContainerParams";
const CONDITION_BASE: &str = "SWeaponConditionBase";

/// The authored primary and ordered secondary magazine slots of one physical weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct WeaponMagazineSlots<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    ammo_container_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    secondary_ammo_containers: SourcedValue<Vec<SourcedValue<SecondaryMagazine<'source>>>>,
    should_ignore_primary_ammo_container: SourcedValue<bool>,
}

impl<'source> WeaponMagazineSlots<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            ammo_container_record: source.record_link_to(
                PRIMARY_MAGAZINE_FIELD,
                MAGAZINE_TARGET,
                notes,
            )?,
            secondary_ammo_containers: source.class_array_to(
                "secondaryAmmoContainers",
                SECONDARY_SLOT,
                notes,
                SecondaryMagazine::read,
            )?,
            should_ignore_primary_ammo_container: source
                .boolean("ShouldIgnorePrimaryAmmoContainer", notes)?,
        })
    }

    /// Borrows the original weapon component that owns these slots.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the authored primary magazine Reference.
    #[must_use]
    pub const fn primary_magazine(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.ammo_container_record
    }

    /// Borrows the authored secondary magazine slots in stored order.
    #[must_use]
    pub const fn secondary_magazines(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<SecondaryMagazine<'source>>>> {
        &self.secondary_ammo_containers
    }

    /// Borrows the original primary-magazine ignore flag.
    #[must_use]
    pub const fn ignore_primary_magazine(&self) -> &SourcedValue<bool> {
        &self.should_ignore_primary_ammo_container
    }

    pub(super) const fn route_inputs(&self) -> MagazineRouteInputs<'_, 'source> {
        MagazineRouteInputs { slots: self }
    }
}

pub(super) struct MagazineRouteInputs<'rows, 'source> {
    slots: &'rows WeaponMagazineSlots<'source>,
}

impl<'rows, 'source> MagazineRouteInputs<'rows, 'source> {
    pub(super) const fn origins(&self) -> [&'rows FieldOrigin; 3] {
        [
            self.slots.ammo_container_record.origin(),
            self.slots.secondary_ammo_containers.origin(),
            self.slots.should_ignore_primary_ammo_container.origin(),
        ]
    }

    pub(super) const fn primary(&self) -> &'rows SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.slots.ammo_container_record
    }

    pub(super) fn has_other_linked_route(&self) -> bool {
        self.slots
            .secondary_ammo_containers
            .value()
            .is_some_and(|values| !values.is_empty())
            || self.slots.should_ignore_primary_ammo_container.value() == Some(&true)
    }

    pub(super) fn other_route(&self) -> bool {
        self.primary().value().is_some() || self.has_other_linked_route()
    }

    pub(super) fn permits_own_route(&self) -> bool {
        matches!(
            self.slots.ammo_container_record.state(),
            SourceValueState::Absent | SourceValueState::Null
        ) && self.permits_linked_route()
    }

    pub(super) fn permits_linked_route(&self) -> bool {
        (matches!(
            self.slots.secondary_ammo_containers.state(),
            SourceValueState::Absent | SourceValueState::Null
        ) || self
            .slots
            .secondary_ammo_containers
            .value()
            .is_some_and(Vec::is_empty))
            && self.slots.should_ignore_primary_ammo_container.value() == Some(&false)
    }
}

/// One original secondary Class element, including its unevaluated condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SecondaryMagazine<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    ammo_container_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    condition: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> SecondaryMagazine<'source> {
    /// Borrows the original secondary magazine Reference.
    #[must_use]
    pub const fn magazine(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.ammo_container_record
    }

    /// Borrows the original unevaluated condition and its owned source values.
    #[must_use]
    pub const fn condition(&self) -> &SourcedValue<SourceArenaOutput<'source>> {
        &self.condition
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ammo_container_record: source.record_link_to(
                PRIMARY_MAGAZINE_FIELD,
                MAGAZINE_TARGET,
                notes,
            )?,
            condition: source.raw_strong_pointer_to("condition", CONDITION_BASE, notes)?,
        })
    }
}
