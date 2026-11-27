// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Entry, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::authored_xml_tree::AuthoredXmlElement;
use crate::catalog_read::loose_archive_path::{LooseArchivePath, Resolver};
use crate::catalog_read::original::EntrySourceXml;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;
use crate::{CatalogError, ExportAccountingError};

#[path = "flare_parameter_shape.rs"]
mod shape;

use shape::FieldKind;

/// The authored context of an element in a lens flare library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum FlareNodeRole {
    /// The library root.
    Library,
    /// A direct library item.
    Item,
    /// A component or group inside another flare item.
    Component,
    /// The parameter container of an item or component.
    Parameters,
    /// A parameter group inside a parameter container.
    ParameterGroup,
    /// An element outside the recognised grammar.
    Other,
}

impl FlareNodeRole {
    pub(super) fn child(self, name: &str) -> Self {
        match (self, name) {
            (Self::Library, "FlareItem") => Self::Item,
            (Self::Item | Self::Component, "FlareItem") => Self::Component,
            (Self::Item | Self::Component, "Params") => Self::Parameters,
            (Self::Parameters, _) => Self::ParameterGroup,
            _ => Self::Other,
        }
    }

    fn fields(self, name: &str) -> shape::Fields {
        match self {
            Self::Library => shape::LIBRARY,
            Self::Item => shape::ITEM,
            Self::Component => shape::COMPONENT,
            Self::ParameterGroup => shape::group(name),
            Self::Parameters | Self::Other => &[],
        }
    }
}

/// A typed lexical interpretation of a supported flare parameter.
#[derive(Debug, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[non_exhaustive]
pub enum FlareValue {
    /// An explicitly empty attribute.
    Empty,
    /// An authored text parameter.
    Text(#[schemars(extend("x-gameText" = true))] String),
    /// A boolean token.
    Boolean(bool),
    /// A signed integer token.
    Integer(i64),
    /// A finite scalar token.
    Number(f64),
    /// Two finite scalar tokens in original order.
    Pair([f64; 2]),
    /// Three finite scalar tokens in original order.
    Triple([f64; 3]),
    /// A canonical GUID with the original spelling in the authored tree.
    Guid(CatalogGuid),
}

/// The interpreted value and optional directory-only target of a supported attribute.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct FlareField {
    key: &'static str,
    value: SourcedValue<FlareValue>,
    texture: Option<LooseArchivePath>,
}

impl FlareField {
    /// Returns the exact attribute key of the interpreted field.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }

    /// Borrows the interpreted value state and its physical source evidence.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<FlareValue> {
        &self.value
    }

    /// Borrows the directory outcome of an authored texture field.
    #[must_use]
    pub const fn texture(&self) -> Option<&LooseArchivePath> {
        self.texture.as_ref()
    }
}

pub(super) fn read(
    xml: &EntrySourceXml<'_>,
    element: &AuthoredXmlElement,
    role: &SourcedValue<FlareNodeRole>,
    resolver: &mut Resolver<'_, '_, '_>,
) -> Result<Vec<FlareField>, CatalogError> {
    let name = element
        .name()
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let family = role
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    family
        .fields(name)
        .iter()
        .map(|(key, kind)| field(xml, element, role, key, *kind, resolver))
        .collect()
}

fn field(
    xml: &EntrySourceXml<'_>,
    element: &AuthoredXmlElement,
    role: &SourcedValue<FlareNodeRole>,
    key: &'static str,
    kind: FieldKind,
    resolver: &mut Resolver<'_, '_, '_>,
) -> Result<FlareField, CatalogError> {
    let mut occurrences = element
        .declarations()
        .iter()
        .filter(|entry| entry.name().value().is_some_and(|name| name == key))
        .map(|entry| entry.value().value());
    let sources = xml.sources();
    let (value, texture) = match (occurrences.next(), occurrences.next()) {
        (None, _) => {
            let raw =
                sources.value_with_origins(SourceValue::Absent, &[element.position().origin()])?;
            typed_field(xml, role, &raw, kind, resolver)?
        }
        (Some(raw), None) => typed_field(xml, role, raw, kind, resolver)?,
        (Some(first), Some(second)) => {
            let origins: Vec<_> = [first, second]
                .into_iter()
                .chain(occurrences)
                .map(SourcedValue::origin)
                .collect();
            let raw = sources.value_with_origins(SourceValue::Invalid, &origins)?;
            typed_field(xml, role, &raw, kind, resolver)?
        }
    };
    Ok(FlareField {
        key,
        value,
        texture,
    })
}

fn typed_field(
    xml: &EntrySourceXml<'_>,
    role: &SourcedValue<FlareNodeRole>,
    raw: &SourcedValue<String>,
    kind: FieldKind,
    resolver: &mut Resolver<'_, '_, '_>,
) -> Result<(SourcedValue<FlareValue>, Option<LooseArchivePath>), CatalogError> {
    let sources = xml.sources();
    let value = match raw.value() {
        None => raw.map_ref(|_| FlareValue::Empty),
        Some(text) => {
            sources.value_with_origins(interpret(kind, text), &[raw.origin(), role.origin()])?
        }
    };
    let texture = match (kind, raw.value()) {
        (FieldKind::Texture, Some(text)) if !text.is_empty() && !texture_path(text) => {
            let unknown = sources.value_with_origins(SourceValue::Invalid, &[raw.origin()])?;
            Some(resolver.resolve(&unknown, texture_entry)?)
        }
        (FieldKind::Texture, _) => Some(resolver.resolve(raw, texture_entry)?),
        _ => None,
    };
    Ok((value, texture))
}

fn interpret(kind: FieldKind, text: &str) -> SourceValue<FlareValue> {
    if text.is_empty() {
        return SourceValue::Present(FlareValue::Empty);
    }
    let token = text.trim();
    let value = match kind {
        FieldKind::Text | FieldKind::Texture => Some(FlareValue::Text(text.to_owned())),
        FieldKind::Boolean => match token {
            "1" | "true" => Some(FlareValue::Boolean(true)),
            "0" | "false" => Some(FlareValue::Boolean(false)),
            _ => None,
        },
        FieldKind::Integer => token.parse().ok().map(FlareValue::Integer),
        FieldKind::Number => finite(token).map(FlareValue::Number),
        FieldKind::Pair => tuple(token).map(FlareValue::Pair),
        FieldKind::Triple => tuple(token).map(FlareValue::Triple),
        FieldKind::Guid => {
            let token = token
                .strip_prefix('{')
                .and_then(|body| body.strip_suffix('}'))
                .unwrap_or(token);
            token.parse().ok().map(CatalogGuid).map(FlareValue::Guid)
        }
    };
    value.map_or(SourceValue::Invalid, SourceValue::Present)
}

fn finite(token: &str) -> Option<f64> {
    token.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn tuple<const N: usize>(token: &str) -> Option<[f64; N]> {
    let mut parts = token.split(',');
    let mut values = [0.0; N];
    for value in &mut values {
        *value = finite(parts.next()?.trim())?;
    }
    parts.next().is_none().then_some(values)
}

fn texture_entry(entry: &Entry) -> bool {
    texture_path(entry.path())
}

fn texture_path(path: &str) -> bool {
    let relative = path
        .split_once(['/', '\\'])
        .filter(|(root, _)| root.eq_ignore_ascii_case("data"))
        .map_or(path, |(_, relative)| relative);
    ["textures/", "objects/", "engineassets/", "engine/"]
        .into_iter()
        .any(|folder| starts_with_folder(relative, folder))
        && relative.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("dds") || extension.eq_ignore_ascii_case("tif")
        })
}
