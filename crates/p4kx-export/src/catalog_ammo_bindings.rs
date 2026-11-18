// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ammo_definition;
use super::authored_fields::Observed;
use super::catalog_ammo_containers;
use super::catalog_weapon_magazine_slots;
pub use super::catalog_weapon_magazine_slots::{SecondaryMagazine, WeaponMagazineSlots};
use super::catalog_weapon_projectile_ammo::ProjectileAmmoRole;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// The original ammunition links authored on a physical Item component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum AmmoBinding<'source> {
    /// Ammunition References on a physical container.
    ContainerSlots {
        /// Original container slots.
        value: Box<ContainerAmmoSlots<'source>>,
    },
    /// Magazine References on a physical weapon.
    WeaponMagazineSlots {
        /// Original weapon slots and conditions.
        value: Box<WeaponMagazineSlots<'source>>,
    },
}

impl<'source> AmmoBinding<'source> {
    pub(super) fn container(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self::ContainerSlots {
            value: Box::new(ContainerAmmoSlots::read(owner, source, notes)?),
        })
    }

    /// Borrows the physical source component of this binding.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        match self {
            Self::ContainerSlots { value } => &value.owner,
            Self::WeaponMagazineSlots { value } => value.owner(),
        }
    }
}

/// The two ordered source roles owned by one container.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContainerAmmoSlots<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    primary_ammo: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    secondary_ammo: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> ContainerAmmoSlots<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            primary_ammo: source.record_link_to(
                "ammoParamsRecord",
                ammo_definition::SOURCE_TYPE,
                notes,
            )?,
            secondary_ammo: source.record_link_to(
                "secondaryAmmoParamsRecord",
                ammo_definition::SOURCE_TYPE,
                notes,
            )?,
        })
    }

    /// Borrows the physical source component for these slots.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original primary ammunition Reference and its state.
    #[must_use]
    pub const fn primary_ammo(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.primary_ammo
    }

    /// Borrows the original secondary ammunition Reference and its state.
    #[must_use]
    pub const fn secondary_ammo(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.secondary_ammo
    }

    pub(super) const fn slot(
        &self,
        role: ProjectileAmmoRole,
    ) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        match role {
            ProjectileAmmoRole::Primary => &self.primary_ammo,
            ProjectileAmmoRole::Secondary => &self.secondary_ammo,
        }
    }
}

pub(super) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<AmmoBinding<'source>>,
) -> CatalogTable<AmmoBinding<'source>> {
    rows.sort_by_key(|row| row.owner().key());
    CatalogTable::new(
        forge,
        &[
            catalog_ammo_containers::SOURCE_TYPE,
            catalog_weapon_magazine_slots::SOURCE_TYPE,
        ],
        rows,
    )
}
