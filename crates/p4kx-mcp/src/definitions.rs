// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_extraction::{DataForge, DataForgeError, DataForgeOrigin, Enum, Filter, Property, Struct};
use serde::Serialize;

use crate::budget::{fill_within, fit_prefix, json_len};
use crate::definition_origins::{self, Declaration, Kind, PropertySite, Target};
use crate::error::ToolError;
use crate::forge_shapes::{
    EnumShape, EnumsResult, FieldShape, ForgeFieldOrigins, StructShape, StructsResult,
};
use crate::page::Page;

/// Returns the matching struct definitions in case-insensitive name order.
pub(crate) fn structs(
    forge: &DataForge<'_>,
    archive: String,
    origin: &DataForgeOrigin,
    name: Option<&str>,
    page: Page,
) -> Result<StructsResult, ToolError> {
    let definitions = forge.structs();
    let listing = Listing {
        archive: &archive,
        origin,
        page,
        kind: Kind::Structs,
        definitions: definitions.len(),
        filtered: name.is_some(),
    };
    let named = matching(definitions, name, Struct::name)?;
    let items = page
        .slice(&named)
        .iter()
        .map(|(name, definition)| struct_shape(name, definition));
    let selected = listing.fill(named.len(), items, |value| &value.name)?;
    Ok(StructsResult {
        archive,
        total: named.len(),
        structs: selected.values,
        next_cursor: selected.next_cursor,
        origins: selected.origins,
    })
}

/// Returns the matching enum definitions in case-insensitive name order.
pub(crate) fn enums(
    forge: &DataForge<'_>,
    archive: String,
    origin: &DataForgeOrigin,
    name: Option<&str>,
    page: Page,
) -> Result<EnumsResult, ToolError> {
    let definitions = forge.enums();
    let listing = Listing {
        archive: &archive,
        origin,
        page,
        kind: Kind::Enums,
        definitions: definitions.len(),
        filtered: name.is_some(),
    };
    let named = matching(definitions, name, Enum::name)?;
    let items = page
        .slice(&named)
        .iter()
        .map(|(name, definition)| enum_shape(name, definition));
    let selected = listing.fill(named.len(), items, |value| &value.name)?;
    Ok(EnumsResult {
        archive,
        total: named.len(),
        enums: selected.values,
        next_cursor: selected.next_cursor,
        origins: selected.origins,
    })
}

#[derive(Serialize)]
struct Observed<T> {
    #[serde(flatten)]
    value: T,
    #[serde(skip)]
    declaration: Declaration,
}

struct Listing<'a> {
    archive: &'a str,
    origin: &'a DataForgeOrigin,
    page: Page,
    kind: Kind,
    definitions: usize,
    filtered: bool,
}

struct Selected<T> {
    values: Vec<T>,
    next_cursor: Option<String>,
    origins: ForgeFieldOrigins,
}

#[derive(Serialize)]
struct MeasuredPage<'a, T> {
    archive: &'a str,
    total: usize,
    #[serde(flatten)]
    values: NamedValues<'a, T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    #[serde(flatten)]
    origins: ForgeFieldOrigins,
}

#[derive(Serialize)]
#[serde(untagged)]
enum NamedValues<'a, T> {
    Structs { structs: &'a [Observed<T>] },
    Enums { enums: &'a [Observed<T>] },
}

impl Listing<'_> {
    fn fill<T: Serialize>(
        &self,
        total: usize,
        items: impl IntoIterator<Item = Result<Observed<T>, ToolError>>,
        name: impl Fn(&T) -> &str,
    ) -> Result<Selected<T>, ToolError> {
        let reserved = json_len(&(self.archive, total));
        let (rows, _) = fill_within(self.page, total, reserved, items, |row| {
            format!("{} {}", self.kind.singular(), name(&row.value))
        })?;
        let (keep, measured) = fit_prefix(
            rows.len(),
            |len| {
                let next_cursor = self.page.next_cursor_after(len, total)?;
                let selected = rows.split_at(len).0;
                let origins = definition_origins::project(
                    self.origin,
                    self.kind,
                    self.definitions,
                    self.filtered,
                    next_cursor.is_some(),
                    selected.iter().map(|row| &row.declaration),
                )?;
                let values = match self.kind {
                    Kind::Structs => NamedValues::Structs { structs: selected },
                    Kind::Enums => NamedValues::Enums { enums: selected },
                };
                Ok(MeasuredPage {
                    archive: self.archive,
                    total,
                    values,
                    next_cursor,
                    origins,
                })
            },
            || {
                rows.first().map_or_else(
                    || self.kind.list().to_owned(),
                    |row| format!("{} {}", self.kind.singular(), name(&row.value)),
                )
            },
        )?;
        let MeasuredPage {
            next_cursor,
            origins,
            ..
        } = measured;
        Ok(Selected {
            values: rows.into_iter().take(keep).map(|row| row.value).collect(),
            next_cursor,
            origins,
        })
    }
}

fn matching<'f, T>(
    definitions: impl Iterator<Item = T>,
    filter: Option<&str>,
    name: impl Fn(&T) -> Result<&'f str, DataForgeError>,
) -> Result<Vec<(&'f str, T)>, ToolError> {
    let filter = filter.map(Filter::parse).transpose()?;
    let mut named = Vec::new();
    for definition in definitions {
        let text = name(&definition)?;
        if filter.as_ref().is_none_or(|filter| filter.matches(text)) {
            named.push((text, definition));
        }
    }
    named.sort_by(|left, right| compare_ignoring_case(left.0, right.0));
    Ok(named)
}

fn struct_shape(name: &str, definition: &Struct<'_>) -> Result<Observed<StructShape>, ToolError> {
    let parent = definition
        .parent()
        .map(|parent| parent.name().map(|name| (parent.index(), name.to_owned())))
        .transpose()?;
    let mut fields = Vec::new();
    let mut properties = Vec::new();
    for property in definition.properties() {
        let (value, site) = field_shape(&property)?;
        fields.push(value);
        properties.push(site);
    }
    let parent_index = parent.as_ref().map(|(index, _)| *index);
    Ok(Observed {
        value: StructShape {
            name: name.to_owned(),
            parent: parent.map(|(_, name)| name),
            fields,
        },
        declaration: Declaration::Struct {
            index: definition.index(),
            parent: parent_index,
            range: definition.property_range(),
            properties,
        },
    })
}

fn field_shape(property: &Property<'_>) -> Result<(FieldShape, PropertySite), ToolError> {
    let target = property.target_struct();
    let enumeration = property.target_enum();
    Ok((
        FieldShape {
            name: property.name()?.to_owned(),
            data_type: property.data_type()?.to_string(),
            array: property.is_array(),
            target: target
                .map(|target| target.name().map(str::to_owned))
                .transpose()?,
            enum_name: enumeration
                .map(|target| target.name().map(str::to_owned))
                .transpose()?,
        },
        PropertySite {
            index: property.index(),
            target: target
                .map(|target| Target::Struct(target.index()))
                .or_else(|| enumeration.map(|target| Target::Enum(target.index()))),
        },
    ))
}

fn enum_shape(name: &str, definition: &Enum<'_>) -> Result<Observed<EnumShape>, ToolError> {
    Ok(Observed {
        value: EnumShape {
            name: name.to_owned(),
            values: definition
                .options()
                .map(|option| option.map(str::to_owned))
                .collect::<Result<_, _>>()?,
        },
        declaration: Declaration::Enum {
            index: definition.index(),
            options: definition.option_range(),
        },
    })
}

fn compare_ignoring_case(left: &str, right: &str) -> Ordering {
    let key = |byte: u8| byte.to_ascii_lowercase();
    left.bytes().map(key).cmp(right.bytes().map(key))
}
