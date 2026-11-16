// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::catalog_text_contract::GameTextRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::{FieldCapture, FieldError};

const DISPLAY: &str = "Localization.displayFeatures";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ManufacturerDisplayFeatures<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    callout1: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    callout2: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    callout3: SourcedValue<GameTextRef>,
    frontend_background: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    history: SourcedValue<GameTextRef>,
    location_audio_play_trigger: SourcedValue<ManufacturerAudioTrigger<'source>>,
    location_audio_stop_trigger: SourcedValue<ManufacturerAudioTrigger<'source>>,
    logo_simplified_white: SourcedValue<&'source str>,
    ui_priority: SourcedValue<i32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ManufacturerAudioTrigger<'source> {
    source_type: SourcedValue<&'source str>,
    audio_trigger: SourcedValue<&'source str>,
}

pub(super) fn read<'source>(
    source: &StructValue<'source>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    context: &CatalogRecordContext<'_>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ManufacturerDisplayFeatures<'source>, CatalogError> {
    let fields = CatalogFields::observed(source, DISPLAY, context, capture)?;
    fields.check_observed(
        &[
            "Callout1",
            "Callout2",
            "Callout3",
            "FrontendBackground",
            "History",
            "locationAudioPlayTrigger",
            "locationAudioStopTrigger",
            "LogoSimplifiedWhite",
            "UIPriority",
        ],
        notes,
    )?;
    let mut localized = |name| {
        projection.description(
            fields.locale(name, notes)?,
            record_index,
            &format!("{DISPLAY}.{name}"),
        )
    };
    let callout1 = localized("Callout1")?;
    let callout2 = localized("Callout2")?;
    let callout3 = localized("Callout3")?;
    let history = localized("History")?;
    Ok(ManufacturerDisplayFeatures {
        source_type: fields.source_type()?,
        callout1,
        callout2,
        callout3,
        frontend_background: fields.text("FrontendBackground", notes)?,
        history,
        location_audio_play_trigger: audio(
            &fields,
            "locationAudioPlayTrigger",
            context,
            capture,
            notes,
        )?,
        location_audio_stop_trigger: audio(
            &fields,
            "locationAudioStopTrigger",
            context,
            capture,
            notes,
        )?,
        logo_simplified_white: fields.text("LogoSimplifiedWhite", notes)?,
        ui_priority: fields.read(
            "UIPriority",
            "a nullable Int32",
            notes,
            |value| match value {
                Value::Int32(value) => Some(*value),
                _ => None,
            },
        )?,
    })
}

fn audio<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    name: &str,
    context: &CatalogRecordContext<'_>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ManufacturerAudioTrigger<'source>>, CatalogError> {
    fields.structure(name, notes)?.try_map(|source| {
        let path = format!("{DISPLAY}.{name}");
        let fields = CatalogFields::observed(source, &path, context, capture)?;
        fields.check_observed(&["audioTrigger"], notes)?;
        Ok(ManufacturerAudioTrigger {
            source_type: fields.source_type()?,
            audio_trigger: fields.text("audioTrigger", notes)?,
        })
    })
}
