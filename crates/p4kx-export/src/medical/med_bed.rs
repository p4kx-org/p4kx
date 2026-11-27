// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::interaction_source::RecordLink;
use super::med_bed_links::MedBedLinks;
use super::med_bed_resource_consumption::{self, Capacity};
use super::med_bed_surgery_names::{self, MedBedSurgeryNames};
use super::medical_tier;
use super::medical_tier_config;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(in crate::catalog_v3) const COMPONENT: &str = "MedBedComponentParams";
const RANGE_SOURCE: &str = "MedBedRespawnRangeOverride";

/// The original fields of one authored medical facility component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct MedBed<'source> {
    #[serde(flatten)]
    links: MedBedLinks<'source>,
    #[serde(flatten)]
    fields: MedBedFields<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> MedBed<'source> {
    pub(in crate::catalog_v3) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let links = MedBedLinks::read(source, notes)?;
        let fields = MedBedFields::read(source, projection, notes)?;
        let additional_fields =
            source.authored_members_except(links.members().into_iter().chain(fields.members()))?;
        Ok(Self {
            links,
            fields,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MedBedFields<'source> {
    respawn_range_override: SourcedValue<MedBedRespawnRange<'source>>,
    time_to_heal: SourcedValue<ResourceFloat32>,
    delay_before_heal: SourcedValue<ResourceFloat32>,
    med_bed_tier: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    medical_item_tier_config: RecordLink<'source>,
    can_respawn_here: SourcedValue<bool>,
    invulnerable_user: SourcedValue<bool>,
    invulnerable_duration: SourcedValue<ResourceFloat32>,
    surgery_names: SourcedValue<MedBedSurgeryNames<'source>>,
    resource_regeneration_per_minute: Capacity<'source>,
}

impl<'source> MedBedFields<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            respawn_range_override: source.strong_pointer_to(
                "respawnRangeOverride",
                RANGE_SOURCE,
                &[RANGE_SOURCE],
                notes,
                MedBedRespawnRange::read,
            )?,
            time_to_heal: source.single("timeToHeal", notes)?,
            delay_before_heal: source.single("delayBeforeHeal", notes)?,
            med_bed_tier: source.borrowed_enum_to("medBedTier", medical_tier::TIER_ENUM, notes)?,
            medical_item_tier_config: source.record_link_to(
                "medicalItemTierConfig",
                medical_tier_config::SOURCE_TYPE,
                notes,
            )?,
            can_respawn_here: source.boolean("canRespawnHere", notes)?,
            invulnerable_user: source.boolean("invulnerableUser", notes)?,
            invulnerable_duration: source.single("invulnerableDuration", notes)?,
            surgery_names: source.declared_class_to(
                "surgeryNames",
                med_bed_surgery_names::SOURCE_TYPE,
                notes,
                |source, notes| MedBedSurgeryNames::read(source, projection, notes),
            )?,
            resource_regeneration_per_minute: med_bed_resource_consumption::capacity(
                source,
                "resourceRegenerationPerMinute",
                notes,
            )?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 10] {
        [
            ("respawnRangeOverride", self.respawn_range_override.state()),
            ("timeToHeal", self.time_to_heal.state()),
            ("delayBeforeHeal", self.delay_before_heal.state()),
            ("medBedTier", self.med_bed_tier.state()),
            (
                "medicalItemTierConfig",
                self.medical_item_tier_config.state(),
            ),
            ("canRespawnHere", self.can_respawn_here.state()),
            ("invulnerableUser", self.invulnerable_user.state()),
            ("invulnerableDuration", self.invulnerable_duration.state()),
            ("surgeryNames", self.surgery_names.state()),
            (
                "resourceRegenerationPerMinute",
                self.resource_regeneration_per_minute.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MedBedRespawnRange<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    respawn_range: SourcedValue<i64>,
}

impl<'source> MedBedRespawnRange<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let respawn_range = source.int64("respawnRange", notes)?;
        let source = UnitSource::read(source, [("respawnRange", respawn_range.state())])?;
        Ok(Self {
            source,
            respawn_range,
        })
    }
}
