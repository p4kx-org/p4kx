// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, FieldOrigin, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::schema_fields::every_field_required;

/// An authored launcher selection tied to one original weapon action occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponProjectileAmmoSelection {
    #[serde(flatten)]
    owner: ComponentOwner,
    action_source_value_ordinal: SourcedValue<DecimalU64>,
    selection: SourcedValue<Selection>,
}

/// The selected physical container and its authored ammunition role.
pub(in crate::catalog_v3) struct SelectedAmmoRoute<'row> {
    pub(in crate::catalog_v3) role: &'row SourcedValue<ProjectileAmmoRole>,
    pub(in crate::catalog_v3) container_record_index: Option<&'row SourcedValue<u32>>,
    pub(in crate::catalog_v3) container_component_ordinal: &'row SourcedValue<u32>,
    pub(in crate::catalog_v3) ammo_record_index: &'row SourcedValue<u32>,
}

impl WeaponProjectileAmmoSelection {
    pub(super) const fn new(
        owner: ComponentOwner,
        action_source_value_ordinal: SourcedValue<DecimalU64>,
        selection: SourcedValue<Selection>,
    ) -> Self {
        Self {
            owner,
            action_source_value_ordinal,
            selection,
        }
    }

    pub(in crate::catalog_v3) const fn selected_route(&self) -> Option<SelectedAmmoRoute<'_>> {
        match self.selection.value() {
            Some(Selection::Selected {
                role,
                container_component_ordinal,
                ammo_record_index,
            }) => Some(SelectedAmmoRoute {
                role,
                container_record_index: None,
                container_component_ordinal,
                ammo_record_index,
            }),
            Some(Selection::SelectedLinkedMagazine {
                role,
                magazine_record_index,
                container_component_ordinal,
                ammo_record_index,
            }) => Some(SelectedAmmoRoute {
                role,
                container_record_index: Some(magazine_record_index),
                container_component_ordinal,
                ammo_record_index,
            }),
            _ => None,
        }
    }

    pub(in crate::catalog_v3) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    pub(in crate::catalog_v3) const fn action_source_value_ordinal(
        &self,
    ) -> &SourcedValue<DecimalU64> {
        &self.action_source_value_ordinal
    }

    pub(in crate::catalog_v3) const fn origin(&self) -> &FieldOrigin {
        self.selection.origin()
    }

    pub(in crate::catalog_v3) const fn selected_ammo_index(&self) -> Option<&SourcedValue<u32>> {
        match self.selected_route() {
            Some(route) => Some(route.ammo_record_index),
            None => None,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum Selection {
    Selected {
        role: SourcedValue<ProjectileAmmoRole>,
        container_component_ordinal: SourcedValue<u32>,
        ammo_record_index: SourcedValue<u32>,
    },
    SelectedLinkedMagazine {
        role: SourcedValue<ProjectileAmmoRole>,
        magazine_record_index: SourcedValue<u32>,
        container_component_ordinal: SourcedValue<u32>,
        ammo_record_index: SourcedValue<u32>,
    },
    DanglingMagazine,
    UnsupportedMagazineTarget,
    SelfLinkedMagazine,
    AmbiguousMagazineReference,
    UnreadableMagazine,
    LinkedMagazineWithoutContainer,
    AmbiguousLinkedMagazineContainer,
    IncompleteLinkedMagazineComponents,
    AmbiguousAction,
    IncompleteActions,
    AmbiguousComponents,
    IncompleteComponents,
    NoAdmittedOwnContainer,
    OtherMagazineRoute,
    IndeterminateMagazineRoute,
    UnsupportedProjectileType,
    MissingProjectileType,
    IndeterminateProjectileType,
    MissingAmmo,
    DanglingAmmo,
    UnsupportedAmmoTarget,
    IndeterminateAmmo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(in crate::catalog_v3) enum ProjectileAmmoRole {
    Primary,
    Secondary,
}

impl ProjectileAmmoRole {
    pub(super) const fn field_name(self) -> &'static str {
        match self {
            Self::Primary => "ammoParamsRecord",
            Self::Secondary => "secondaryAmmoParamsRecord",
        }
    }
}
