// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::sound::radar_audio::{RadarAudioElement, RadarAudioField, RadarAudioValue};
use p4kx_extraction::{CancelToken, SourceValue, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::authored_xml_tree::{self, AuthoredXmlElement};
use crate::catalog_read::original::EntrySourceXml;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

/// The available selector evidence for an authored sound-event slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum RadarAudioBindingStatus {
    /// The slot has no authored value attribute.
    MissingValue,
    /// The slot has one explicitly empty value.
    EmptyValue,
    /// The slot has an ill-formed or multiply declared value.
    InvalidValue,
    /// The slot has a value with no evidence of the target selector or namespace.
    UnprovenSelector,
}

/// One typed reading of a radar-audio declaration.
#[derive(Debug, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum RadarAudioDatum {
    /// A present attribute with no text.
    Empty,
    /// The decoded original token.
    Text {
        /// The exact decoded token.
        #[schemars(extend("x-gameText" = true))]
        token: String,
    },
    /// An integral decimal token with no fixed integer storage width.
    Integer {
        /// The exact integral spelling, including any sign and leading zeros.
        #[schemars(regex(pattern = "^[+-]?[0-9]+$"))]
        decimal: String,
    },
    /// A finite parsed number whose original spelling remains in the authored tree.
    Number {
        /// The binary64 interpretation.
        value: f64,
    },
}

/// One sourced typed field of a recognised radar-audio element.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct RadarAudioCell {
    field: RadarAudioField,
    value: SourcedValue<RadarAudioDatum>,
}

impl RadarAudioCell {
    /// Returns the meaning of the field in the owning element.
    #[must_use]
    pub const fn field(&self) -> RadarAudioField {
        self.field
    }

    /// Borrows the typed value and its original source evidence.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<RadarAudioDatum> {
        &self.value
    }
}

/// One radar-audio configuration element and the recognised attributes it owns.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct RadarAudioNode {
    authored: AuthoredXmlElement,
    role: SourcedValue<Option<RadarAudioElement>>,
    fields: Vec<RadarAudioCell>,
    binding_status: Option<SourcedValue<RadarAudioBindingStatus>>,
}

impl RadarAudioNode {
    /// Borrows the physical element and all its original declarations.
    #[must_use]
    pub const fn authored(&self) -> &AuthoredXmlElement {
        &self.authored
    }

    /// Borrows the source-bound structural classification.
    #[must_use]
    pub const fn role(&self) -> &SourcedValue<Option<RadarAudioElement>> {
        &self.role
    }

    /// Lists the typed fields recognised for this element.
    #[must_use]
    pub fn fields(&self) -> &[RadarAudioCell] {
        &self.fields
    }

    /// Borrows the authored selector outcome of a sound-event element.
    #[must_use]
    pub const fn binding_status(&self) -> Option<&SourcedValue<RadarAudioBindingStatus>> {
        self.binding_status.as_ref()
    }
}

struct Projection {
    role: SourcedValue<Option<RadarAudioElement>>,
    fields: Vec<RadarAudioCell>,
}

pub(super) fn read(
    xml: &EntrySourceXml<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<RadarAudioNode>, CatalogError> {
    let mut projections = Vec::new();
    let authored = authored_xml_tree::visit(xml, cancel, |source, element| {
        let role = classify(xml, source, element, &projections)?;
        let fields = fields(xml, source, element, &role)?;
        projections.push(Projection { role, fields });
        Ok(())
    })?;
    authored
        .into_iter()
        .zip(projections)
        .map(|(authored, Projection { role, fields })| {
            let binding_status = fields
                .iter()
                .find(|field| field.field == RadarAudioField::SoundEventValue)
                .map(|field| binding(xml, &field.value))
                .transpose()?;
            Ok(RadarAudioNode {
                authored,
                role,
                fields,
                binding_status,
            })
        })
        .collect()
}

fn classify(
    xml: &EntrySourceXml<'_>,
    source: SourceElement<'_>,
    element: &AuthoredXmlElement,
    projected: &[Projection],
) -> Result<SourcedValue<Option<RadarAudioElement>>, CatalogError> {
    let at = element.position().origin();
    let name = element.name().origin();
    let parent = element
        .parent()
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let sources = xml.sources();
    let Some(index) = parent else {
        return Ok(sources.value_with_origins(
            SourceValue::Present(RadarAudioElement::root(source)),
            &[at, name],
        )?);
    };
    let index = usize::try_from(*index).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let owner = &projected
        .get(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?
        .role;
    let family = owner
        .value()
        .copied()
        .flatten()
        .and_then(|role| role.child(source));
    Ok(sources.value_with_origins(
        SourceValue::Present(family),
        &[at, name, element.parent().origin(), owner.origin()],
    )?)
}

fn fields(
    xml: &EntrySourceXml<'_>,
    source: SourceElement<'_>,
    element: &AuthoredXmlElement,
    role: &SourcedValue<Option<RadarAudioElement>>,
) -> Result<Vec<RadarAudioCell>, CatalogError> {
    let Some(family) = role.value().copied().flatten() else {
        return Ok(Vec::new());
    };
    let expected = expected_fields(family);
    if expected.is_empty() {
        return Ok(Vec::new());
    }
    let syntax = source
        .syntax()
        .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    let mut slots = vec![Vec::new(); expected.len()];
    for (index, declaration) in syntax.declarations().enumerate() {
        let declaration = declaration.map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let Some(observed) = family.attribute(&declaration) else {
            continue;
        };
        let slot = expected
            .iter()
            .position(|&field| field == observed.field)
            .and_then(|index| slots.get_mut(index))
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let authored = element
            .declarations()
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        slot.push((observed.value, authored.value().value().origin()));
    }
    expected
        .iter()
        .zip(slots)
        .map(|(&field, observed)| {
            let value = match observed.as_slice() {
                [] => xml.sources().value_with_origins(
                    SourceValue::Absent,
                    &[role.origin(), element.position().origin()],
                )?,
                [(value, origin)] => xml
                    .sources()
                    .value_with_origins(datum(*value), &[role.origin(), *origin])?,
                _ => {
                    let origins: Vec<_> = std::iter::once(role.origin())
                        .chain(observed.into_iter().map(|(_, origin)| origin))
                        .collect();
                    xml.sources()
                        .value_with_origins(SourceValue::Invalid, &origins)?
                }
            };
            Ok(RadarAudioCell { field, value })
        })
        .collect()
}

const fn expected_fields(family: RadarAudioElement) -> &'static [RadarAudioField] {
    use RadarAudioField::{
        CategoryName, CategoryValue, DisplayStartAlpha, DisplayTime, SoundEventName,
        SoundEventValue, ZoomMax, ZoomMin,
    };
    match family {
        RadarAudioElement::DefaultZoom => &[ZoomMin, ZoomMax],
        RadarAudioElement::CategoryType => {
            &[CategoryName, CategoryValue, DisplayTime, DisplayStartAlpha]
        }
        RadarAudioElement::SoundEvent => &[SoundEventName, SoundEventValue],
        _ => &[],
    }
}

fn datum(value: RadarAudioValue<'_>) -> SourceValue<RadarAudioDatum> {
    let value = match value {
        RadarAudioValue::Empty => RadarAudioDatum::Empty,
        RadarAudioValue::Text { raw } => RadarAudioDatum::Text {
            token: raw.to_owned(),
        },
        RadarAudioValue::Integer { raw } => RadarAudioDatum::Integer {
            decimal: raw.to_owned(),
        },
        RadarAudioValue::Number { value, .. } => RadarAudioDatum::Number { value },
        RadarAudioValue::Invalid { .. } => return SourceValue::Invalid,
        _ => return SourceValue::Unsupported,
    };
    SourceValue::Present(value)
}

fn binding(
    xml: &EntrySourceXml<'_>,
    value: &SourcedValue<RadarAudioDatum>,
) -> Result<SourcedValue<RadarAudioBindingStatus>, CatalogError> {
    let status = match value.value() {
        Some(RadarAudioDatum::Text { .. }) => RadarAudioBindingStatus::UnprovenSelector,
        Some(RadarAudioDatum::Empty) => RadarAudioBindingStatus::EmptyValue,
        None if value.state() == SourceValueState::Absent => RadarAudioBindingStatus::MissingValue,
        _ => RadarAudioBindingStatus::InvalidValue,
    };
    Ok(xml
        .sources()
        .value_with_origins(SourceValue::Present(status), &[value.origin()])?)
}
