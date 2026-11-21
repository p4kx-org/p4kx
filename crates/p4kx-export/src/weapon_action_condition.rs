// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

use super::super::read_child;

/// Original conditional slots and the separately authored default action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ConditionalAction<'source> {
    conditional_weapon_actions: SourcedValue<Vec<SourcedValue<ConditionalActionEntry<'source>>>>,
    default_weapon_action: SourcedValue<u32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ConditionalAction<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        recoil: [(&'static str, SourceValueState); 3],
    ) -> Result<Self, CatalogError> {
        let conditional_weapon_actions = source.class_array_to(
            "conditionalWeaponActions",
            "SConditionalWeaponAction",
            notes,
            |entry, notes| ConditionalActionEntry::read(entry, pending, notes),
        )?;
        let default_weapon_action = read_child(source, "defaultWeaponAction", pending, notes)?;
        let authored = source.authored_members_except(
            [
                (
                    "conditionalWeaponActions",
                    conditional_weapon_actions.state(),
                ),
                ("defaultWeaponAction", default_weapon_action.state()),
            ]
            .into_iter()
            .chain(recoil),
        )?;
        Ok(Self {
            conditional_weapon_actions,
            default_weapon_action,
            authored,
        })
    }
}

/// An original unevaluated condition and its action slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ConditionalActionEntry<'source> {
    condition: SourcedValue<SourceArenaOutput<'source>>,
    weapon_action: SourcedValue<u32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ConditionalActionEntry<'source> {
    fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let condition = source.raw_strong_pointer_to("condition", "SWeaponConditionBase", notes)?;
        let weapon_action = read_child(source, "weaponAction", pending, notes)?;
        let authored = source.authored_members_except([
            ("condition", condition.state()),
            ("weaponAction", weapon_action.state()),
        ])?;
        Ok(Self {
            condition,
            weapon_action,
            authored,
        })
    }
}

/// The child of an original toggle action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ToggleAction<'source> {
    weapon_action: SourcedValue<u32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ToggleAction<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        recoil: [(&'static str, SourceValueState); 3],
    ) -> Result<Self, CatalogError> {
        let weapon_action = read_child(source, "weaponAction", pending, notes)?;
        let authored = source.authored_members_except(
            std::iter::once(("weaponAction", weapon_action.state())).chain(recoil),
        )?;
        Ok(Self {
            weapon_action,
            authored,
        })
    }
}
