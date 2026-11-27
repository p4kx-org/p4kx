// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{Format, Signature, SourceElement};
use p4kx_extraction::{CancelToken, Guid, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::control;
use super::control::SubsumptionEventControl;
use crate::catalog_read::authored_xml_tree::{self, AuthoredXmlElement};
use crate::catalog_read::original::{EntrySourceXml, PhysicalEntryIdentity};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

pub(super) const ROOT: &str = "Subsumption";
const EVENT: &str = "Event";
const ID: &str = "ID";

/// Encoding of the original Subsumption event document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum SubsumptionEventFormat {
    /// Text XML.
    TextXml,
    /// Binary CryXmlB.
    CryXmlB,
    /// Binary CryXml.
    CryXml,
    /// Binary CRY3SDK.
    Cry3Sdk,
}

impl From<Format> for SubsumptionEventFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::Text => Self::TextXml,
            Format::CryXml(Signature::CryXmlB) => Self::CryXmlB,
            Format::CryXml(Signature::CryXml) => Self::CryXml,
            Format::CryXml(Signature::Cry3Sdk) => Self::Cry3Sdk,
        }
    }
}

/// A physical XML document with its original markup and ordered event declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SubsumptionEventDocument {
    identity: PhysicalEntryIdentity,
    source_format: SourcedValue<SubsumptionEventFormat>,
    elements: Vec<AuthoredXmlElement>,
    events: Vec<SubsumptionEvent>,
}

impl SubsumptionEventDocument {
    /// Borrows the original physical Archive Entry identity.
    #[must_use]
    pub const fn identity(&self) -> &PhysicalEntryIdentity {
        &self.identity
    }

    /// Borrows the original encoding detection and its evidence.
    #[must_use]
    pub const fn source_format(&self) -> &SourcedValue<SubsumptionEventFormat> {
        &self.source_format
    }

    /// Lists the original elements in preorder with their attributes and direct text.
    #[must_use]
    pub fn elements(&self) -> &[AuthoredXmlElement] {
        &self.elements
    }

    /// Lists the direct authored event declarations in document order.
    #[must_use]
    pub fn events(&self) -> &[SubsumptionEvent] {
        &self.events
    }
}

/// A direct Event declaration positioned within its physical document.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SubsumptionEvent {
    element_index: SourcedValue<u32>,
    id: SourcedValue<CatalogGuid>,
    #[schemars(extend("x-gameText" = true))]
    name: SourcedValue<String>,
    controls: Vec<SubsumptionEventControl>,
}

impl SubsumptionEvent {
    /// Borrows the document-local preorder index and source evidence of the event.
    #[must_use]
    pub const fn element_index(&self) -> &SourcedValue<u32> {
        &self.element_index
    }

    /// Borrows the authored event identifier.
    #[must_use]
    pub const fn id(&self) -> &SourcedValue<CatalogGuid> {
        &self.id
    }

    /// Borrows the authored event symbol.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Lists the typed known controls and explicit source states.
    #[must_use]
    pub fn controls(&self) -> &[SubsumptionEventControl] {
        &self.controls
    }
}

pub(super) fn read(
    xml: &EntrySourceXml<'_>,
    source_format: SourcedValue<SubsumptionEventFormat>,
    cancel: Option<&CancelToken>,
) -> Result<SubsumptionEventDocument, CatalogError> {
    let mut index = 0_u32;
    let mut events = Vec::new();
    let elements = authored_xml_tree::visit(xml, cancel, |source, element| {
        if element.parent().value() == Some(&Some(0)) && is_event(source) {
            events.push(SubsumptionEvent {
                element_index: element.position().map_ref(|_| index),
                id: control::attribute(xml, element, ID, |value| {
                    guid(value)
                        .map(CatalogGuid)
                        .map_or(SourceValue::Invalid, SourceValue::Present)
                })?,
                name: control::attribute(xml, element, "Name", |value| {
                    SourceValue::Present(value.to_owned())
                })?,
                controls: control::read(xml, element)?,
            });
        }
        index = index
            .checked_add(1)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        Ok(())
    })?;
    Ok(SubsumptionEventDocument {
        identity: xml.identity()?,
        source_format,
        elements,
        events,
    })
}

pub(super) fn contains_id(root: SourceElement<'_>, id: Guid) -> bool {
    root.elements().filter(|node| is_event(*node)).any(|node| {
        let mut values = node
            .attributes()
            .filter(|attribute| attribute.name() == ID && attribute.namespace().is_none());
        matches!(
            (values.next(), values.next()),
            (Some(value), None) if guid(value.value()) == Some(id)
        )
    })
}

fn is_event(node: SourceElement<'_>) -> bool {
    node.name() == EVENT && node.namespace().is_none()
}

fn guid(value: &str) -> Option<Guid> {
    let value = value
        .strip_prefix('{')
        .and_then(|tail| tail.strip_suffix('}'))
        .unwrap_or(value);
    value.parse().ok()
}
