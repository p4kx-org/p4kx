// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::OriginalValuePart;
use crate::record_read::ReadError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldCapture;
use crate::source_value_catalog::SourceValueRef;
use crate::template_context_value::TemplateVocabularyProjection;
use crate::template_issues::{TemplateError, Unknowns};
use crate::template_modifier_kind::TemplateModifierKind;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateModifierProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: SourcedValue<Option<TemplateModifierKind>>,
    pub(crate) kiosk_terms: Vec<SourceValueRef>,
}

pub(crate) fn read<'source>(
    members: &TemplateMembers<'_, '_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(Vec<TemplateModifierProjection>, Vec<ReadError>), CatalogError> {
    read_selected(members, capture, false, |_| Ok(()))
}

pub(crate) fn required<'source>(
    members: &TemplateMembers<'_, '_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    unknowns: &Unknowns,
    vocabularies: &mut Vec<TemplateVocabularyProjection>,
) -> Result<Vec<TemplateModifierProjection>, ReadError> {
    let (rows, errors) = read_selected(members, capture, true, |fact| {
        crate::template_vocabulary_read::broker_modifier(members, fact, unknowns, vocabularies)
    })?;
    if let Some(error) = errors.into_iter().next() {
        return Err(error);
    }
    Ok(rows)
}

fn read_selected<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    capture: &dyn FieldCapture<'source>,
    required: bool,
    mut recognized: impl FnMut(
        &'fields crate::catalog_read::original::OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<(), CatalogError>,
) -> Result<(Vec<TemplateModifierProjection>, Vec<ReadError>), CatalogError> {
    let fields = members.fields();
    let Some((_, declaration)) = fields.field(None, "modifiers")? else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for modifier in fields.structs(declaration)? {
        let Value::Struct(source) = modifier.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let kind = TemplateModifierKind::read(source.type_name());
        if kind.is_none() {
            let body = capture.unknown_type(source, "modifiers")?;
            failures.push(
                Box::new(TemplateError::unknown("modifiers", source).with_body(body)) as ReadError,
            );
            if required {
                break;
            }
        } else {
            recognized(modifier)?;
        }
        let mut interpreted = modifier.bind(
            OriginalValuePart::ConcreteType,
            SourceValue::Present(kind),
            SourceUseRole::Selector,
            true,
        )?;
        interpreted.contribute(
            modifier.observe(OriginalValuePart::ConcreteType, SourceUseRole::Conversion)?,
        );
        let kiosk_terms = if matches!(kind, Some(TemplateModifierKind::ShipKioskFilter))
            && let Some(filter) = fields.selected_struct(modifier, "kioskFilterType")?
            && let Some((_, terms)) = fields.field(Some(filter), "terms")?
        {
            fields
                .structs(terms)?
                .into_iter()
                .map(|term| members.value_ref(term))
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };
        rows.push(TemplateModifierProjection {
            source: members.value_ref(modifier)?,
            kind: interpreted.into_cell(fields.sources()),
            kiosk_terms,
        });
    }
    Ok((rows, failures))
}
