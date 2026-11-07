// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_layers::key;
use crate::mission_property_option::{
    DependentProperty, FloatOption, IntegerOption, OptionMetadata, StringOption, TagOption,
};
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::scoped_location_weak;
use crate::source_fields::Fields;
use crate::spawn_read::Reader;
use crate::template_issues::TemplateError;
use crate::texts::Texts;

const BASE_FIELDS: [&str; 4] = [
    "textId",
    "weighting",
    "DEBUG_forceChooseThisOption",
    "dependentProperties",
];

fn check(source: &StructValue<'_>, expected: &str, fields: &[&str]) -> Result<(), ReadError> {
    if source.type_name() != expected {
        return Err(TemplateError::unknown("options", source).into());
    }
    for field in source.fields() {
        if !BASE_FIELDS.contains(&field.name()) && !fields.contains(&field.name()) {
            return Err(Box::new(
                Fields::new(source, "options").unsupported(field.name()),
            ));
        }
    }
    Ok(())
}

fn metadata<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<OptionMetadata<'f>, ReadError> {
    let fields = Fields::new(source, path);
    Ok(OptionMetadata {
        text_id: fields.locale("textId")?,
        display_name: texts.localised(key(Some(source), "textId")),
        weighting: fields.number("weighting")?,
        debug_force_choose_this_option: fields.boolean("DEBUG_forceChooseThisOption")?,
        dependent_properties: mission_property_source::list(
            source,
            "dependentProperties",
            path,
            |source, path| dependent(reader, source, path),
        )?,
    })
}

fn dependent(
    reader: &Reader<'_, '_>,
    source: &StructValue<'_>,
    path: &str,
) -> Result<DependentProperty, ReadError> {
    mission_property_source::check(source, path, &["property", "values"])?;
    let property = source
        .field("property")
        .map(|value| {
            scoped_location_weak::read(reader, value).map_err(|()| {
                Box::new(Fields::new(source, path).invalid(
                    "property",
                    value,
                    "a nullable mission-property weak pointer",
                )) as ReadError
            })
        })
        .transpose()?
        .flatten();
    Ok(DependentProperty {
        property,
        values: mission_property_source::weaks(reader, source, "values", path)?,
    })
}

pub(crate) fn string<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<StringOption<'f>, ReadError> {
    check(source, "MissionPropertyValueOption_StringHash", &["value"])?;
    Ok(StringOption {
        metadata: metadata(reader, source, path, texts)?,
        value: Fields::new(source, path).text("value")?.map(str::to_owned),
    })
}

pub(crate) fn integer<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<IntegerOption<'f>, ReadError> {
    check(
        source,
        "MissionPropertyValueOption_Integer",
        &["value", "variation"],
    )?;
    let fields = Fields::new(source, path);
    Ok(IntegerOption {
        metadata: metadata(reader, source, path, texts)?,
        value: fields.int32("value")?,
        variation: fields.int32("variation")?,
    })
}

pub(crate) fn float<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<FloatOption<'f>, ReadError> {
    check(
        source,
        "MissionPropertyValueOption_Float",
        &["value", "variation"],
    )?;
    let fields = Fields::new(source, path);
    Ok(FloatOption {
        metadata: metadata(reader, source, path, texts)?,
        value: fields.number("value")?,
        variation: fields.number("variation")?,
    })
}

pub(crate) fn tags<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<TagOption<'f>, ReadError> {
    check(
        source,
        "MissionPropertyValueOption_Tags",
        &["tags", "negativeTags"],
    )?;
    let fields = Fields::new(source, path);
    Ok(TagOption {
        metadata: metadata(reader, source, path, texts)?,
        tags: reader.tag_list(Some(source), "tags"),
        negative_tags: reader.tag_list(Some(source), "negativeTags"),
        source_tags: mission_property_source::tags(
            fields.structure("tags")?,
            &format!("{path}.tags"),
        )?,
        source_negative_tags: mission_property_source::tags(
            fields.structure("negativeTags")?,
            &format!("{path}.negativeTags"),
        )?,
    })
}
