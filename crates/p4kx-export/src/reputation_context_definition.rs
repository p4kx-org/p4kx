// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::reputation_blackboard::BlackboardProperty;
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

const SOURCE_TYPE: &str = "SReputationContextUI";
const SCOPE_CONTEXT: &str = "SReputationScopeContextUI";
const SORT_ORDER: &str = "EReputationSortOrderScope";

/// Authored reputation display scope configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContextDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    sort_order_scope: SourcedValue<&'source str>,
    primary_scope_context: SourcedValue<ScopeContext<'source>>,
    scope_context_list: SourcedValue<Vec<SourcedValue<ScopeContext<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ScopeContext<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    scope: SourcedValue<Box<SourceRecordReference<'source>>>,
    properties_bb: SourcedValue<Vec<SourcedValue<BlackboardProperty<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ScopeContext<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let scope = fields.record_link("scope", notes)?;
        let properties_bb = BlackboardProperty::read_array(fields, projection, notes)?;
        let authored_fields = fields.authored_members_except([
            ("scope", scope.state()),
            ("propertiesBB", properties_bb.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            scope,
            properties_bb,
            authored_fields,
        })
    }
}

impl<'source> ContextDefinition<'source> {
    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let sort_order_scope = fields.borrowed_enum_to("sortOrderScope", SORT_ORDER, notes)?;
        let primary_scope_context = fields.inline_class(
            "primaryScopeContext",
            &[SCOPE_CONTEXT],
            notes,
            |scope, notes| ScopeContext::read(scope, projection, notes),
        )?;
        let scope_context_list =
            fields.class_array_to("scopeContextList", SCOPE_CONTEXT, notes, |scope, notes| {
                ScopeContext::read(scope, projection, notes)
            })?;
        let authored_fields = fields.authored_members_except([
            ("sortOrderScope", sort_order_scope.state()),
            ("primaryScopeContext", primary_scope_context.state()),
            ("scopeContextList", scope_context_list.state()),
        ])?;
        Ok(Self {
            source,
            sort_order_scope,
            primary_scope_context,
            scope_context_list,
            authored_fields,
        })
    }
}

pub(crate) struct PreparedContext<'source> {
    definition: ContextDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct ContextRead<'source> {
    pub(crate) value: PreparedContext<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one physical reputation context with its owned scope occurrences.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ContextRead<'source>, CatalogError> {
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
    let definition = ContextDefinition::read(
        CanonicalRecordHeader::read(tree, projection.origins())?,
        fields,
        projection,
        &mut notes,
    )?;
    Ok(ContextRead {
        value: PreparedContext {
            definition,
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

/// Builds the context table with its physical source-family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedContext<'source>>,
) -> CatalogTable<ContextDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
