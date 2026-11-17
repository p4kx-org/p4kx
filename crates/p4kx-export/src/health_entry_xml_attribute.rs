// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole};

use super::super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::{AuthoredEntryPath, EntryCapture, EntryMetadata};
use crate::{CatalogError, ExportAccountingError};

/// One actual attribute yielded by an admitted XML parse.
pub(crate) struct XmlAttribute<'entry, 'text> {
    entry: &'entry EntryMetadata,
    node_index: u32,
    ordinal: u64,
    name: &'text str,
    value: &'text str,
}

impl<'entry, 'text> XmlAttribute<'entry, 'text> {
    pub(super) fn new(
        entry: &'entry EntryMetadata,
        node_index: u32,
        ordinal: usize,
        name: &'text str,
        value: &'text str,
    ) -> Result<Self, CatalogError> {
        let ordinal =
            u64::try_from(ordinal).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        Ok(Self {
            entry,
            node_index,
            ordinal,
            name,
            value,
        })
    }

    /// The exact original attribute name.
    pub(crate) const fn name(&self) -> &'text str {
        self.name
    }

    /// The original document-local node index.
    pub(crate) const fn node_index(&self) -> u32 {
        self.node_index
    }

    /// The zero-based ordinal in the node's original attribute sequence.
    pub(crate) const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Seals this original attribute with its exact node and ordinal coordinates.
    pub(crate) fn captured(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        capture_value(
            self.entry,
            capture,
            SourceLocation::Subresource {
                namespace: "xml".to_owned(),
                indices: vec![u64::from(self.node_index), self.ordinal],
                field: format!("@{}", self.name),
            },
            self.value,
        )
    }
}

pub(super) fn capture_value(
    entry: &EntryMetadata,
    capture: &EntryCapture<'_>,
    location: SourceLocation,
    value: &str,
) -> Result<AuthoredEntryPath, CatalogError> {
    let origin = entry.clone();
    AuthoredEntryPath::captured(
        capture,
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

#[cfg(test)]
#[path = "health_entry_xml_attribute_tests.rs"]
mod tests;
