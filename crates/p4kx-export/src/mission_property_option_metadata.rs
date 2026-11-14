// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::mission_layers;
use crate::mission_property_option::{DependentProperty, OptionMetadata};
use crate::mission_property_source;
use crate::owned_reference_read;
use crate::record_read::ReadError;
use crate::scoped_location_weak;
use crate::source_fields::{FieldCapture, FieldChecks};
use crate::spawn_read::Reader;
use crate::texts::Texts;

struct MetadataView<'capture, 'view, 'source> {
    fields: FieldChecks<'capture, 'view, 'source>,
}

impl<'capture, 'view, 'source> MetadataView<'capture, 'view, 'source> {
    fn new(
        source: &'view StructValue<'source>,
        path: &'view str,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        Ok(Self {
            fields: FieldChecks::for_context(source, path, capture)?,
        })
    }
    fn text_id(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("textId")
    }
    fn weighting(&self) -> Result<Option<f32>, ReadError> {
        self.fields.number("weighting")
    }
    fn forced(&self) -> Result<Option<bool>, ReadError> {
        self.fields.boolean("DEBUG_forceChooseThisOption")
    }
}

pub(super) fn metadata<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<OptionMetadata<'source>, ReadError> {
    let view = MetadataView::new(source, path, capture)?;
    Ok(OptionMetadata {
        text_id: view.text_id()?,
        display_name: texts.localised(mission_layers::key(Some(source), "textId")),
        weighting: view.weighting()?,
        debug_force_choose_this_option: view.forced()?,
        dependent_properties: mission_property_source::list(
            source,
            "dependentProperties",
            path,
            capture,
            |source, path| dependent(reader, source, path, capture),
        )?,
    })
}

fn dependent_fields<'capture, 'view, 'source>(
    source: &'view StructValue<'source>,
    path: &'view str,
    capture: Option<&'capture dyn FieldCapture<'source>>,
) -> Result<FieldChecks<'capture, 'view, 'source>, ReadError> {
    let fields = FieldChecks::for_context(source, path, capture)?;
    fields.check(&["property", "values"])?;
    Ok(fields)
}

fn property<'view, 'source>(
    source: &'view StructValue<'source>,
    fields: &FieldChecks<'_, '_, 'source>,
) -> Result<Option<&'view Value<'source>>, ReadError> {
    if let Some(value) = source.field("property") {
        owned_reference_read::check(value).map_err(|()| {
            fields.invalid(
                "property",
                value,
                "a nullable mission-property weak pointer",
            )
        })?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

fn dependent<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<DependentProperty, ReadError> {
    let fields = dependent_fields(source, path, capture)?;
    let property = property(source, &fields)?
        .map(|value| {
            scoped_location_weak::read(reader, value).map_err(|()| {
                fields.invalid(
                    "property",
                    value,
                    "a nullable mission-property weak pointer",
                )
            })
        })
        .transpose()?
        .flatten();
    Ok(DependentProperty {
        property,
        values: mission_property_source::weaks(reader, source, "values", path, capture)?,
    })
}

pub(super) fn validate_metadata<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    let view = MetadataView::new(source, path, capture)?;
    view.text_id()?;
    view.weighting()?;
    view.forced()?;
    mission_property_source::visit_context(
        source,
        "dependentProperties",
        path,
        capture,
        |entry, path| {
            if let Some(source) = entry {
                let fields = dependent_fields(source, path, capture)?;
                property(source, &fields)?;
                if let Some(values) = fields.array("values")? {
                    for (index, value) in values.iter().enumerate() {
                        owned_reference_read::check(value).map_err(|()| {
                            fields.invalid(
                                &format!("values[{index}]"),
                                value,
                                "a nullable owner-local weak pointer",
                            )
                        })?;
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok(())
}
