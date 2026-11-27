// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::OriginalDatum;
use super::health_entry_frame::EntryCapture;
use super::health_entry_metadata::EntryMetadata;
use super::health_entry_read::EntryRead;
use super::health_entry_receipt::{EntryReceipt, PhysicalEntryRef};
use super::health_entry_selection::AuthoredEntryPath;
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Document, Node, Root, XmlError};
use crate::{CatalogError, ExportAccountingError};

#[path = "health_entry_xml_attribute.rs"]
mod attribute;

pub(crate) use attribute::XmlAttribute;

#[path = "health_entry_xml_deferred.rs"]
mod deferred;
pub(crate) use deferred::DeferredXmlNode;

/// One original XML attribute occurrence.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SourcedXmlAttribute {
    /// Original attribute name.
    pub name: SourcedValue<String>,
    /// Original attribute text.
    #[schemars(extend("x-gameText" = true))]
    pub value: SourcedValue<String>,
}

/// One actual parse borrowing only bytes from its sealed original Entry read.
pub(crate) struct EntryXml<'read, 'capture, 'input> {
    read: &'read EntryRead,
    entry: Arc<EntryMetadata>,
    capture: &'capture EntryCapture<'input>,
    document: Document<'read>,
}

impl<'read, 'capture, 'input> EntryXml<'read, 'capture, 'input> {
    pub(crate) fn parse(
        read: &'read EntryRead,
        capture: &'capture EntryCapture<'input>,
    ) -> Result<Result<Self, XmlError>, CatalogError> {
        Self::parse_using(read, capture, Document::parse)
    }

    /// Opens the Starmap main descriptor from its original sealed Entry.
    pub(crate) fn parse_starmap_main(
        read: &'read EntryRead,
        capture: &'capture EntryCapture<'input>,
    ) -> Result<Result<Self, XmlError>, CatalogError> {
        Self::parse_using(read, capture, Document::parse_starmap_main)
    }

    fn parse_using(
        read: &'read EntryRead,
        capture: &'capture EntryCapture<'input>,
        parser: impl FnOnce(&'read [u8]) -> Result<Document<'read>, XmlError>,
    ) -> Result<Result<Self, XmlError>, CatalogError> {
        read.receipt().check_frame(&capture.frame())?;
        let document = match parser(read.bytes()) {
            Ok(document) => document,
            Err(error) => return Ok(Err(error)),
        };
        Ok(Ok(Self {
            read,
            entry: Arc::new(read.receipt().payload_entry()?.clone()),
            capture,
            document,
        }))
    }

    pub(crate) fn containing_entry(&self) -> Result<EntryReceipt, ExportAccountingError> {
        self.read.receipt().containing_entry()
    }

    pub(crate) fn payload_reference(
        &self,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.read.receipt().payload_reference(self.capture)
    }

    pub(crate) const fn is_lossy(&self) -> bool {
        matches!(&self.document, Document::Cry(document) if document.is_lossy())
    }

    pub(crate) const fn capture(&self) -> &EntryCapture<'input> {
        self.capture
    }

    /// Selects the original content of this successfully parsed physical document.
    pub(crate) fn document_source(&self) -> Result<SourceUse, CatalogError> {
        self.read.complete_bytes()?;
        self.entry.payload_read(SourceUseRole::Selector)
    }

    pub(crate) fn root(&self) -> Root<'_> {
        self.document.root()
    }

    pub(crate) fn node_source<'node>(
        &self,
        node: impl Node<'node>,
    ) -> Result<XmlNodeSource, CatalogError> {
        let (_, index) = self.checked_node(node)?;
        Ok(XmlNodeSource {
            entry: Arc::clone(&self.entry),
            index,
        })
    }

    pub(crate) fn attribute_source<'node>(
        &self,
        node: impl Node<'node>,
        field: &'static str,
    ) -> Result<XmlAttributeSource, CatalogError> {
        Ok(XmlAttributeSource {
            node: self.node_source(node)?,
            slot: XmlAttributeSlot::capture(node, field)?,
        })
    }

    pub(crate) fn attribute<'node>(
        &self,
        node: impl Node<'node>,
        name: &str,
    ) -> Result<Option<AuthoredEntryPath>, CatalogError> {
        let (node, index) = self.checked_node(node)?;
        let Some((name, value)) = node
            .attributes()
            .find(|(have, _)| have.eq_ignore_ascii_case(name))
        else {
            return Ok(None);
        };
        self.capture_value(index, &format!("@{name}"), value)
            .map(Some)
    }

    /// Visits each actual original attribute once in source order.
    pub(crate) fn visit_attributes<'node>(
        &self,
        node: impl Node<'node>,
        mut visit: impl FnMut(XmlAttribute<'_, 'node>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        let (node, index) = self.checked_node(node)?;
        for (ordinal, (name, value)) in node.attributes().enumerate() {
            visit(XmlAttribute::new(
                self.entry.as_ref(),
                index,
                ordinal,
                name,
                value,
            )?)?;
        }
        Ok(())
    }

    pub(crate) fn attribute_at<'node>(
        &self,
        node: impl Node<'node>,
        ordinal: usize,
    ) -> Result<Option<AuthoredEntryPath>, CatalogError> {
        let (node, index) = self.checked_node(node)?;
        let Some((name, value)) = node.attributes().nth(ordinal) else {
            return Ok(None);
        };
        let ordinal =
            u64::try_from(ordinal).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let location = SourceLocation::Subresource {
            namespace: "xml".to_owned(),
            indices: vec![u64::from(index), ordinal],
            field: format!("@{name}"),
        };
        self.capture_at(location, value).map(Some)
    }

    /// Returns all attribute occurrences in original order with exact source coordinates.
    pub(crate) fn sourced_attributes<'node>(
        &self,
        node: impl Node<'node>,
    ) -> Result<Vec<SourcedXmlAttribute>, CatalogError> {
        let source = self.node_source(node)?;
        deferred::sourced_attributes(
            &source,
            node.attributes(),
            self.capture.source_uses(),
            !self.is_lossy(),
        )
    }

    /// Returns direct text occurrences in their original order.
    pub(crate) fn sourced_text<'node>(
        &self,
        node: impl Node<'node>,
    ) -> Result<Vec<SourcedValue<String>>, CatalogError> {
        let source = self.node_source(node)?;
        let sources = self.capture.source_uses();
        let complete = !self.is_lossy();
        let text = |ordinal: usize, value: &str| {
            let ordinal = u64::try_from(ordinal)
                .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
            let usage = source.observe(
                vec![u64::from(source.index()), ordinal],
                "$text".to_owned(),
                SourceUseRole::Payload,
            )?;
            Ok(sources.value(SourceValue::Present(value.to_owned()), &[usage], complete))
        };
        match node.original() {
            Root::Cry(node) => node
                .content()
                .map(|value| text(0, value))
                .into_iter()
                .collect(),
            Root::Text(node) => node
                .children()
                .enumerate()
                .filter(|(_, child)| child.is_text())
                .filter_map(|(ordinal, child)| child.text().map(|value| text(ordinal, value)))
                .collect(),
        }
    }

    pub(crate) fn name<'node>(
        &self,
        node: impl Node<'node>,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        let (node, index) = self.checked_node(node)?;
        self.capture_value(index, "$name", node.name())
    }

    fn checked_node<'node>(
        &self,
        node: impl Node<'node>,
    ) -> Result<(Root<'node>, u32), CatalogError> {
        let node = node.original();
        let index = self
            .document
            .original_index(node)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok((node, index))
    }

    fn capture_value(
        &self,
        index: u32,
        field: &str,
        value: &str,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        let location = SourceLocation::Subresource {
            namespace: "xml".to_owned(),
            indices: vec![u64::from(index)],
            field: field.to_owned(),
        };
        self.capture_at(location, value)
    }

    fn capture_at(
        &self,
        location: SourceLocation,
        value: &str,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        attribute::capture_value(self.entry.as_ref(), self.capture, location, value)
    }
}

/// Original node coordinates in one successfully read XML Entry.
#[derive(Debug, Clone)]
pub(crate) struct XmlNodeSource {
    entry: Arc<EntryMetadata>,
    index: u32,
}

impl XmlNodeSource {
    pub(crate) const fn index(&self) -> u32 {
        self.index
    }

    pub(crate) fn selected(&self) -> Result<SourceUse, CatalogError> {
        self.observe(
            vec![u64::from(self.index)],
            "$name".to_owned(),
            SourceUseRole::Selector,
        )
    }

    fn observe(
        &self,
        indices: Vec<u64>,
        field: String,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        self.entry.payload_at(
            SourceLocation::Subresource {
                namespace: "xml".to_owned(),
                indices,
                field,
            },
            role,
        )
    }
}

/// An original attribute occurrence or the observed absence at its node.
#[derive(Debug)]
pub(crate) struct XmlAttributeSource {
    node: XmlNodeSource,
    slot: XmlAttributeSlot,
}

impl XmlAttributeSource {
    pub(crate) const fn node_index(&self) -> u32 {
        self.node.index()
    }

    pub(crate) fn observe(&self, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
        self.slot.observe(&self.node, role)
    }
}

#[derive(Debug)]
enum XmlAttributeSlot {
    Present {
        ordinal: u64,
        name: Cow<'static, str>,
    },
    Absent(&'static str),
}

impl XmlAttributeSlot {
    fn capture<'node>(node: impl Node<'node>, field: &'static str) -> Result<Self, CatalogError> {
        let Some((ordinal, (name, _))) = node
            .attributes()
            .enumerate()
            .find(|(_, (name, _))| name.eq_ignore_ascii_case(field))
        else {
            return Ok(Self::Absent(field));
        };
        Ok(Self::Present {
            ordinal: u64::try_from(ordinal)
                .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?,
            name: if name == field {
                Cow::Borrowed(field)
            } else {
                Cow::Owned(name.to_owned())
            },
        })
    }

    fn observe(
        &self,
        node: &XmlNodeSource,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        let index = u64::from(node.index);
        match self {
            Self::Present { ordinal, name } => {
                node.observe(vec![index, *ordinal], format!("@{name}"), role)
            }
            Self::Absent(name) => node.observe(
                vec![index],
                format!("$absent:@{name}"),
                SourceUseRole::Selector,
            ),
        }
    }
}
