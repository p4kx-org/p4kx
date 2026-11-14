// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::item_component_owner::ComponentOwner;
#[path = "missile_guidance.rs"]
mod missile_guidance;
#[path = "missile_shake.rs"]
mod missile_shake;
#[path = "missile_targeting.rs"]
mod missile_targeting;

use super::super::ordnance_fields::OrdnanceFields;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use missile_guidance::MissileGuidance;
use missile_shake::MissileShake;
use missile_targeting::MissileTargeting;

pub(in crate::catalog_v3) const SOURCE_TYPE: &str = "SCItemMissileParams";

/// One missile's authored guidance, targeting and shake on its physical item slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissileConfiguration<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    guidance: SourcedValue<MissileGuidance<'source>>,
    targeting: SourcedValue<MissileTargeting<'source>>,
    shake: SourcedValue<MissileShake<'source>>,
}

impl<'source> MissileConfiguration<'source> {
    pub(in crate::catalog_v3) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        ordnance: &OrdnanceFields<'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let guidance = source.declared_class_to(
            "GCSParams",
            "SGuidanceAndControlParams",
            notes,
            MissileGuidance::read,
        )?;
        let targeting = source.declared_class_to(
            "targetingParams",
            "STargetingParams",
            notes,
            MissileTargeting::read,
        )?;
        let shake = source.strong_pointer_family_to(
            "shakeParams",
            "SShakeParams",
            notes,
            MissileShake::read,
        )?;
        let source = UnitSource::read(
            source,
            ordnance.projected().into_iter().chain([
                ("GCSParams", guidance.state()),
                ("targetingParams", targeting.state()),
                ("shakeParams", shake.state()),
            ]),
        )?;
        Ok(Self {
            owner,
            source,
            guidance,
            targeting,
            shake,
        })
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<MissileConfiguration<'source>>,
) -> CatalogTable<MissileConfiguration<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}
