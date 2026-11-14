// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::record_read::ReadError;
use crate::scoped_location_kind::ScopedLocationFilterKind;
use crate::source_fields::{FieldCapture, unknown_struct};
use crate::template_property_kind::PropertyValueKind;
use crate::template_property_plan::SelectedProperty;
use crate::template_source_members::TemplateMembers;
use crate::template_value_meaning::{
    TemplateDatasetCondition, TemplateMissionItemSelection, TemplateScopedFilter,
    TemplateValueMeaning, TemplateValueStatus,
};
use crate::{CatalogError, ExportAccountingError};

pub(crate) struct ValueMeanings {
    pub(crate) rows: Vec<TemplateValueMeaning>,
    pub(crate) failures: Vec<(u64, ReadError)>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    selected: &[SelectedProperty<'fields, 'scope, 'read, 'source>],
    spawn_failures: &HashMap<u64, Option<ExportDiagnosticCode>>,
    capture: &dyn FieldCapture<'source>,
) -> Result<ValueMeanings, CatalogError> {
    let mut result = ValueMeanings {
        rows: Vec::new(),
        failures: Vec::new(),
    };
    let mut seen = HashSet::new();
    let ordered = selected
        .iter()
        .filter(|property| property.objective.is_some())
        .chain(
            selected
                .iter()
                .filter(|property| property.objective.is_none()),
        );
    for property in ordered {
        let Some(fact) = property.value else {
            continue;
        };
        members.fields().check_cancel()?;
        if !seen.insert(fact.ordinal()) {
            continue;
        }
        let validated_status = spawn_failures
            .get(&fact.ordinal())
            .copied()
            .map_or(TemplateValueStatus::Recognized, status);
        let CheckedValue { row, failure } = one(members, fact, validated_status, capture)?;
        result.rows.push(row);
        if let Some(error) = failure {
            result.failures.push((fact.ordinal(), error));
        }
    }
    Ok(result)
}

pub(crate) struct CheckedValue {
    pub(crate) row: TemplateValueMeaning,
    pub(crate) failure: Option<ReadError>,
}

pub(crate) fn one<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    validated_status: TemplateValueStatus,
    capture: &dyn FieldCapture<'source>,
) -> Result<CheckedValue, CatalogError> {
    let Value::Struct(source) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let kind = PropertyValueKind::read(source.type_name());
    let checked = kind.map_or_else(
        || Err(unknown_struct(source, "property value", Some(capture))),
        |kind| crate::template_property_value_check::validate(source, kind, capture),
    );
    let (status, failure) = match checked {
        Ok(()) => (validated_status, None),
        Err(error) => {
            let error = match error.downcast::<CatalogError>() {
                Ok(error) => return Err(*error),
                Err(error) => error,
            };
            let status = status(crate::export_source_failure::diagnosis(error.as_ref()).code);
            (status, Some(error))
        }
    };
    Ok(CheckedValue {
        row: TemplateValueMeaning {
            source: members.value_ref(fact)?,
            kind: kind_cell(members, fact, kind)?,
            status,
            conditions: conditions(members, fact, kind)?,
            mission_item: matches!(kind, Some(PropertyValueKind::MissionItem))
                .then(|| {
                    TemplateMissionItemSelection::read(members.fields(), fact, |value| {
                        members.value_ref(value)
                    })
                })
                .transpose()?,
            scoped_filters: scoped(members, fact, kind)?,
        },
        failure,
    })
}

const fn status(code: Option<ExportDiagnosticCode>) -> TemplateValueStatus {
    match code {
        Some(ExportDiagnosticCode::UnsupportedValue) => TemplateValueStatus::Unsupported,
        Some(ExportDiagnosticCode::UnreadableSource) => TemplateValueStatus::Unreadable,
        _ => TemplateValueStatus::Malformed,
    }
}

fn kind_cell<T>(
    members: &TemplateMembers<'_, '_, '_, '_>,
    fact: &OriginalValueFact<'_, '_, '_>,
    value: T,
) -> Result<SourcedValue<T>, CatalogError> {
    let mut value = fact.bind(
        OriginalValuePart::ConcreteType,
        SourceValue::Present(value),
        SourceUseRole::Selector,
        true,
    )?;
    value.contribute(fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Conversion)?);
    Ok(value.into_cell(members.fields().sources()))
}

fn conditions<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    kind: Option<PropertyValueKind>,
) -> Result<Vec<TemplateDatasetCondition>, CatalogError> {
    match kind {
        Some(PropertyValueKind::Organization | PropertyValueKind::MissionItem) => {
            crate::template_value_meaning::dataset_conditions(
                members.fields(),
                fact,
                matches!(kind, Some(PropertyValueKind::MissionItem)),
                |fact| members.value_ref(fact),
            )
        }
        _ => Ok(Vec::new()),
    }
}

fn scoped<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    kind: Option<PropertyValueKind>,
) -> Result<Vec<TemplateScopedFilter>, CatalogError> {
    if !matches!(kind, Some(PropertyValueKind::ScopedLocation)) {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    for name in [
        "factionFilter",
        "locationClassFilter",
        "locationSizeFilter",
        "locationFunctionFilter",
        "otherLocationFilter",
        "relativeLocationFilter",
    ] {
        if let Some((_, fact)) = members.fields().field(Some(fact), name)?
            && let Value::Struct(source) = fact.value()
        {
            rows.push(TemplateScopedFilter {
                source: members.value_ref(fact)?,
                kind: kind_cell(
                    members,
                    fact,
                    ScopedLocationFilterKind::read(source.type_name()),
                )?,
            });
        }
    }
    Ok(rows)
}
