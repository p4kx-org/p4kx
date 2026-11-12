// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::interaction_source;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "MedBedTierParams";
pub(super) const TIER_ENUM: &str = "MedBedTier";
const STAT_ENUM: &str = "ActorStatType";
const CONSUMABLE_EFFICACY: &str = "DrugEfficacyForConsumableType";

pub(super) type MedicalList<T> = SourcedValue<Vec<SourcedValue<T>>>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MedBedTier<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(rename = "medBedTier")]
    tier: SourcedValue<&'source str>,
    default_respawn_range: SourcedValue<i64>,
    drug_efficacy_for_consumable_types: MedicalList<UnitSource<'source>>,
    stat_types_to_heal: MedicalList<&'source str>,
}

impl<'source> MedBedTier<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tier = source.borrowed_enum_to("medBedTier", TIER_ENUM, notes)?;
        let default_respawn_range = source.int64("defaultRespawnRange", notes)?;
        let drug_efficacy_for_consumable_types = source.class_array_to(
            "drugEfficacyForConsumableTypes",
            CONSUMABLE_EFFICACY,
            notes,
            interaction_source::original,
        )?;
        let stat_types_to_heal =
            source.borrowed_enum_array_to("statTypesToHeal", STAT_ENUM, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("medBedTier", tier.state()),
                ("defaultRespawnRange", default_respawn_range.state()),
                (
                    "drugEfficacyForConsumableTypes",
                    drug_efficacy_for_consumable_types.state(),
                ),
                ("statTypesToHeal", stat_types_to_heal.state()),
            ],
        )?;
        Ok(Self {
            source,
            tier,
            default_respawn_range,
            drug_efficacy_for_consumable_types,
            stat_types_to_heal,
        })
    }
}
