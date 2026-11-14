// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    CancelToken, Guid, SourceUse, SourceUseRole, SourceUses, SourceValue, SourceValueState,
    SourcedValue,
};

use super::{COMPONENT, JumpGateEntity, JumpGateObject};
use crate::CatalogError;
use crate::catalog_read::original::{EntryXml, PhysicalEntryRef};
use crate::guid_text::CatalogGuid;
use crate::starmap_xml::{self, Node, Root};

pub(super) struct Entity {
    node_index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
    guid_text: SourcedValue<String>,
    offset: SourcedValue<String>,
    proof: Vec<SourceUse>,
}

impl Entity {
    pub(super) fn link(
        self,
        sources: &SourceUses,
        document: Option<&Objects>,
        editor: &SourcedValue<PhysicalEntryRef>,
    ) -> JumpGateEntity {
        let mut uses = self.proof;
        let mut matches = Vec::new();
        if let Some(document) = document {
            uses.push(document.root.clone());
            if let Some(guid) = self.guid.value() {
                for (object, input) in document.rows.get(&guid.0).into_iter().flatten() {
                    uses.push(input.clone());
                    matches.push(object.clone());
                }
            }
        }
        let state = match (self.guid.value(), document, matches.as_slice()) {
            (None, _, _) | (Some(_), Some(_), [_, _, ..]) => SourceValue::Invalid,
            (Some(_), Some(_), [selected]) => selected
                .node_index
                .value()
                .copied()
                .map_or(SourceValue::Invalid, SourceValue::Present),
            (Some(_), Some(_), []) => SourceValue::Absent,
            _ => match editor.state() {
                SourceValueState::Absent => SourceValue::Absent,
                SourceValueState::Invalid => SourceValue::Invalid,
                _ => SourceValue::Unreadable,
            },
        };
        let complete = editor.origin().source_set_complete();
        JumpGateEntity {
            node_index: self.node_index,
            cry_guid: self.guid,
            cry_guid_text: self.guid_text,
            offset_from_pivot: self.offset,
            matches: sources.value(SourceValue::Present(matches), &uses, complete),
            object: sources.value(state, &uses, complete),
            operating_entity: sources.value(SourceValue::Absent, &uses, false),
            operating_configurations: sources.value(SourceValue::Absent, &uses, false),
            object_uses: uses,
        }
    }
}

pub(super) fn entities(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Vec<Entity>>, CatalogError> {
    if xml.root().name() != COMPONENT {
        return Ok(None);
    }
    let mut output = Vec::new();
    for node in xml.root().children().filter(|node| node.name() == "Entity") {
        checkpoint(cancel)?;
        let fields = Fields { xml, node };
        let proof = vec![
            xml.node_source(xml.root())?.selected()?,
            fields.selector("CryGUID")?,
        ];
        output.push(Entity {
            node_index: fields.index()?,
            guid: fields.guid("CryGUID")?,
            guid_text: fields.text("CryGUID")?,
            offset: fields.text("offsetFromPivot")?,
            proof,
        });
    }
    Ok(Some(output))
}

pub(super) struct Objects {
    pub(super) root: SourceUse,
    rows: HashMap<Guid, Vec<(JumpGateObject, SourceUse)>>,
}

pub(super) fn objects(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Objects>, CatalogError> {
    let root = xml.root();
    if root.name() != "ObjectContainer" {
        return Ok(None);
    }
    let mut rows: HashMap<Guid, Vec<(JumpGateObject, SourceUse)>> = HashMap::new();
    let mut pending = Vec::new();
    for layers in root.children().filter(|node| node.name() == "Layers") {
        pending.extend(layers.children().filter(|node| node.name() == "Layer"));
    }
    pending.reverse();
    while let Some(node) = pending.pop() {
        checkpoint(cancel)?;
        if node.name() == "Object" {
            let fields = Fields { xml, node };
            let guid = fields.guid("guid")?;
            if let Some(value) = guid.value() {
                rows.entry(value.0).or_default().push((
                    JumpGateObject {
                        node_index: fields.index()?,
                        guid,
                        class: fields.text("type")?,
                        class_use: fields.selector("type")?,
                        name: fields.text("name")?,
                        tags: fields.text("tags")?,
                    },
                    fields.selector("guid")?,
                ));
            }
        } else {
            let start = pending.len();
            pending.extend(
                node.children()
                    .filter(|item| matches!(item.name(), "Layer" | "Object")),
            );
            if let Some(added) = pending.get_mut(start..) {
                added.reverse();
            }
        }
    }
    Ok(Some(Objects {
        root: xml.node_source(root)?.selected()?,
        rows,
    }))
}

struct Fields<'a, 'read, 'capture, 'input, 'node> {
    xml: &'a EntryXml<'read, 'capture, 'input>,
    node: Root<'node>,
}

impl Fields<'_, '_, '_, '_, '_> {
    fn index(&self) -> Result<SourcedValue<u32>, CatalogError> {
        let source = self.xml.node_source(self.node)?;
        Ok(self.xml.capture().source_uses().value(
            SourceValue::Present(source.index()),
            &[source.selected()?],
            true,
        ))
    }

    fn selector(&self, field: &'static str) -> Result<SourceUse, CatalogError> {
        self.xml
            .attribute_source(self.node, field)?
            .observe(SourceUseRole::Selector)
    }

    fn guid(&self, field: &'static str) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let value = starmap_xml::attribute(self.node, field).map_or(SourceValue::Absent, |raw| {
            raw.trim()
                .parse::<Guid>()
                .ok()
                .filter(|guid| *guid != Guid::default())
                .map_or(SourceValue::Invalid, |guid| {
                    SourceValue::Present(CatalogGuid(guid))
                })
        });
        Ok(self
            .xml
            .capture()
            .source_uses()
            .value(value, &[self.selector(field)?], true))
    }

    fn text(&self, field: &'static str) -> Result<SourcedValue<String>, CatalogError> {
        let value = starmap_xml::attribute(self.node, field).map_or(SourceValue::Absent, |text| {
            SourceValue::Present(text.to_owned())
        });
        let used = self
            .xml
            .attribute_source(self.node, field)?
            .observe(SourceUseRole::Payload)?;
        Ok(self.xml.capture().source_uses().value(value, &[used], true))
    }
}

fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        Err(CatalogError::Cancelled)
    } else {
        Ok(())
    }
}
