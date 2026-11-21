// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::{LocationAttribute, state};
use crate::catalog_read::original::{EntryCapture, EntryXml, XmlNodeSource};
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Node, Root};
use crate::{CatalogError, ExportAccountingError};

const METADATA: &str = "InstanceMetadata";

/// One original metadata element with its physical node and direct parent.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationMetadataElement {
    node_index: SourcedValue<u32>,
    parent_node_index: SourcedValue<u32>,
    name: SourcedValue<String>,
    attributes: Vec<LocationAttribute>,
}

impl LocationMetadataElement {
    /// Returns the physical XML node index.
    #[must_use]
    pub const fn node_index(&self) -> &SourcedValue<u32> {
        &self.node_index
    }

    /// Returns the direct parent index within this metadata.
    #[must_use]
    pub const fn parent_node_index(&self) -> &SourcedValue<u32> {
        &self.parent_node_index
    }

    /// Returns the authored element name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Returns the ordered original attributes.
    #[must_use]
    pub const fn attributes(&self) -> &[LocationAttribute] {
        self.attributes.as_slice()
    }
}

pub(super) fn read(
    xml: &EntryXml<'_, '_, '_>,
    child: Root<'_>,
    capture: &EntryCapture<'_>,
) -> Result<SourcedValue<Vec<LocationMetadataElement>>, CatalogError> {
    let sources = capture.source_uses();
    let mut elements = Vec::new();
    let mut selected = vec![xml.node_source(child)?.selected()?];
    for scope in child
        .children()
        .filter(|node| node.name().eq_ignore_ascii_case(METADATA))
    {
        let source = xml.node_source(scope)?;
        selected.push(source.selected()?);
        append_metadata(xml, scope, capture, &mut elements)?;
    }
    Ok(sources.value(
        if elements.is_empty() {
            SourceValue::Absent
        } else {
            SourceValue::Present(elements)
        },
        &selected,
        !xml.is_lossy(),
    ))
}

fn append_metadata(
    xml: &EntryXml<'_, '_, '_>,
    scope: Root<'_>,
    capture: &EntryCapture<'_>,
    elements: &mut Vec<LocationMetadataElement>,
) -> Result<(), CatalogError> {
    let mut pending: Vec<(Root<'_>, Option<XmlNodeSource>)> = vec![(scope, None)];
    let sources = capture.source_uses();
    let complete = !xml.is_lossy();
    while let Some((node, parent)) = pending.pop() {
        let source = xml.node_source(node)?;
        let index = source.index();
        let selected = source.selected()?;
        let mut parent_uses = vec![selected.clone()];
        if let Some(parent) = &parent {
            parent_uses.push(parent.selected()?);
        }
        elements.push(LocationMetadataElement {
            node_index: sources.value(
                SourceValue::Present(index),
                std::slice::from_ref(&selected),
                complete,
            ),
            parent_node_index: sources.value(
                state(parent.as_ref().map(XmlNodeSource::index)),
                &parent_uses,
                complete,
            ),
            name: sources.value(
                SourceValue::Present(node.name().to_owned()),
                std::slice::from_ref(&selected),
                complete,
            ),
            attributes: metadata_attributes(xml, node, capture)?,
        });
        let first = pending.len();
        pending.extend(node.children().map(|child| (child, Some(source.clone()))));
        if let Some(children) = pending.get_mut(first..) {
            children.reverse();
        }
    }
    Ok(())
}

fn metadata_attributes(
    xml: &EntryXml<'_, '_, '_>,
    node: Root<'_>,
    capture: &EntryCapture<'_>,
) -> Result<Vec<LocationAttribute>, CatalogError> {
    let sources = capture.source_uses();
    node.attributes()
        .enumerate()
        .map(|(ordinal, (name, _))| {
            let observed = xml
                .attribute_at(node, ordinal)?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let (name_uses, name_complete) = observed.selector_inputs()?;
            let datum = observed.retain_in(capture)?;
            let (value_uses, value_complete) = datum.contributions();
            Ok(LocationAttribute {
                name: sources.value(
                    SourceValue::Present(name.to_owned()),
                    &name_uses,
                    name_complete && !xml.is_lossy(),
                ),
                value: sources.value(
                    SourceValue::Present(observed.as_str().to_owned()),
                    value_uses,
                    value_complete && !xml.is_lossy(),
                ),
            })
        })
        .collect()
}
