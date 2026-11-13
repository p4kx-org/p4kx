// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceDocument;
use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value, ValueError};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_components::{ComponentInput, struct_type};
use crate::soc_scene_class_mannequin::{children, string_attribute};
use crate::soc_scene_class_resources::{Resource, ResourceKind};
use crate::soc_scene_classes::string;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DefaultLoadout {
    declared_type: SourcedValue<String>,
    xml_path: SourcedValue<String>,
    xml: SourcedValue<Resource<XmlLoadout>>,
    manual: SourcedValue<Vec<ManualEntry>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct XmlLoadout {
    root_groups: Vec<SocXmlPosition>,
    items: Vec<XmlItem>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct XmlItem {
    parent: Option<usize>,
    position: SocXmlPosition,
    item_groups: Vec<SocXmlPosition>,
    port: SourcedValue<String>,
    item: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ManualEntry {
    parent: Option<usize>,
    ordinal: usize,
    port: SourcedValue<String>,
    item_class: SourcedValue<String>,
    class_reference: SourcedValue<String>,
    nested_type: SourcedValue<String>,
}

impl DefaultLoadout {
    pub(crate) fn read(
        input: &ComponentInput<'_, '_, '_>,
        prefix: &str,
    ) -> Result<Self, SocSceneError> {
        let loadout = input.component.field("loadout");
        let declared_type = input.cell(prefix, "loadout", struct_type(loadout))?;
        let loadout_prefix = format!("{prefix}.loadout");
        let path = match loadout {
            Some(Value::Struct(value)) if value.type_name() == "SItemPortLoadoutXMLParams" => {
                string(value.field("loadoutPath"))
            }
            _ => SourceValue::Absent,
        };
        let xml_path = input.cell(&loadout_prefix, "loadoutPath", path.clone())?;
        let selector = input.use_at(&loadout_prefix, "loadoutPath", SourceUseRole::Selector)?;
        let xml = input
            .resources
            .read(path, &selector, ResourceKind::Loadout, XmlLoadout::read)?;
        let manual = match loadout {
            Some(Value::Struct(value)) if value.type_name() == "SItemPortLoadoutManualParams" => {
                match value.field("entries") {
                    Some(Value::Array(entries)) => {
                        SourceValue::Present(read_manual(entries, &loadout_prefix, input)?)
                    }
                    None => SourceValue::Absent,
                    Some(Value::Null) => SourceValue::Null,
                    Some(Value::Error(_)) => SourceValue::Unreadable,
                    _ => SourceValue::Invalid,
                }
            }
            _ => SourceValue::Absent,
        };
        Ok(Self {
            declared_type,
            xml_path,
            xml,
            manual: input.cell(&loadout_prefix, "entries", manual)?,
        })
    }
}

impl XmlLoadout {
    fn read(
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Option<Self>, SocSceneError> {
        let root = document.root();
        if root.name() != "Loadout" || root.namespace().is_some() {
            return Ok(None);
        }
        let root_groups = children(root, "Items")
            .map(position)
            .collect::<Result<_, _>>()?;
        let mut items = Vec::new();
        let mut pending: Vec<_> = children(root, "Items")
            .flat_map(|group| children(group, "Item"))
            .map(|node| (None, node))
            .collect();
        pending.reverse();
        while let Some((parent, node)) = pending.pop() {
            let index = items.len();
            let start = pending.len();
            pending.extend(
                children(node, "Items")
                    .flat_map(|group| children(group, "Item"))
                    .map(|node| (Some(index), node)),
            );
            if let Some(nested) = pending.get_mut(start..) {
                nested.reverse();
            }
            items.push(XmlItem {
                parent,
                position: position(node)?,
                item_groups: children(node, "Items")
                    .map(position)
                    .collect::<Result<_, _>>()?,
                port: string_attribute(node, "portName", origin)?,
                item: string_attribute(node, "itemName", origin)?,
            });
        }
        Ok(Some(Self { root_groups, items }))
    }
}

fn read_manual(
    entries: &[Value<'_>],
    prefix: &str,
    input: &ComponentInput<'_, '_, '_>,
) -> Result<Vec<ManualEntry>, SocSceneError> {
    let mut rows = Vec::new();
    let mut pending = Vec::new();
    enqueue(&mut pending, entries, None, prefix);
    while let Some((parent, ordinal, path, value)) = pending.pop() {
        let Value::Struct(entry) = value else {
            continue;
        };
        let nested = entry.field("loadout");
        let index = rows.len();
        if let Some(Value::Struct(loadout)) = nested
            && loadout.type_name() == "SItemPortLoadoutManualParams"
            && let Some(Value::Array(entries)) = loadout.field("entries")
        {
            enqueue(
                &mut pending,
                entries,
                Some(index),
                &format!("{path}.loadout"),
            );
        }
        let reference = reference(entry.field("entityClassReference"));
        rows.push(ManualEntry {
            parent,
            ordinal,
            port: input.cell(&path, "itemPortName", string(entry.field("itemPortName")))?,
            item_class: input.cell(
                &path,
                "entityClassName",
                string(entry.field("entityClassName")),
            )?,
            class_reference: input.cell(&path, "entityClassReference", reference)?,
            nested_type: input.cell(&path, "loadout", struct_type(nested))?,
        });
    }
    Ok(rows)
}

fn enqueue<'value, 'source>(
    pending: &mut Vec<(Option<usize>, usize, String, &'value Value<'source>)>,
    entries: &'value [Value<'source>],
    parent: Option<usize>,
    prefix: &str,
) {
    pending.extend(entries.iter().enumerate().rev().map(|(ordinal, entry)| {
        (
            parent,
            ordinal,
            format!("{prefix}.entries[{ordinal}]"),
            entry,
        )
    }));
}

fn reference(value: Option<&Value<'_>>) -> SourceValue<String> {
    match value {
        Some(Value::Reference(link)) => SourceValue::Present(link.guid().to_string()),
        Some(Value::Error(ValueError::DanglingReference { guid })) => {
            SourceValue::Present(guid.to_string())
        }
        Some(Value::Null) => SourceValue::Null,
        None => SourceValue::Absent,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        _ => SourceValue::Invalid,
    }
}
