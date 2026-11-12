// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::reputation_perk::ReputationPerk;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "SPerkReputationListParams";
const ENTRY: &str = "SPerkReputationParams";

/// One physical reputation-perk list and its ordered owned entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ReputationPerkList<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    perks: SourcedValue<Vec<SourcedValue<ReputationPerkEntry<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

/// One original association from a scope and standing to an owned perk.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ReputationPerkEntry<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    scope: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    standing: SourcedValue<Box<SourceRecordReference<'source>>>,
    perk: SourcedValue<ReputationPerk<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ReputationPerkEntry<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let scope = fields.record_link_to("scope", "SReputationScopeParams", notes)?;
        let standing = fields.record_link_to("standing", "SReputationStandingParams", notes)?;
        let perk = ReputationPerk::read_field(fields, projection, notes)?;
        let authored_fields = fields.authored_members_except([
            ("scope", scope.state()),
            ("standing", standing.state()),
            ("perk", perk.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            scope,
            standing,
            perk,
            authored_fields,
        })
    }
}

/// Prepared physical list and its source accounting.
pub(crate) struct PreparedPerkList<'source> {
    definition: ReputationPerkList<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_path: &'source str,
    pub(crate) physical_index: u32,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct PerkListRead<'source> {
    pub(crate) value: PreparedPerkList<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one selected list without following its Reference targets.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<PerkListRead<'source>, CatalogError> {
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
    let perks = fields.class_array_to("perks", ENTRY, &mut notes, |entry, notes| {
        ReputationPerkEntry::read(entry, projection, notes)
    })?;
    let authored_fields = fields.authored_members_except([("perks", perks.state())])?;
    Ok(PerkListRead {
        value: PreparedPerkList {
            definition: ReputationPerkList {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
                perks,
                authored_fields,
            },
            physical_guid: tree.guid(),
            physical_path: tree.record_path(),
            physical_index: tree.record_index(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

/// Builds the available physical perk-list table.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedPerkList<'source>>,
) -> CatalogTable<ReputationPerkList<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
