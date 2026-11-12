// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole};

use super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::health_entry_frame::EntryCapture;
use super::health_entry_metadata::EntryMetadata;
use super::health_entry_read::EntryRead;
use super::health_entry_selection::AuthoredEntryPath;
use crate::starmap_xml::{Document, Node, Root, XmlError};
use crate::{CatalogError, ExportAccountingError};

/// One actual parse borrowing only bytes from its sealed original Entry read.
pub(crate) struct EntryXml<'read, 'capture, 'input> {
    entry: Arc<EntryMetadata>,
    capture: &'capture EntryCapture<'input>,
    document: Document<'read>,
}

impl<'read, 'capture, 'input> EntryXml<'read, 'capture, 'input> {
    pub(crate) fn parse(
        read: &'read EntryRead,
        capture: &'capture EntryCapture<'input>,
    ) -> Result<Result<Self, XmlError>, CatalogError> {
        read.receipt().check_frame(&capture.frame())?;
        let document = match Document::parse(read.bytes()) {
            Ok(document) => document,
            Err(error) => return Ok(Err(error)),
        };
        Ok(Ok(Self {
            entry: Arc::new(read.receipt().payload_entry()?.clone()),
            capture,
            document,
        }))
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
        let origin = self.entry.as_ref().clone();
        AuthoredEntryPath::captured(
            self.capture,
            value,
            vec![
                EntrySelectionUse {
                    source: SelectionSource::Entry(origin.clone()),
                    location: location.clone(),
                    role: SourceUseRole::Payload,
                },
                EntrySelectionUse {
                    source: SelectionSource::Entry(origin),
                    location,
                    role: SourceUseRole::Conversion,
                },
            ],
        )
    }
}

/// Original node coordinates in one successfully read XML Entry.
#[derive(Debug, Clone)]
pub(crate) struct XmlNodeSource {
    entry: Arc<EntryMetadata>,
    index: u32,
}

impl XmlNodeSource {
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
