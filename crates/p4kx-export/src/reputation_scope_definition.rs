// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::GameTextRef;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

const SOURCE_TYPE: &str = "SReputationScopeParams";
const MAP_TYPE: &str = "SReputationStandingMapParams";
const STANDING_TYPE: &str = "SReputationStandingParams";

type StandingLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// An authored reputation scope and its ordered standing map.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScopeDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    scope_name: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    display_name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<GameTextRef>,
    icon: SourcedValue<&'source str>,
    standing_map: SourcedValue<ScopeStandingMap<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ScopeStandingMap<'source> {
    source_type: SourcedValue<&'source str>,
    reputation_ceiling: SourcedValue<ResourceNumber>,
    initial_reputation: SourcedValue<ResourceNumber>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    standings: SourcedValue<Vec<StandingLink<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ScopeStandingMap<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let reputation_ceiling = fields.authored_number("reputationCeiling", notes)?;
        let initial_reputation = fields.authored_number("initialReputation", notes)?;
        let standings = fields.record_link_array_to("standings", STANDING_TYPE, notes)?;
        let authored_fields = fields.authored_members_except([
            ("reputationCeiling", reputation_ceiling.state()),
            ("initialReputation", initial_reputation.state()),
            ("standings", standings.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            reputation_ceiling,
            initial_reputation,
            standings,
            authored_fields,
        })
    }
}

pub(crate) struct PreparedScope<'source> {
    definition: ScopeDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct ScopeRead<'source> {
    pub(crate) value: PreparedScope<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one physical scope Record and its owned standing map.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopeRead<'source>, CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let definition = ScopeDefinition::read(
        CanonicalRecordHeader::read(tree, projection.origins())?,
        fields,
        projection,
        &mut notes,
    )?;
    Ok(ScopeRead {
        value: PreparedScope {
            definition,
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

impl<'source> ScopeDefinition<'source> {
    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let scope_name = fields.borrowed_string("scopeName", notes)?;
        let display_name =
            fields.protected_locale_or_legacy_key("displayName", projection, notes)?;
        let description = fields.protected_locale("description", projection, notes)?;
        let icon = fields.borrowed_string("icon", notes)?;
        let standing_map =
            fields.inline_class("standingMap", &[MAP_TYPE], notes, ScopeStandingMap::read)?;
        let authored_fields = fields.authored_members_except([
            ("scopeName", scope_name.state()),
            ("displayName", display_name.state()),
            ("description", description.state()),
            ("icon", icon.state()),
            ("standingMap", standing_map.state()),
        ])?;
        Ok(Self {
            source,
            scope_name,
            display_name,
            description,
            icon,
            standing_map,
            authored_fields,
        })
    }
}

/// Builds the scope table with its physical source-family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedScope<'source>>,
) -> CatalogTable<ScopeDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
