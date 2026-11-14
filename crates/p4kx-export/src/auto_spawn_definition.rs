// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::auto_spawn_data::AutoSpawnData;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(crate) const SPAWN_TYPE: &str = "AutoSpawn";
pub(crate) const SET_TYPE: &str = "AutoSpawns";
pub(crate) const ENCOUNTER_TYPE: &str = "EncounterIndex";
const SET_ENTRY_TYPE: &str = "AutoSpawnsData";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// The physical Record for one authored automatic spawn.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AutoSpawnDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    auto_spawn_data: SourcedValue<AutoSpawnData<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> AutoSpawnDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let auto_spawn_data = AutoSpawnData::read(fields, notes)?;
        let authored_fields =
            fields.authored_members_except([("autoSpawnData", auto_spawn_data.state())])?;
        Ok(Self {
            source,
            auto_spawn_data,
            authored_fields,
        })
    }
}

/// A physical set of ordered, individually authored automatic spawn entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AutoSpawnSet<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    auto_spawn_data: SourcedValue<Vec<SourcedValue<AutoSpawnSetEntry<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> AutoSpawnSet<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let auto_spawn_data = fields.strong_pointer_family_array_to(
            "autoSpawnData",
            SET_ENTRY_TYPE,
            notes,
            AutoSpawnSetEntry::read,
        )?;
        let authored_fields =
            fields.authored_members_except([("autoSpawnData", auto_spawn_data.state())])?;
        Ok(Self {
            source,
            auto_spawn_data,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AutoSpawnSetEntry<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    data: SetEntryData<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum SetEntryData<'source> {
    Record {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        record: RecordLink<'source>,
    },
    Manual {
        auto_spawn_data: Box<SourcedValue<AutoSpawnData<'source>>>,
    },
    Authored,
}

impl<'source> AutoSpawnSetEntry<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let data = match fields.source_type()? {
            "AutoSpawnsDataRef" => SetEntryData::Record {
                record: fields.record_link_to("record", SPAWN_TYPE, notes)?,
            },
            "AutoSpawnsDataManual" => SetEntryData::Manual {
                auto_spawn_data: Box::new(AutoSpawnData::read(fields, notes)?),
            },
            _ => {
                fields.note_retained_variant(notes)?;
                SetEntryData::Authored
            }
        };
        Ok(Self {
            source_type: fields.source_type_value()?,
            authored_fields: fields.authored_members_except(data.projected())?,
            data,
        })
    }
}

impl SetEntryData<'_> {
    const fn projected(&self) -> Option<(&'static str, SourceValueState)> {
        match self {
            Self::Record { record } => Some(("record", record.state())),
            Self::Manual { auto_spawn_data } => Some(("autoSpawnData", auto_spawn_data.state())),
            Self::Authored => None,
        }
    }
}

/// A physical encounter index and its ordered links to spawn sets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EncounterIndex<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    encounters: SourcedValue<Vec<SourcedValue<EncounterIndexEntry<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> EncounterIndex<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let encounters = fields.class_array_to(
            "encounters",
            "EncounterIndexEntry",
            notes,
            EncounterIndexEntry::read,
        )?;
        let authored_fields =
            fields.authored_members_except([("encounters", encounters.state())])?;
        Ok(Self {
            source,
            encounters,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EncounterIndexEntry<'source> {
    encounter_preset: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    auto_spawns: SourcedValue<Vec<RecordLink<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> EncounterIndexEntry<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let encounter_preset =
            fields.borrowed_enum_to("encounterPreset", "EncounterPreset", notes)?;
        let auto_spawns = fields.record_link_array_to("autoSpawns", SET_TYPE, notes)?;
        let authored_fields = fields.authored_members_except([
            ("encounterPreset", encounter_preset.state()),
            ("autoSpawns", auto_spawns.state()),
        ])?;
        Ok(Self {
            encounter_preset,
            auto_spawns,
            authored_fields,
        })
    }
}
