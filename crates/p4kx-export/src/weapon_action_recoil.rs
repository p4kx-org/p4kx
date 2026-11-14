// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::interaction_source::RecordLink;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const CONFIG_TYPE: &str = "WeaponProceduralRecoilConfigDef";

/// Original recoil references and interval of one action occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WeaponActionRecoil<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    recoil: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    recoil_grappling: RecordLink<'source>,
    recoil_interval: SourcedValue<ResourceFloat32>,
}

impl<'source> WeaponActionRecoil<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            recoil: source.record_link_to("recoil", CONFIG_TYPE, notes)?,
            recoil_grappling: source.record_link_to("recoilGrappling", CONFIG_TYPE, notes)?,
            recoil_interval: source.single("recoilInterval", notes)?,
        })
    }

    pub(super) fn linked_definitions(&self) -> impl Iterator<Item = u32> + '_ {
        [&self.recoil, &self.recoil_grappling]
            .into_iter()
            .filter_map(|link| link.value()?.resolved_index())
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 3] {
        [
            ("recoil", self.recoil.state()),
            ("recoilGrappling", self.recoil_grappling.state()),
            ("recoilInterval", self.recoil_interval.state()),
        ]
    }
}
