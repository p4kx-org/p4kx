// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue};

use super::{
    EntryCapture, EntryReceipt, EntryXml, SourcedXmlAttribute, XmlAttributeSlot,
    XmlAttributeSource, XmlNodeSource,
};
use crate::catalog_read::original::{OriginalDatum, PhysicalEntryRef};
use crate::starmap_xml::Node;
use crate::{CatalogError, ExportAccountingError};

/// Original attribute values sealed during one admitted XML parse, before contribution retention.
pub(crate) struct DeferredXmlNode {
    source: XmlNodeSource,
    receipt: EntryReceipt,
    attributes: Vec<(String, String)>,
    complete: bool,
}

impl EntryXml<'_, '_, '_> {
    pub(crate) fn deferred_node<'node>(
        &self,
        node: impl Node<'node>,
    ) -> Result<DeferredXmlNode, CatalogError> {
        let (node, index) = self.checked_node(node)?;
        Ok(DeferredXmlNode {
            source: XmlNodeSource {
                entry: std::sync::Arc::clone(&self.entry),
                index,
            },
            receipt: self.read.receipt().clone(),
            attributes: node
                .attributes()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            complete: !self.is_lossy(),
        })
    }
}

impl DeferredXmlNode {
    pub(crate) const fn source(&self) -> &XmlNodeSource {
        &self.source
    }

    pub(crate) fn descriptor(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.receipt.payload_reference(capture)
    }

    pub(crate) fn attributes(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<Vec<SourcedXmlAttribute>, CatalogError> {
        self.receipt.check_capture(capture)?;
        sourced_attributes(
            &self.source,
            self.attributes
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
            capture.source_uses(),
            self.complete,
        )
    }

    pub(crate) fn attribute<T>(
        &self,
        capture: &EntryCapture<'_>,
        field: &'static str,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.receipt.check_capture(capture)?;
        let original = self
            .attributes
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name.eq_ignore_ascii_case(field));
        let (value, slot) = match original {
            Some((ordinal, (name, raw))) => (
                parse(raw).map_or(SourceValue::Invalid, SourceValue::Present),
                XmlAttributeSlot::Present {
                    ordinal: u64::try_from(ordinal)
                        .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?,
                    name: Cow::Owned(name.clone()),
                },
            ),
            None => (SourceValue::Absent, XmlAttributeSlot::Absent(field)),
        };
        let source = XmlAttributeSource {
            node: self.source.clone(),
            slot,
        };
        let used = source.observe(SourceUseRole::Conversion)?;
        Ok(capture.source_uses().value(value, &[used], self.complete))
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.complete
    }
}

pub(super) fn sourced_attributes<'text>(
    source: &XmlNodeSource,
    attributes: impl Iterator<Item = (&'text str, &'text str)>,
    sources: &SourceUses,
    complete: bool,
) -> Result<Vec<SourcedXmlAttribute>, CatalogError> {
    attributes
        .enumerate()
        .map(|(ordinal, (name, value))| {
            let ordinal = u64::try_from(ordinal)
                .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
            let indices = vec![u64::from(source.index()), ordinal];
            let field = format!("@{name}");
            let name_use =
                source.observe(indices.clone(), field.clone(), SourceUseRole::Selector)?;
            let value_use = source.observe(indices, field, SourceUseRole::Payload)?;
            Ok(SourcedXmlAttribute {
                name: sources.value(SourceValue::Present(name.to_owned()), &[name_use], complete),
                value: sources.value(
                    SourceValue::Present(value.to_owned()),
                    &[value_use],
                    complete,
                ),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "health_entry_xml_deferred_tests.rs"]
mod tests;
