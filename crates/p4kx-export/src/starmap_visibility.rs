// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct VisibilityFields {
    hide_in_starmap: SourcedValue<bool>,
    hide_in_world: SourcedValue<bool>,
    hide_when_in_adoption_radius: SourcedValue<bool>,
    block_travel: SourcedValue<bool>,
    only_show_when_parent_selected: SourcedValue<bool>,
}

impl VisibilityFields {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            hide_in_starmap: fields.boolean("hideInStarmap", notes)?,
            hide_in_world: fields.boolean("hideInWorld", notes)?,
            hide_when_in_adoption_radius: fields.boolean("hideWhenInAdoptionRadius", notes)?,
            block_travel: fields.boolean("blockTravel", notes)?,
            only_show_when_parent_selected: fields.boolean("onlyShowWhenParentSelected", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 5] {
        [
            ("hideInStarmap", self.hide_in_starmap.state()),
            ("hideInWorld", self.hide_in_world.state()),
            (
                "hideWhenInAdoptionRadius",
                self.hide_when_in_adoption_radius.state(),
            ),
            ("blockTravel", self.block_travel.state()),
            (
                "onlyShowWhenParentSelected",
                self.only_show_when_parent_selected.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct VisibilitySettings<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: VisibilityFields,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> VisibilitySettings<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to(
            "visibilitySettings",
            "StarMapPresetVisibilitySettings",
            notes,
            |fields, notes| {
                let values = VisibilityFields::read(fields, notes)?;
                let authored_fields = fields.authored_members_except(values.projected())?;
                Ok(Self {
                    source_type: fields.source_type_value()?,
                    fields: values,
                    authored_fields,
                })
            },
        )
    }
}
