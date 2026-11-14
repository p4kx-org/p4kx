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
use crate::catalog_v3::interaction_source::RecordLink;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(in crate::catalog_v3) const RECOIL_CONFIG_SOURCE_TYPE: &str = "ActorProceduralRecoilConfig";
pub(in crate::catalog_v3) const RECOIL_MODIFIERS_SOURCE_TYPE: &str =
    "ActorProceduralRecoilModifiers";
const SETUP_FIELD: &str = "actorProceduralRecoilSetup";
const SETUP_TYPE: &str = "ActorProceduralRecoilSetup";
const MODIFIERS_FIELD: &str = "actorProceduralRecoilModifiers";

/// Ordered recoil setups belonging to one physical actor recoil configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ActorProceduralRecoilConfig<'source> {
    source: CanonicalRecordHeader<'source>,
    actor_procedural_recoil_setup:
        SourcedValue<Vec<SourcedValue<ActorProceduralRecoilSetup<'source>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ActorProceduralRecoilConfig<'source> {
    pub(in crate::catalog_v3) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let setups = fields.class_array_to(
            SETUP_FIELD,
            SETUP_TYPE,
            notes,
            ActorProceduralRecoilSetup::read,
        )?;
        let additional_fields =
            fields.authored_members_except([(SETUP_FIELD, complete_array_state(&setups))])?;
        Ok(Self {
            source,
            actor_procedural_recoil_setup: setups,
            additional_fields,
        })
    }

    pub(in crate::catalog_v3) fn linked_definitions(&self) -> impl Iterator<Item = u32> + '_ {
        self.actor_procedural_recoil_setup
            .value()
            .into_iter()
            .flatten()
            .filter_map(SourcedValue::value)
            .filter_map(|setup| linked_record_index(&setup.actor_procedural_recoil_modifiers))
    }
}

/// Recoil modifier Reference and actor conditions of an original setup occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ActorProceduralRecoilSetup<'source> {
    source_type: SourcedValue<&'source str>,
    filter: ActorStateFilter<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    actor_procedural_recoil_modifiers: RecordLink<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ActorProceduralRecoilSetup<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let filter = ActorStateFilter::read(fields, notes)?;
        let actor_procedural_recoil_modifiers =
            fields.record_link_to(MODIFIERS_FIELD, RECOIL_MODIFIERS_SOURCE_TYPE, notes)?;
        let additional_fields = fields.authored_members_except(
            filter
                .projected()
                .into_iter()
                .chain([(MODIFIERS_FIELD, actor_procedural_recoil_modifiers.state())]),
        )?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            filter,
            actor_procedural_recoil_modifiers,
            additional_fields,
        })
    }
}
