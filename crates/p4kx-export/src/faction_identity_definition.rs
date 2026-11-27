// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::GameTextRef;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "FactionIdentity";
const PRESET_WEIGHT: &str = "EncounterPresetWeight";
const ENCOUNTER_PRESET: &str = "EncounterPreset";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// A physical NPC and encounter identity with its own authored faction relationships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct FactionIdentityDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[schemars(extend("x-gameText" = true))]
    name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<GameTextRef>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction_tag: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction_ref: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    security_network_ref: RecordLink<'source>,
    encounter_preset_weights: SourcedValue<Vec<SourcedValue<EncounterPresetWeight<'source>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    loot_pool_theme_tags: SourcedValue<Vec<RecordLink<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

pub(crate) type PreparedFactionIdentity<'source> =
    PreparedDefinition<'source, FactionIdentityDefinition<'source>>;

impl<'source> FactionIdentityDefinition<'source> {
    /// The physical identity and original header fields.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// The reference to the protected name text.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<GameTextRef> {
        &self.name
    }

    /// The reference to the protected description text.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }

    /// The authored faction tag reference.
    #[must_use]
    pub const fn faction_tag(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.faction_tag
    }

    /// The authored faction definition reference.
    #[must_use]
    pub const fn faction_ref(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.faction_ref
    }

    /// The authored security network reference.
    #[must_use]
    pub const fn security_network_ref(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.security_network_ref
    }

    /// The authored encounter preset weights in source order.
    #[must_use]
    pub const fn encounter_preset_weights(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<EncounterPresetWeight<'source>>>> {
        &self.encounter_preset_weights
    }

    /// The authored loot theme tags in source order.
    #[must_use]
    pub const fn loot_pool_theme_tags(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.loot_pool_theme_tags
    }

    /// The original declarations outside the typed fields.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.protected_locale("name", projection, notes)?;
        let description = fields.protected_locale("description", projection, notes)?;
        let faction_tag = fields.record_link_to("factionTag", "Tag", notes)?;
        let faction_ref =
            fields.record_link_to("factionRef", super::faction_definition::SOURCE_TYPE, notes)?;
        let security_network_ref = fields.record_link_to(
            "securityNetworkRef",
            super::law_kinds::SECURITY_NETWORK_TYPE,
            notes,
        )?;
        let encounter_preset_weights = fields.class_array_to(
            "encounterPresetWeights",
            PRESET_WEIGHT,
            notes,
            EncounterPresetWeight::read,
        )?;
        let loot_pool_theme_tags =
            fields.record_link_array_to("lootPoolThemeTags", "Tag", notes)?;
        let authored_fields = fields.authored_members_except([
            ("name", name.state()),
            ("description", description.state()),
            ("factionTag", faction_tag.state()),
            ("factionRef", faction_ref.state()),
            ("securityNetworkRef", security_network_ref.state()),
            ("encounterPresetWeights", encounter_preset_weights.state()),
            ("lootPoolThemeTags", loot_pool_theme_tags.state()),
        ])?;
        Ok(Self {
            source,
            name,
            description,
            faction_tag,
            faction_ref,
            security_network_ref,
            encounter_preset_weights,
            loot_pool_theme_tags,
            authored_fields,
        })
    }
}

/// An authored encounter preset selector and its exact weight.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EncounterPresetWeight<'source> {
    source_type: SourcedValue<&'source str>,
    encounter_preset: SourcedValue<&'source str>,
    weight: SourcedValue<ResourceFloat32>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> EncounterPresetWeight<'source> {
    /// The original declaration type of this array member.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The original encounter preset enum name.
    #[must_use]
    pub const fn encounter_preset(&self) -> &SourcedValue<&'source str> {
        &self.encounter_preset
    }

    /// The original single-precision preset weight.
    #[must_use]
    pub const fn weight(&self) -> &SourcedValue<ResourceFloat32> {
        &self.weight
    }

    /// The original declarations outside the typed fields.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let encounter_preset =
            fields.borrowed_enum_to("encounterPreset", ENCOUNTER_PRESET, notes)?;
        let weight = fields.single("weight", notes)?;
        let authored_fields = fields.authored_members_except([
            ("encounterPreset", encounter_preset.state()),
            ("weight", weight.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            encounter_preset,
            weight,
            authored_fields,
        })
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedFactionIdentity<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        FactionIdentityDefinition::read,
    )
}

pub(crate) fn order(rows: &mut [PreparedFactionIdentity<'_>]) {
    PreparedDefinition::order(rows);
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedFactionIdentity<'source>>,
) -> CatalogTable<FactionIdentityDefinition<'source>> {
    PreparedDefinition::table(forge, SOURCE_TYPE, rows)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedFactionIdentity<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::FactionIdentities, rows)
}
