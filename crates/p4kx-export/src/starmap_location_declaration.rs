// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{EntryCapture, EntryXml, XmlNodeSource};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Node, Root, attribute};
use crate::{CatalogError, ExportAccountingError};

const LOCATION_TYPE: &str = "locationType";
const METADATA: &str = "InstanceMetadata";

/// The authored fields and component declarations of a typed location Child.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ChildLocation {
    location_type: SourcedValue<u32>,
    streaming_root: SourcedValue<bool>,
    guid: SourcedValue<CatalogGuid>,
    class_guid: SourcedValue<CatalogGuid>,
    #[schemars(extend("x-gameText" = true))]
    label: SourcedValue<String>,
    entity_name: SourcedValue<String>,
    tags_text: SourcedValue<String>,
    tags: SourcedValue<Vec<SourcedValue<LocationTag>>>,
    has_hierarchical_context_data: SourcedValue<bool>,
    invalid_attributes: Vec<LocationAttribute>,
    instance_metadata: SourcedValue<Vec<LocationMetadataElement>>,
}

impl ChildLocation {
    /// Retains a location declaration and its exact source uses from its owning Child.
    pub(crate) fn read(
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        capture: &EntryCapture<'_>,
    ) -> Result<Option<Box<Self>>, CatalogError> {
        if attribute(child, LOCATION_TYPE).is_none() {
            return Ok(None);
        }
        let mut fields = LocationFields {
            xml,
            child,
            sources: capture.source_uses(),
            complete: !xml.is_lossy(),
            invalid_attributes: Vec::new(),
        };
        let location_type = fields.typed(LOCATION_TYPE, |raw| raw.trim().parse().ok())?;
        let streaming_root = fields.typed("streamingRoot", boolean)?;
        let guid = fields.typed("guid", parse_guid)?;
        let class_guid = fields.typed("classGuid", parse_guid)?;
        let has_hierarchical_context_data = fields.typed("hasHierarchicalContextData", boolean)?;
        let tags = fields.tags()?;
        Ok(Some(Box::new(Self {
            location_type,
            streaming_root,
            guid,
            class_guid,
            label: fields.text("label")?,
            entity_name: fields.text("entityName")?,
            tags_text: tags.text,
            tags: tags.tokens,
            has_hierarchical_context_data,
            instance_metadata: read_metadata(xml, child, capture)?,
            invalid_attributes: fields.invalid_attributes,
        })))
    }
}

struct LocationFields<'xml, 'read, 'capture, 'input, 'node> {
    xml: &'xml EntryXml<'read, 'capture, 'input>,
    child: Root<'node>,
    sources: &'xml SourceUses,
    complete: bool,
    invalid_attributes: Vec<LocationAttribute>,
}

impl LocationFields<'_, '_, '_, '_, '_> {
    fn text(&self, name: &'static str) -> Result<SourcedValue<String>, CatalogError> {
        let raw = attribute(self.child, name).map(str::to_owned);
        let source = self.origin(name, SourceUseRole::Payload)?;
        Ok(self.sources.value(state(raw), &[source], self.complete))
    }

    fn typed<T>(
        &mut self,
        field_name: &'static str,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let source = self.origin(field_name, SourceUseRole::Conversion)?;
        let original = self
            .child
            .attributes()
            .find(|(field, _)| field.eq_ignore_ascii_case(field_name));
        let value = match original {
            None => SourceValue::Absent,
            Some((name, raw)) => {
                if let Some(parsed) = parse(raw) {
                    SourceValue::Present(parsed)
                } else {
                    let name_use = self.origin(field_name, SourceUseRole::Selector)?;
                    let value_use = self.origin(field_name, SourceUseRole::Payload)?;
                    self.invalid_attributes.push(LocationAttribute::new(
                        self.sources,
                        name,
                        raw.to_owned(),
                        name_use,
                        value_use,
                        self.complete,
                    ));
                    SourceValue::Invalid
                }
            }
        };
        Ok(self.sources.value(value, &[source], self.complete))
    }

    fn origin(&self, name: &'static str, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
        self.xml.attribute_source(self.child, name)?.observe(role)
    }

    fn tags(&self) -> Result<LocationTags, CatalogError> {
        let text = self.text("tags")?;
        let source = self.origin("tags", SourceUseRole::Conversion)?;
        let selection = std::slice::from_ref(&source);
        let parsed = attribute(self.child, "tags").map_or(SourceValue::Absent, |raw| {
            let mut tags = Vec::new();
            if !raw.is_empty() {
                for token in raw.split(',') {
                    let raw = self.sources.value(
                        SourceValue::Present(token.to_owned()),
                        selection,
                        self.complete,
                    );
                    let parsed =
                        parse_guid(token).map_or(SourceValue::Invalid, SourceValue::Present);
                    let guid = self.sources.value(parsed, selection, self.complete);
                    tags.push(self.sources.value(
                        SourceValue::Present(LocationTag { raw, guid }),
                        selection,
                        self.complete,
                    ));
                }
            }
            SourceValue::Present(tags)
        });
        Ok(LocationTags {
            text,
            tokens: self.sources.value(parsed, selection, self.complete),
        })
    }
}

struct LocationTags {
    text: SourcedValue<String>,
    tokens: SourcedValue<Vec<SourcedValue<LocationTag>>>,
}

fn state<T>(raw: Option<T>) -> SourceValue<T> {
    raw.map_or(SourceValue::Absent, SourceValue::Present)
}

fn parse_guid(raw: &str) -> Option<CatalogGuid> {
    raw.trim().parse::<Guid>().ok().map(CatalogGuid)
}

fn boolean(raw: &str) -> Option<bool> {
    match raw.trim() {
        "0" | "false" => Some(false),
        "1" | "true" => Some(true),
        _ => None,
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct LocationTag {
    raw: SourcedValue<String>,
    guid: SourcedValue<CatalogGuid>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct LocationAttribute {
    name: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    value: SourcedValue<String>,
}

impl LocationAttribute {
    fn new(
        sources: &SourceUses,
        name: &str,
        value: String,
        name_use: SourceUse,
        value_use: SourceUse,
        complete: bool,
    ) -> Self {
        Self {
            name: sources.value(SourceValue::Present(name.to_owned()), &[name_use], complete),
            value: sources.value(SourceValue::Present(value), &[value_use], complete),
        }
    }
}

/// One original metadata element with its physical node and direct parent.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LocationMetadataElement {
    node_index: SourcedValue<u32>,
    parent_node_index: SourcedValue<u32>,
    name: SourcedValue<String>,
    attributes: Vec<LocationAttribute>,
}

fn read_metadata(
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
