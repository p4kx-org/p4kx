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
pub struct WeaponProjectileAmmoSelection {
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

    /// Borrows the original weapon owner and its physical component position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original action occurrence ordinal in the owner Record.
    #[must_use]
    pub const fn action_source_value_ordinal(&self) -> &SourcedValue<DecimalU64> {
        &self.action_source_value_ordinal
    }

    /// Borrows the resolved or rejected route with its original selection evidence.
    #[must_use]
    pub const fn selection(&self) -> &SourcedValue<Selection> {
        &self.selection
    }

    pub(in crate::catalog_v3) const fn origin(&self) -> &FieldOrigin {
        self.selection.origin()
    }

    pub(crate) const fn selected_ammo_index(&self) -> Option<&SourcedValue<u32>> {
        match self.selected_route() {
            Some(route) => Some(route.ammo_record_index),
            None => None,
        }
    }
}

/// The outcome of resolving the original ammunition route of a launcher action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum Selection {
    /// The launcher selects ammunition from a container on its own Item.
    Selected {
        /// The launcher role that selects the slot.
        role: SourcedValue<ProjectileAmmoRole>,
        /// The original container position in the weapon's Components array.
        container_component_ordinal: SourcedValue<u32>,
        /// The admitted physical Ammo Record index.
        ammo_record_index: SourcedValue<u32>,
    },
    /// The launcher selects ammunition from a uniquely linked magazine.
    SelectedLinkedMagazine {
        /// The launcher role that selects the slot.
        role: SourcedValue<ProjectileAmmoRole>,
        /// The uniquely admitted magazine's physical Record index.
        magazine_record_index: SourcedValue<u32>,
        /// The original container position in the magazine's Components array.
        container_component_ordinal: SourcedValue<u32>,
        /// The admitted physical Ammo Record index.
        ammo_record_index: SourcedValue<u32>,
    },
    /// The original magazine reference has no target.
    DanglingMagazine,
    /// The original magazine reference targets a different source type.
    UnsupportedMagazineTarget,
    /// The magazine reference resolves to the owner Item.
    SelfLinkedMagazine,
    /// Multiple physical Records match the magazine reference.
    AmbiguousMagazineReference,
    /// The admitted magazine body could not be read.
    UnreadableMagazine,
    /// The admitted magazine has no supported container.
    LinkedMagazineWithoutContainer,
    /// The admitted magazine has competing container components.
    AmbiguousLinkedMagazineContainer,
    /// The magazine's component inventory could not be determined.
    IncompleteLinkedMagazineComponents,
    /// The same original action occurrence appears more than once.
    AmbiguousAction,
    /// The original fire-action inventory could not be determined.
    IncompleteActions,
    /// The weapon owner has competing weapon or container components.
    AmbiguousComponents,
    /// The weapon owner's component inventory could not be determined.
    IncompleteComponents,
    /// The owner's direct container and linked magazine are both absent.
    NoAdmittedOwnContainer,
    /// The configured magazine route requires other selection inputs.
    OtherMagazineRoute,
    /// An unusable magazine route field prevents selecting a slot.
    IndeterminateMagazineRoute,
    /// The launcher type is outside the supported primary and secondary roles.
    UnsupportedProjectileType,
    /// The original launcher does not name a projectile role.
    MissingProjectileType,
    /// The original projectile role could not be determined.
    IndeterminateProjectileType,
    /// The selected slot has no Ammo reference.
    MissingAmmo,
    /// The selected Ammo reference has no target.
    DanglingAmmo,
    /// The selected Ammo reference targets a different source type.
    UnsupportedAmmoTarget,
    /// The Ammo reference or its physical target cannot be determined.
    IndeterminateAmmo,
}

/// The launcher role used to select an original ammunition slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum ProjectileAmmoRole {
    /// The authored primary ammunition slot.
    Primary,
    /// The authored secondary ammunition slot.
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
