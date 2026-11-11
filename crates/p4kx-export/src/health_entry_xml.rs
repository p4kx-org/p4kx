// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole};

use super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::health_entry_frame::EntryCapture;
use super::health_entry_read::EntryRead;
use super::health_entry_selection::AuthoredEntryPath;
use crate::starmap_xml::{Document, Node, XmlError};
use crate::{CatalogError, ExportAccountingError};

/// One actual parse borrowing only bytes from its sealed original Entry read.
pub(crate) struct EntryXml<'read, 'capture, 'input> {
    read: &'read EntryRead,
    capture: &'capture EntryCapture<'input>,
    document: Document<'read>,
}

impl<'read, 'capture, 'input> EntryXml<'read, 'capture, 'input> {
    pub(crate) fn parse(
        read: &'read EntryRead,
        capture: &'capture EntryCapture<'input>,
    ) -> Result<Result<Self, XmlError>, CatalogError> {
        read.receipt().check_frame(&capture.frame())?;
        Ok(Document::parse(read.bytes()).map(|document| Self {
            read,
            capture,
            document,
        }))
    }

    pub(crate) fn root(&self) -> crate::starmap_xml::Root<'_> {
        self.document.root()
    }

    pub(crate) fn attribute<'node>(
        &self,
        node: impl Node<'node>,
        name: &str,
    ) -> Result<Option<AuthoredEntryPath>, CatalogError> {
        let node = node.original();
        let index = self
            .document
            .original_index(node)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
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
        let node = node.original();
        let index = self
            .document
            .original_index(node)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
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
        let node = node.original();
        let index = self
            .document
            .original_index(node)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.capture_value(index, "$name", node.name())
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
        let origin = self.read.receipt().payload_entry()?.clone();
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
