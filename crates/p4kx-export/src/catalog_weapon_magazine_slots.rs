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
const MAGAZINE_TARGET: &str = "EntityClassDefinition";
const SECONDARY_SLOT: &str = "SSecondaryAmmoContainerParams";
const CONDITION_BASE: &str = "SWeaponConditionBase";

/// The authored primary and ordered secondary magazine slots of one physical weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponMagazineSlots<'source> {
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

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    pub(super) const fn route_inputs(&self) -> MagazineRouteInputs<'_, 'source> {
        MagazineRouteInputs { slots: self }
    }
}

pub(super) struct MagazineRouteInputs<'rows, 'source> {
    slots: &'rows WeaponMagazineSlots<'source>,
}

impl<'rows> MagazineRouteInputs<'rows, '_> {
    pub(super) const fn origins(&self) -> [&'rows FieldOrigin; 3] {
        [
            self.slots.ammo_container_record.origin(),
            self.slots.secondary_ammo_containers.origin(),
            self.slots.should_ignore_primary_ammo_container.origin(),
        ]
    }

    pub(super) fn other_route(&self) -> bool {
        self.slots.ammo_container_record.value().is_some()
            || self
                .slots
                .secondary_ammo_containers
                .value()
                .is_some_and(|values| !values.is_empty())
            || self.slots.should_ignore_primary_ammo_container.value() == Some(&true)
    }

    pub(super) fn permits_own_route(&self) -> bool {
        matches!(
            self.slots.ammo_container_record.state(),
            SourceValueState::Absent | SourceValueState::Null
        ) && (matches!(
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
struct SecondaryMagazine<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    ammo_container_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    condition: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> SecondaryMagazine<'source> {
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
