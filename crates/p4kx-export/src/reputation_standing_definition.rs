// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::GameTextRef;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

const SOURCE_TYPE: &str = "SReputationStandingParams";

/// An original standing definition with its authored presentation and progression fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StandingDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    presentation: StandingPresentation<'source>,
    #[serde(flatten)]
    progression: StandingProgression,
    additional_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct StandingPresentation<'source> {
    name: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    display_name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    perk_description: SourcedValue<GameTextRef>,
    icon: SourcedValue<&'source str>,
    mission_board_icon: SourcedValue<&'source str>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct StandingProgression {
    min_reputation: SourcedValue<ResourceNumber>,
    drift_reputation: SourcedValue<ResourceNumber>,
    /// The original number of hours between reputation drifts.
    drift_time_hours: SourcedValue<ResourceFloat32>,
    gated: SourcedValue<bool>,
}

pub(crate) struct PreparedStanding<'source> {
    value: StandingDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct StandingRead<'source> {
    pub(crate) value: PreparedStanding<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one physical standing Record.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<StandingRead<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let presentation = StandingPresentation::read(fields, projection, &mut notes)?;
    let progression = StandingProgression::read(fields, &mut notes)?;
    let additional_fields = fields.authored_members_except(
        presentation
            .projected()
            .into_iter()
            .chain(progression.projected()),
    )?;
    Ok(StandingRead {
        value: PreparedStanding {
            value: StandingDefinition {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
                presentation,
                progression,
                additional_fields,
            },
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

impl<'source> StandingPresentation<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: source.borrowed_string("name", notes)?,
            description: source.borrowed_string("description", notes)?,
            display_name: source.protected_locale("displayName", projection, notes)?,
            perk_description: source.protected_locale("perkDescription", projection, notes)?,
            icon: source.borrowed_string("icon", notes)?,
            mission_board_icon: source.borrowed_string("missionBoardIcon", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("name", self.name.state()),
            ("description", self.description.state()),
            ("displayName", self.display_name.state()),
            ("perkDescription", self.perk_description.state()),
            ("icon", self.icon.state()),
            ("missionBoardIcon", self.mission_board_icon.state()),
        ]
    }
}

impl StandingProgression {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min_reputation: source.authored_number("minReputation", notes)?,
            drift_reputation: source.authored_number("driftReputation", notes)?,
            drift_time_hours: source.single("driftTimeHours", notes)?,
            gated: source.boolean("gated", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 4] {
        [
            ("minReputation", self.min_reputation.state()),
            ("driftReputation", self.drift_reputation.state()),
            ("driftTimeHours", self.drift_time_hours.state()),
            ("gated", self.gated.state()),
        ]
    }
}

/// Builds the standing table in canonical GUID and physical Record order.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedStanding<'source>>,
) -> CatalogTable<StandingDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.value).collect(),
    )
}
