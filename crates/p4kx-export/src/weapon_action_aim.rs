// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::interaction_source::original;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

use super::super::operating::WeaponModifier;

/// Stored aim modifier, geometry settings and reload UI flag of a concrete aim action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AimAction<'source> {
    aim_modifier: SourcedValue<WeaponModifier<'source>>,
    aim_geometry_tags: SourcedValue<UnitSource<'source>>,
    #[serde(rename = "hasReloadModesOnUI")]
    has_reload_modes_on_ui: SourcedValue<bool>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> AimAction<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        recoil: [(&'static str, SourceValueState); 3],
    ) -> Result<Self, CatalogError> {
        let aim_modifier = WeaponModifier::read_field(source, "aimModifier", notes)?;
        let aim_geometry_tags = source.strong_pointer_family_to(
            "aimGeometryTags",
            "SWeaponGeometryTagsParams",
            notes,
            original,
        )?;
        let has_reload_modes_on_ui = source.boolean("hasReloadModesOnUI", notes)?;
        let authored = source.authored_members_except(
            [
                ("aimModifier", aim_modifier.state()),
                ("aimGeometryTags", aim_geometry_tags.state()),
                ("hasReloadModesOnUI", has_reload_modes_on_ui.state()),
            ]
            .into_iter()
            .chain(recoil),
        )?;
        Ok(Self {
            aim_modifier,
            aim_geometry_tags,
            has_reload_modes_on_ui,
            authored,
        })
    }
}
