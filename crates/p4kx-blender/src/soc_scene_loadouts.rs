// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue};
use p4kx_model::{SocInlineLoadout, SocInlineLoadoutEntry, SocInlineLoadoutKind, SocSourceValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::{guid_value, position};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Loadout {
    parent_entry: Option<usize>,
    ordinal: usize,
    position: SocXmlPosition,
    kind: SourcedValue<Kind>,
    groups: Vec<Group>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Group {
    ordinal: usize,
    position: SocXmlPosition,
    entries: Vec<Entry>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: usize,
    ordinal: usize,
    position: SocXmlPosition,
    port_name: SourcedValue<String>,
    class_name: SourcedValue<String>,
    class_reference: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Kind {
    Manual,
    Other { name: String },
}

impl Loadout {
    pub(crate) fn read_all<'document>(
        declarations: impl Iterator<Item = SocInlineLoadout<'document>>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Vec<Self>, SocSceneError> {
        let mut pending: Vec<_> = declarations.map(|item| (None, item)).collect();
        pending.reverse();
        let mut next_entry = 0;
        let mut rows = Vec::new();
        while let Some((parent_entry, loadout)) = pending.pop() {
            let start = pending.len();
            rows.push(Self::read(
                loadout,
                parent_entry,
                &mut next_entry,
                &mut pending,
                origin,
            )?);
            if let Some(nested) = pending.get_mut(start..) {
                nested.reverse();
            }
        }
        Ok(rows)
    }

    fn read<'document>(
        loadout: SocInlineLoadout<'document>,
        parent_entry: Option<usize>,
        next_entry: &mut usize,
        pending: &mut Vec<(Option<usize>, SocInlineLoadout<'document>)>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = loadout.element();
        let value = match loadout.kind() {
            SocSourceValue::Present(SocInlineLoadoutKind::Manual) => {
                SourceValue::Present(Kind::Manual)
            }
            SocSourceValue::Present(SocInlineLoadoutKind::Null) => SourceValue::Null,
            SocSourceValue::Present(SocInlineLoadoutKind::Other(name)) => {
                SourceValue::Present(Kind::Other {
                    name: name.to_owned(),
                })
            }
            SocSourceValue::Absent => SourceValue::Absent,
            _ => SourceValue::Invalid,
        };
        let attributes = element
            .attributes()
            .filter(|attribute| attribute.namespace().is_none() && attribute.name() == "__type");
        let kind = origin.field(element, attributes, value)?;
        let groups = loadout
            .entry_groups()
            .map(|group| {
                let entries = group
                    .entries()
                    .map(|entry| {
                        let id = *next_entry;
                        *next_entry += 1;
                        pending.extend(entry.loadouts().map(|loadout| (Some(id), loadout)));
                        Entry::read(entry, id, origin)
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Group {
                    ordinal: group.ordinal(),
                    position: position(group.element())?,
                    entries,
                })
            })
            .collect::<Result<_, SocSceneError>>()?;
        Ok(Self {
            parent_entry,
            ordinal: loadout.ordinal(),
            position: position(element)?,
            kind,
            groups,
        })
    }
}

impl Entry {
    fn read(
        entry: SocInlineLoadoutEntry<'_>,
        id: usize,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = entry.element();
        Ok(Self {
            id,
            ordinal: entry.ordinal(),
            position: position(element)?,
            port_name: origin
                .named(element, "itemPortName", entry.port_name())?
                .map(str::to_owned),
            class_name: origin
                .named(element, "entityClassName", entry.class_name())?
                .map(str::to_owned),
            class_reference: origin.field(
                element,
                entry.class_reference_attributes(),
                guid_value(entry.class_reference_attributes()),
            )?,
        })
    }
}
