// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::authored_nodes::AuthoredNodes;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::item_stats::WEAPON_COMPONENT;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[path = "weapon_action.rs"]
mod action;
#[path = "weapon_operating_configuration.rs"]
mod operating;
#[path = "weapon_action_recoil.rs"]
mod recoil;

pub(super) use action::{ProjectileAction, ProjectileSelector};
pub use action::{ProjectileCombatInputs, WeaponAction, WeaponModeKind};
use operating::WeaponOperatingConfiguration;

const ACTION_BASE: &str = "SWeaponActionParams";

/// Authored fire-action slots and local action nodes of one physical weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct WeaponActions<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    fire_actions: SourcedValue<Vec<SourcedValue<u32>>>,
    aim_action: SourcedValue<u32>,
    action_nodes: Vec<WeaponAction<'source>>,
    operating_configuration: WeaponOperatingConfiguration<'source>,
}

impl<'source> WeaponActions<'source> {
    /// Borrows the original Item and Components slot identity.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original ordered fire slots and their action-node indices.
    #[must_use]
    pub const fn fire_actions(&self) -> &SourcedValue<Vec<SourcedValue<u32>>> {
        &self.fire_actions
    }

    pub(super) fn action(&self, index: u32) -> Option<ProjectileAction<'_, 'source>> {
        let index = usize::try_from(index).ok()?;
        self.action_nodes.get(index).map(WeaponAction::selection)
    }

    /// Borrows the original aim slot, indexed into the local action nodes.
    #[must_use]
    pub const fn aim_action(&self) -> &SourcedValue<u32> {
        &self.aim_action
    }

    /// Borrows the local action nodes in the order addressed by the authored slots.
    #[must_use]
    pub fn action_nodes(&self) -> &[WeaponAction<'source>] {
        &self.action_nodes
    }

    /// Borrows the action node at an index retained by an authored action slot.
    #[must_use]
    pub fn mode(&self, node: u32) -> Option<&WeaponAction<'source>> {
        self.action_nodes.get(usize::try_from(node).ok()?)
    }

    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut pending = AuthoredNodes::default();
        let fire_actions = read_children(source, "fireActions", &mut pending, notes)?;
        let aim_action = read_child(source, "aimAction", &mut pending, notes)?;
        let action_nodes = pending.read(notes, WeaponAction::read)?;
        Ok(Self {
            owner,
            fire_actions,
            aim_action,
            action_nodes,
            operating_configuration: WeaponOperatingConfiguration::read(source, notes)?,
        })
    }

    pub(super) fn animation_index(&self) -> Option<u32> {
        self.operating_configuration.animation_index()
    }

    pub(super) fn actor_recoil_index(&self) -> Option<u32> {
        self.operating_configuration.actor_recoil_index()
    }

    pub(super) fn linked_recoil_definitions(&self) -> impl Iterator<Item = u32> + '_ {
        self.action_nodes
            .iter()
            .flat_map(WeaponAction::linked_recoil_definitions)
    }
}

fn read_child<'reader, 'scope, 'read, 'source>(
    source: Observed<'reader, 'scope, 'read, 'source>,
    name: &str,
    pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<u32>, CatalogError> {
    source.strong_pointer_family_to(name, ACTION_BASE, notes, |child, _| pending.push(child))
}

fn read_children<'reader, 'scope, 'read, 'source>(
    source: Observed<'reader, 'scope, 'read, 'source>,
    name: &str,
    pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<u32>>>, CatalogError> {
    source.strong_pointer_family_array_to(name, ACTION_BASE, notes, |child, _| pending.push(child))
}

pub(super) const RECOIL_DEFINITION: &str = recoil::CONFIG_TYPE;

pub(super) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<WeaponActions<'source>>,
) -> CatalogTable<WeaponActions<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[WEAPON_COMPONENT], rows)
}
