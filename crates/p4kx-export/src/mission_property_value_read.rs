// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_auto_read;
use crate::mission_property_investigation_read;
use crate::mission_property_option_read;
use crate::mission_property_race;
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::Fields;
use crate::spawn_read::Reader;
use crate::template_property::PropertyValue;
use crate::texts::Texts;

pub(crate) fn read<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    texts: &Texts<'f>,
) -> Result<Option<PropertyValue<'f>>, ReadError> {
    let value = match source.type_name() {
        "MissionPropertyValue_AutoSpawns" => {
            PropertyValue::AutoSpawns(Box::new(mission_property_auto_read::read(source)?))
        }
        "MissionPropertyValue_Investigation" => PropertyValue::Investigation(Box::new(
            mission_property_investigation_read::read(reader, source, texts)?,
        )),
        "MissionPropertyValue_StringHash" => PropertyValue::StringHash {
            options: options(source, |value, path| {
                mission_property_option_read::string(reader, value, path, texts)
            })?,
        },
        "MissionPropertyValue_Integer" => PropertyValue::Integer {
            options: options(source, |value, path| {
                mission_property_option_read::integer(reader, value, path, texts)
            })?,
        },
        "MissionPropertyValue_Float" => PropertyValue::Float {
            options: options(source, |value, path| {
                mission_property_option_read::float(reader, value, path, texts)
            })?,
        },
        "MissionPropertyValue_Tags" => tags(reader, source, texts)?,
        "MissionPropertyValue_TimeTrialRace" => PropertyValue::TimeTrialRace {
            target_splits: mission_property_race::read(source)?,
        },
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn options<'f, T>(
    source: &StructValue<'f>,
    read: impl FnMut(&StructValue<'f>, &str) -> Result<T, ReadError>,
) -> Result<Option<Vec<Option<T>>>, ReadError> {
    mission_property_source::check(source, "", &["options"])?;
    mission_property_source::list(source, "options", "", read)
}

fn tags<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    texts: &Texts<'f>,
) -> Result<PropertyValue<'f>, ReadError> {
    mission_property_source::check(source, "", &["tags", "negativeTags", "options"])?;
    let fields = Fields::new(source, "");
    Ok(PropertyValue::Tags {
        tags: reader.tag_list(Some(source), "tags"),
        negative_tags: reader.tag_list(Some(source), "negativeTags"),
        source_tags: mission_property_source::tags(fields.structure("tags")?, "tags")?,
        source_negative_tags: mission_property_source::tags(
            fields.structure("negativeTags")?,
            "negativeTags",
        )?,
        options: mission_property_source::list(source, "options", "", |value, path| {
            mission_property_option_read::tags(reader, value, path, texts)
        })?,
    })
}
