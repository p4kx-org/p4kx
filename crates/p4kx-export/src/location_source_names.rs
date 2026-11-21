// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::location_template::LocationName;
use crate::mission_layers;
use crate::mission_location_tag::Reader;
use crate::source_fields::{FieldCapture, FieldError, FieldViolation};
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read<'fields, 'scope, 'read, 'source>(
    originals: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    capture: &TemplateFieldCapture<'_, 'scope, 'read, 'source>,
    reader: &Reader<'_, 'fields, 'scope, 'read, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<LocationName>>>, CatalogError> {
    let selected = reader.original_value(&["locationData", "stringVariants", "variants"])?;
    let Some(fact) = selected.value() else {
        return super::non_present(originals.sources(), &selected);
    };
    if matches!(fact.value(), Value::Null) {
        return Ok(originals
            .sources()
            .value_with_origins(SourceValue::Null, &[selected.origin()])?);
    }
    let owner = reader.original_owner(&["locationData", "stringVariants"])?;
    let owner = owner
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let Value::Struct(parent) = owner.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let path = originals.logical_path(owner)?;
    let state = match fact.value() {
        Value::Array(_) | Value::Struct(_) => SourceValue::Present(rows(
            originals,
            capture,
            reader,
            projection,
            &Collection {
                parent,
                path: &path,
                original: fact,
                selection: selected.origin(),
            },
            notes,
        )?),
        value => invalid(capture, parent, &path, "variants", value, notes)?,
    };
    let value = originals
        .sources()
        .value_with_origins(state, &[selected.origin()])?;
    if matches!(fact.value(), Value::Struct(_)) {
        let conversion = fact.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?;
        let origin = originals.sources().origin(&[conversion], true);
        return Ok(originals.sources().add_value_origins(value, &[&origin])?);
    }
    Ok(value)
}

struct Collection<'view, 'fields, 'scope, 'read, 'source> {
    parent: &'view StructValue<'source>,
    path: &'view str,
    original: &'fields OriginalValueFact<'scope, 'read, 'source>,
    selection: &'view FieldOrigin,
}

fn rows<'fields, 'scope, 'read, 'source>(
    originals: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    capture: &TemplateFieldCapture<'_, 'scope, 'read, 'source>,
    reader: &Reader<'_, 'fields, 'scope, 'read, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    collection: &Collection<'_, 'fields, 'scope, 'read, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Vec<SourcedValue<LocationName>>, CatalogError> {
    let failures = reader.variant_failures()?;
    let value = collection.original.value();
    let values = mission_layers::items_value(value);
    let mut rows = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        originals.check_cancel()?;
        let name = if matches!(collection.original.value(), Value::Array(_)) {
            format!("variants[{index}]")
        } else {
            "variants".to_owned()
        };
        rows.push(slot(
            originals,
            capture,
            reader,
            projection,
            &Slot {
                parent: collection.parent,
                path: collection.path,
                name: &name,
                original: originals.fact(value)?,
                selection: collection.selection,
                failures: &failures,
            },
            notes,
        )?);
    }
    Ok(rows)
}

struct Slot<'view, 'fields, 'scope, 'read, 'source> {
    parent: &'view StructValue<'source>,
    path: &'view str,
    name: &'view str,
    original: &'fields OriginalValueFact<'scope, 'read, 'source>,
    selection: &'view FieldOrigin,
    failures: &'view [SourceUse],
}

fn slot<'fields, 'scope, 'read, 'source>(
    originals: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    capture: &TemplateFieldCapture<'_, 'scope, 'read, 'source>,
    reader: &Reader<'_, 'fields, 'scope, 'read, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    selected: &Slot<'_, 'fields, 'scope, 'read, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<LocationName>, CatalogError> {
    let sources = originals.sources();
    let (part, state) = match selected.original.value() {
        Value::Struct(_) => {
            let display_name =
                projection.protected_key_field(originals, Some(selected.original), "string")?;
            (
                OriginalValuePart::ConcreteType,
                SourceValue::Present(LocationName {
                    tag: reader.variant(selected.original, selected.failures)?,
                    display_name: sources.add_value_origins(display_name, &[selected.selection])?,
                }),
            )
        }
        Value::Null => (OriginalValuePart::Value, SourceValue::Null),
        value => (
            OriginalValuePart::Value,
            invalid(
                capture,
                selected.parent,
                selected.path,
                selected.name,
                value,
                notes,
            )?,
        ),
    };
    let cell = selected
        .original
        .bind(part, state, SourceUseRole::Selector, true)?
        .into_cell(sources);
    Ok(sources.add_value_origins(cell, &[selected.selection])?)
}

fn invalid<'source, T>(
    capture: &TemplateFieldCapture<'_, '_, '_, 'source>,
    parent: &StructValue<'source>,
    path: &str,
    name: &str,
    value: &Value<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourceValue<T>, CatalogError> {
    let violation = FieldViolation::invalid(
        parent,
        path,
        name,
        value,
        "a nullable location-name Struct or collection",
    );
    let body = capture.field_failure(&violation)?;
    notes.push(violation.into_error().with_body(body));
    Ok(if matches!(value, Value::Error(_)) {
        SourceValue::Unreadable
    } else {
        SourceValue::Invalid
    })
}
