// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, Guid, SourceUse, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::{CapturedLink, LootEntityConfigReference};
use crate::catalog_read::original::{EntryXml, SourcedXmlAttribute, XmlAttribute, XmlNodeSource};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Node, Root};
use crate::{CatalogError, ExportAccountingError};

/// One relevant physical XML node, stored once inside its document.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LootEntityLinkNode {
    node_index: SourcedValue<u32>,
    parent_node_index: SourcedValue<u32>,
    entity_node_index: SourcedValue<u32>,
    name: SourcedValue<String>,
    attributes: Vec<SourcedXmlAttribute>,
}

pub(super) struct CapturedNodes {
    pub(super) nodes: Vec<LootEntityLinkNode>,
    pub(super) links: Vec<CapturedLink>,
    pub(super) configs: Vec<LootEntityConfigReference>,
}

#[derive(Clone, Copy)]
struct Visit<'node> {
    node: Root<'node>,
    parent: Option<Root<'node>>,
    entity: Option<Owner<'node>>,
}

#[derive(Clone, Copy)]
struct Owner<'node> {
    node: Root<'node>,
    parent: Option<Root<'node>>,
}

pub(super) fn capture(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<CapturedNodes, CatalogError> {
    let mut pending = vec![Visit {
        node: xml.root(),
        parent: None,
        entity: None,
    }];
    let mut nodes = BTreeMap::new();
    let mut captured = CapturedNodes {
        nodes: Vec::new(),
        links: Vec::new(),
        configs: Vec::new(),
    };
    while let Some(mut visit) = pending.pop() {
        super::checkpoint(cancel)?;
        if visit.node.name().eq_ignore_ascii_case("Entity") {
            visit.entity = Some(Owner {
                node: visit.node,
                parent: visit.parent,
            });
        }
        xml.visit_attributes(visit.node, |attribute| {
            super::checkpoint(cancel)?;
            if !attribute.name().eq_ignore_ascii_case("objectContainer")
                && !attribute.name().eq_ignore_ascii_case("multiConfigRef")
            {
                return Ok(());
            }
            include_node(xml, visit, &mut nodes)?;
            capture_attribute(xml, &attribute, &mut captured)
        })?;
        let first = pending.len();
        pending.extend(visit.node.children().map(|node| Visit {
            node,
            parent: Some(visit.node),
            entity: visit.entity,
        }));
        pending
            .get_mut(first..)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .reverse();
    }
    captured.nodes = nodes.into_values().collect();
    Ok(captured)
}

fn capture_attribute(
    xml: &EntryXml<'_, '_, '_>,
    attribute: &XmlAttribute<'_, '_>,
    captured: &mut CapturedNodes,
) -> Result<(), CatalogError> {
    let token = attribute.captured(xml.capture())?;
    let (uses, complete) = token.selector_inputs()?;
    let complete = complete && !xml.is_lossy();
    let sources = xml.capture().source_uses();
    let node_index = sources.value(
        SourceValue::Present(attribute.node_index()),
        &uses,
        complete,
    );
    let attribute_ordinal =
        sources.value(SourceValue::Present(attribute.ordinal()), &uses, complete);
    let original = token.retain_in(xml.capture())?;
    let (original_uses, original_complete) = original.contributions();
    let original_complete = original_complete && !xml.is_lossy();
    if attribute.name().eq_ignore_ascii_case("objectContainer") {
        let raw = sources.value(
            SourceValue::Present(token.as_str().to_owned()),
            original_uses,
            original_complete,
        );
        captured.links.push(CapturedLink {
            node_index,
            attribute_ordinal,
            declared_path: raw,
            selector: token,
        });
    } else {
        let parsed = token
            .as_str()
            .trim()
            .parse::<Guid>()
            .map_or(SourceValue::Invalid, |guid| {
                SourceValue::Present(CatalogGuid(guid))
            });
        captured.configs.push(LootEntityConfigReference {
            node_index,
            attribute_ordinal,
            guid: sources.value(parsed, original_uses, original_complete),
        });
    }
    Ok(())
}

fn include_node(
    xml: &EntryXml<'_, '_, '_>,
    visit: Visit<'_>,
    nodes: &mut BTreeMap<u32, LootEntityLinkNode>,
) -> Result<(), CatalogError> {
    let source = xml.node_source(visit.node)?;
    if let std::collections::btree_map::Entry::Vacant(slot) = nodes.entry(source.index()) {
        slot.insert(node(xml, visit, &source)?);
    }
    if let Some(entity) = visit.entity {
        let source = xml.node_source(entity.node)?;
        if let std::collections::btree_map::Entry::Vacant(slot) = nodes.entry(source.index()) {
            slot.insert(node(
                xml,
                Visit {
                    node: entity.node,
                    parent: entity.parent,
                    entity: Some(entity),
                },
                &source,
            )?);
        }
    }
    Ok(())
}

fn node(
    xml: &EntryXml<'_, '_, '_>,
    visit: Visit<'_>,
    source: &XmlNodeSource,
) -> Result<LootEntityLinkNode, CatalogError> {
    let sources = xml.capture().source_uses();
    let selected = source.selected()?;
    let complete = !xml.is_lossy();
    Ok(LootEntityLinkNode {
        node_index: sources.value(
            SourceValue::Present(source.index()),
            std::slice::from_ref(&selected),
            complete,
        ),
        parent_node_index: related_node(xml, visit.parent, &selected)?,
        entity_node_index: related_node(xml, visit.entity.map(|owner| owner.node), &selected)?,
        name: xml
            .name(visit.node)?
            .retain_in(xml.capture())?
            .into_cell(sources)
            .map(|text| text.as_str().to_owned()),
        attributes: xml.sourced_attributes(visit.node)?,
    })
}

fn related_node(
    xml: &EntryXml<'_, '_, '_>,
    related: Option<Root<'_>>,
    selected: &SourceUse,
) -> Result<SourcedValue<u32>, CatalogError> {
    let mut inputs = vec![selected.clone(), xml.document_source()?];
    let value = if let Some(related) = related {
        let source = xml.node_source(related)?;
        inputs.push(source.selected()?);
        SourceValue::Present(source.index())
    } else {
        SourceValue::Absent
    };
    Ok(xml
        .capture()
        .source_uses()
        .value(value, &inputs, !xml.is_lossy()))
}
