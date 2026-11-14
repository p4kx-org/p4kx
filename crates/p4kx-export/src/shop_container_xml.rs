// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::catalog_read::original::{AuthoredEntryPath, EntryXml};
use crate::shop_registry::is_guid;
use crate::starmap_xml::{Node, attribute};
use crate::{CatalogError, ExportAccountingError};

const SHOP_TYPE: &str = "SCShop";

pub(super) struct Entities<Value> {
    pub(super) guids: Vec<Value>,
    pub(super) malformed: bool,
}
pub(super) struct Objects<Value> {
    pub(super) names: HashMap<String, Value>,
    pub(super) malformed: bool,
}

pub(super) fn guids<'node, NodeValue: Node<'node>, Value, Error>(
    root: NodeValue,
    mut capture: impl FnMut(NodeValue, &str) -> Result<Value, Error>,
) -> Result<Entities<Value>, Error> {
    let mut found = Entities {
        guids: Vec::new(),
        malformed: false,
    };
    for entity in root
        .children()
        .filter(|child| child.name().eq_ignore_ascii_case("Entity"))
    {
        if let Some(guid) = attribute(entity, "CryGUID") {
            found.malformed |= !is_guid(guid);
            found.guids.push(capture(entity, guid)?);
        } else {
            found.malformed = true;
        }
    }
    Ok(found)
}

pub(super) fn captured_guids(
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Entities<(String, AuthoredEntryPath)>, CatalogError> {
    guids(xml.root(), |node, value| {
        let guid = xml
            .attribute(node, "CryGUID")?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let guid = guid.selected_by(&xml.name(node)?)?;
        Ok((value.to_ascii_lowercase(), guid))
    })
}

pub(super) fn objects<'node, NodeValue: Node<'node>, Value, Error>(
    root: NodeValue,
    mut capture: impl FnMut(NodeValue, &str, &str) -> Result<Value, Error>,
    mut merge: impl FnMut(&mut Value, Value),
) -> Result<Objects<Value>, Error> {
    let mut named: HashMap<&str, Option<(String, Value)>> = HashMap::new();
    let mut malformed = false;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.name().eq_ignore_ascii_case("Object") && attribute(node, "type") == Some(SHOP_TYPE)
        {
            let (Some(name), Some(guid)) = (attribute(node, "name"), attribute(node, "guid"))
            else {
                malformed = true;
                pending.extend(node.children());
                continue;
            };
            malformed |= name.is_empty() || !is_guid(guid);
            let value = capture(node, name, guid)?;
            let guid = guid.to_ascii_lowercase();
            match named.entry(name) {
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(Some((guid, value)));
                }
                std::collections::hash_map::Entry::Occupied(mut slot) => {
                    if let Some((held_guid, held)) = slot.get_mut() {
                        if *held_guid == guid {
                            merge(held, value);
                        } else {
                            malformed = true;
                            *slot.get_mut() = None;
                        }
                    } else {
                        malformed = true;
                    }
                }
            }
        }
        pending.extend(node.children());
    }
    Ok(Objects {
        names: named
            .into_iter()
            .filter_map(|(name, value)| value.map(|(_, value)| (name.to_owned(), value)))
            .collect(),
        malformed,
    })
}

pub(super) fn captured_objects(
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Objects<(String, Vec<AuthoredEntryPath>)>, CatalogError> {
    objects(
        xml.root(),
        |node, _, guid| {
            let mut inputs = Vec::new();
            for field in ["type", "name", "guid"] {
                inputs.push(
                    xml.attribute(node, field)?
                        .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
                );
            }
            inputs.push(xml.name(node)?);
            Ok((guid.to_ascii_lowercase(), inputs))
        },
        |held, incoming| held.1.extend(incoming.1),
    )
}
