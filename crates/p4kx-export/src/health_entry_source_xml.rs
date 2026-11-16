// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceDocument, SourceElement, SourceElementPosition, SourceReadError,
    SourceReadOptions,
};
use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::health_entry_metadata::EntryMetadata;
use super::{EntryCapture, EntryRead, EntryReceipt};
use crate::{CatalogError, ExportAccountingError};

/// One original XML attribute and its spelling, namespace and value.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub(crate) struct SourceXmlAttribute {
    pub(crate) name: SourcedValue<String>,
    pub(crate) namespace: SourcedValue<Option<String>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) value: SourcedValue<String>,
}

/// One complete XML read and its actual Archive Entry evidence.
pub(crate) struct EntrySourceXml<'read> {
    document: SourceDocument<'read>,
    receipt: &'read EntryReceipt,
    entry: &'read EntryMetadata,
}

impl EntryReceipt {
    pub(crate) fn payload_read_result<T>(
        &self,
        value: SourceValue<T>,
        read_succeeded: bool,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let role = if read_succeeded {
            SourceUseRole::Conversion
        } else {
            SourceUseRole::Attempted
        };
        let source = self.payload_entry()?.payload_read(role)?;
        Ok(self.source_uses().value(value, &[source], read_succeeded))
    }
}

impl<'read> EntrySourceXml<'read> {
    pub(crate) fn parse(
        read: &'read EntryRead,
        capture: &EntryCapture<'_>,
    ) -> Result<Result<Self, SourceReadError>, CatalogError> {
        read.receipt().check_capture(capture)?;
        let document = match SourceDocument::parse(read.bytes(), SourceReadOptions::default()) {
            Ok(document) => document,
            Err(error) => return Ok(Err(error)),
        };
        Ok(Ok(Self {
            document,
            receipt: read.receipt(),
            entry: read.receipt().payload_entry()?,
        }))
    }

    pub(crate) fn identity(&self) -> Result<super::PhysicalEntryIdentity, CatalogError> {
        self.receipt
            .physical_payload_file(|name| name.replace('\\', "/"))
    }

    pub(crate) fn sources(&self) -> &p4kx_extraction::SourceUses {
        self.receipt.source_uses()
    }

    pub(crate) fn root(&self) -> SourceElement<'_> {
        self.document.root()
    }

    pub(crate) fn position(
        &self,
        node: SourceElement<'_>,
    ) -> Result<SourcedValue<SourceLocation>, CatalogError> {
        let location = self.node_location(node, "$node")?;
        let proof = self
            .entry
            .payload_at(location.clone(), SourceUseRole::Selector)?;
        Ok(self.project(SourceValue::Present(location), &[proof]))
    }

    pub(crate) fn name(
        &self,
        node: SourceElement<'_>,
    ) -> Result<SourcedValue<String>, CatalogError> {
        let proof = self
            .entry
            .payload_at(self.node_location(node, "$name")?, SourceUseRole::Payload)?;
        Ok(self.project(SourceValue::Present(node.name().to_owned()), &[proof]))
    }

    pub(crate) fn attributes(
        &self,
        node: SourceElement<'_>,
    ) -> Result<Vec<SourceXmlAttribute>, CatalogError> {
        if !self.document.owns_element(node) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        node.attributes()
            .map(|attribute| {
                let location = attribute_location(attribute.position(), attribute.name())?;
                let selector = self
                    .entry
                    .payload_at(location.clone(), SourceUseRole::Selector)?;
                let value = self.entry.payload_at(location, SourceUseRole::Payload)?;
                let name = self.project(
                    SourceValue::Present(attribute.name().to_owned()),
                    &[selector],
                );
                let namespace = name.map_ref(|_| attribute.namespace().map(str::to_owned));
                Ok(SourceXmlAttribute {
                    name,
                    namespace,
                    value: self
                        .project(SourceValue::Present(attribute.value().to_owned()), &[value]),
                })
            })
            .collect()
    }

    fn project<T>(&self, value: SourceValue<T>, references: &[SourceUse]) -> SourcedValue<T> {
        self.receipt
            .source_uses()
            .value(value, references, !self.document.is_lossy())
    }

    fn node_location(
        &self,
        node: SourceElement<'_>,
        field: &str,
    ) -> Result<SourceLocation, CatalogError> {
        if !self.document.owns_element(node) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let (shape, coordinates) = match node.position() {
            SourceElementPosition::Text(range) => (
                "text",
                vec![coordinate(range.start)?, coordinate(range.end)?],
            ),
            SourceElementPosition::CryXml(node) => ("cryXml", vec![u64::from(node)]),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(location(shape, coordinates, field.to_owned()))
    }
}

fn attribute_location(
    position: &SourceAttributePosition,
    name: &str,
) -> Result<SourceLocation, CatalogError> {
    let (shape, coordinates) = match position {
        SourceAttributePosition::Text(range) => (
            "text",
            vec![coordinate(range.start)?, coordinate(range.end)?],
        ),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
        } => (
            "cryXml",
            vec![u64::from(*node_index), u64::from(*attribute_index)],
        ),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    Ok(location(shape, coordinates, format!("@{name}")))
}

fn location(shape: &str, indices: Vec<u64>, field: String) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: format!("xmlSource.{shape}"),
        indices,
        field,
    }
}
fn coordinate(value: usize) -> Result<u64, ExportAccountingError> {
    u64::try_from(value).map_err(|_| ExportAccountingError::CounterOverflow)
}

#[cfg(test)]
#[path = "health_entry_source_xml_tests.rs"]
mod tests;
