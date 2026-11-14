// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::actor_state_filter::ActorStateFilter;
use super::weapon_procedural_source::{complete_array_state, linked_record_index};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::interaction_source::RecordLinks;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(in crate::catalog_v3) const ANIMATION_SOURCE_TYPE: &str = "WeaponProceduralAnimation";
pub(in crate::catalog_v3) const CLIP_SOURCE_TYPE: &str = "WeaponProceduralClip";
const SETUP_FIELD: &str = "weaponProceduralClipsSetUp";
const SETUP_TYPE: &str = "WeaponProceduralClipsSetUp";
const CLIPS_FIELD: &str = "weaponProceduralClips";

/// Ordered authored clip setups belonging to one physical animation definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponProceduralAnimation<'source> {
    source: CanonicalRecordHeader<'source>,
    weapon_procedural_clips_set_up:
        SourcedValue<Vec<SourcedValue<WeaponProceduralClipSetup<'source>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> WeaponProceduralAnimation<'source> {
    pub(in crate::catalog_v3) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let setups = fields.class_array_to(
            SETUP_FIELD,
            SETUP_TYPE,
            notes,
            WeaponProceduralClipSetup::read,
        )?;
        let additional_fields =
            fields.authored_members_except([(SETUP_FIELD, complete_array_state(&setups))])?;
        Ok(Self {
            source,
            weapon_procedural_clips_set_up: setups,
            additional_fields,
        })
    }

    pub(in crate::catalog_v3) fn linked_definitions(&self) -> impl Iterator<Item = u32> + '_ {
        self.weapon_procedural_clips_set_up
            .value()
            .into_iter()
            .flatten()
            .filter_map(SourcedValue::value)
            .flat_map(WeaponProceduralClipSetup::linked_definitions)
    }
}

/// Clip References and actor conditions of an original setup occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct WeaponProceduralClipSetup<'source> {
    source_type: SourcedValue<&'source str>,
    filter: ActorStateFilter<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    weapon_procedural_clips: RecordLinks<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> WeaponProceduralClipSetup<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let filter = ActorStateFilter::read(fields, notes)?;
        let weapon_procedural_clips =
            fields.record_link_array_to(CLIPS_FIELD, CLIP_SOURCE_TYPE, notes)?;
        let additional_fields = fields.authored_members_except(
            filter
                .projected()
                .into_iter()
                .chain([(CLIPS_FIELD, complete_array_state(&weapon_procedural_clips))]),
        )?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            filter,
            weapon_procedural_clips,
            additional_fields,
        })
    }

    fn linked_definitions(&self) -> impl Iterator<Item = u32> + '_ {
        self.weapon_procedural_clips
            .value()
            .into_iter()
            .flatten()
            .filter_map(linked_record_index)
    }
}

#[cfg(test)]
#[path = "weapon_procedural_source_tests.rs"]
mod tests;
