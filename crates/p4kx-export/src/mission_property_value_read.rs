// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_auto_read;
use crate::mission_property_investigation_read;
use crate::mission_property_option_read;
use crate::mission_property_race;
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldChecks};
use crate::spawn_read::Reader;
use crate::template_property::PropertyValue;
use crate::template_property_kind::PropertyValueKind;
use crate::texts::Texts;

pub(crate) fn read<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    texts: &Texts<'f>,
    capture: Option<&dyn FieldCapture<'f>>,
) -> Result<Option<PropertyValue<'f>>, ReadError> {
    let value = match PropertyValueKind::read(source.type_name()) {
        Some(PropertyValueKind::AutoSpawns) => {
            PropertyValue::AutoSpawns(Box::new(mission_property_auto_read::read(source, capture)?))
        }
        Some(PropertyValueKind::Investigation) => PropertyValue::Investigation(Box::new(
            mission_property_investigation_read::read(reader, source, texts, capture)?,
        )),
        Some(PropertyValueKind::StringHash) => PropertyValue::StringHash {
            options: options(source, capture, |value, path| {
                mission_property_option_read::string(reader, value, path, texts, capture)
            })?,
        },
        Some(PropertyValueKind::Integer) => PropertyValue::Integer {
            options: options(source, capture, |value, path| {
                mission_property_option_read::integer(reader, value, path, texts, capture)
            })?,
        },
        Some(PropertyValueKind::Float) => PropertyValue::Float {
            options: options(source, capture, |value, path| {
                mission_property_option_read::float(reader, value, path, texts, capture)
            })?,
        },
        Some(PropertyValueKind::Tags) => tags(reader, source, texts, capture)?,
        Some(PropertyValueKind::TimeTrialRace) => PropertyValue::TimeTrialRace {
            target_splits: mission_property_race::read(source, capture)?,
        },
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn options<'f, T>(
    source: &StructValue<'f>,
    capture: Option<&dyn FieldCapture<'f>>,
    read: impl FnMut(&StructValue<'f>, &str) -> Result<T, ReadError>,
) -> Result<Option<Vec<Option<T>>>, ReadError> {
    mission_property_source::check_context(source, "", &["options"], capture)?;
    mission_property_source::list(source, "options", "", capture, read)
}

fn tags<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    texts: &Texts<'f>,
    capture: Option<&dyn FieldCapture<'f>>,
) -> Result<PropertyValue<'f>, ReadError> {
    mission_property_source::check_context(
        source,
        "",
        &["tags", "negativeTags", "options"],
        capture,
    )?;
    let fields = FieldChecks::for_context(source, "", capture)?;
    Ok(PropertyValue::Tags {
        tags: reader.tag_list(Some(source), "tags"),
        negative_tags: reader.tag_list(Some(source), "negativeTags"),
        source_tags: mission_property_source::tags_context(
            fields.structure("tags")?,
            "tags",
            capture,
        )?,
        source_negative_tags: mission_property_source::tags_context(
            fields.structure("negativeTags")?,
            "negativeTags",
            capture,
        )?,
        options: mission_property_source::list(source, "options", "", capture, |value, path| {
            mission_property_option_read::tags(reader, value, path, texts, capture)
        })?,
    })
}
