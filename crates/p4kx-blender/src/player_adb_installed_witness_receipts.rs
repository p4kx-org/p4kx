// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{
    InputScope, PhysicalSourceRef, SourceEncryption, SourceEncryptionState, SourceInventory,
    SourceLocation, SourceUseDescription, SourceUseRole,
};
use serde_json::{Value, json};

use super::{Check, Original};

pub(super) struct Sources<'capture>(BTreeMap<&'capture str, &'capture SourceUseDescription>);

impl<'capture> Sources<'capture> {
    pub(super) fn new<'document>(
        inventory: &'capture SourceInventory,
        originals: impl IntoIterator<Item = &'document Original>,
    ) -> Self {
        let documents: BTreeSet<&PhysicalSourceRef> =
            originals.into_iter().map(|doc| &doc.source).collect();
        Self(
            inventory
                .iter_uses()
                .filter(|usage| documents.contains(usage.source()))
                .map(|usage| (usage.id().as_str(), usage))
                .collect(),
        )
    }

    pub(super) fn document<'document>(
        &'document self,
        original: &'document Original,
    ) -> DocumentReceipts<'document> {
        DocumentReceipts {
            original,
            uses: &self.0,
        }
    }
}

pub(super) struct DocumentReceipts<'document> {
    original: &'document Original,
    uses: &'document BTreeMap<&'document str, &'document SourceUseDescription>,
}

impl DocumentReceipts<'_> {
    pub(super) fn position(&self, node: &Value) -> Check {
        let position = &node["position"];
        if position["state"] != "present" {
            return Err("original XML node position must be present".into());
        }
        self.origin(position, &[Coordinate::element(node)])
    }

    pub(super) fn attributes(&self, node: &Value, field: &str, name: &str) -> Check<Vec<Value>> {
        let selection = &node[field];
        let rows = selection["occurrences"]
            .as_array()
            .ok_or("source attribute occurrences")?;
        let state = match rows.len() {
            0 => "absent",
            1 => "present",
            _ => "invalid",
        };
        if selection["state"] != state {
            return Err("sourced selector state differs".into());
        }
        let mut coordinates = Vec::with_capacity(rows.len() + 1);
        coordinates.push(Coordinate::element(node));
        let mut projected = Vec::with_capacity(rows.len());
        for row in rows {
            let value = &row["value"];
            if value["state"] != "present" || !value["value"].is_string() {
                return Err("original attribute occurrence must contain text".into());
            }
            let coordinate = Coordinate {
                position: &row["position"],
                field: name,
            };
            self.origin(value, std::slice::from_ref(&coordinate))?;
            coordinates.push(coordinate);
            projected.push(json!([row["position"], value["value"]]));
        }
        self.origin(selection, &coordinates)?;
        Ok(projected)
    }

    pub(super) fn preceding_blend(
        &self,
        layer: &Value,
        animation: &Value,
        blends: &[Value],
    ) -> Check<Option<u64>> {
        let relation = &animation["precedingBlend"];
        let known_elements = [Coordinate::element(layer), Coordinate::element(animation)];
        match relation["state"].as_str() {
            Some("absent") if relation.get("value") == Some(&Value::Null) => {
                self.origin(relation, &known_elements)?;
                Ok(None)
            }
            Some("present") => {
                let ordinal = relation["value"]
                    .as_u64()
                    .ok_or("preceding Blend ordinal")?;
                let blend = blends
                    .iter()
                    .find(|blend| blend["ordinal"].as_u64() == Some(ordinal))
                    .ok_or("preceding Blend is missing from the same layer")?;
                let [layer, animation] = known_elements;
                self.origin(relation, &[layer, animation, Coordinate::element(blend)])?;
                Ok(Some(ordinal))
            }
            _ => Err("Blend relation has an invalid state".into()),
        }
    }

    fn origin(&self, value: &Value, coordinates: &[Coordinate<'_>]) -> Check {
        let (encrypted, evidence) = match self.original.encryption {
            SourceEncryption::Encrypted => (Value::Bool(true), SourceEncryptionState::Encrypted),
            SourceEncryption::Unencrypted => {
                (Value::Bool(false), SourceEncryptionState::Unencrypted)
            }
            _ => (Value::Null, SourceEncryptionState::Unknown),
        };
        let references = value["sourceUsesRefs"]
            .as_array()
            .ok_or("XML source uses")?;
        if value["sourceSetComplete"] != true
            || value.get("containsEncryptedSource") != Some(&encrypted)
            || references.len() != coordinates.len()
        {
            return Err("XML contributor count, completeness or encryption differs".into());
        }
        for coordinate in coordinates {
            if !references.iter().any(|reference| {
                reference
                    .as_str()
                    .and_then(|id| self.uses.get(id))
                    .is_some_and(|usage| {
                        usage.source() == &self.original.source
                            && usage.scope() == InputScope::Payload
                            && usage.role() == SourceUseRole::Payload
                            && usage.effective_encryption() == evidence
                            && coordinate.matches(usage.location())
                    })
            }) {
                return Err("XML source use misses an original field or element coordinate".into());
            }
        }
        Ok(())
    }
}

struct Coordinate<'a> {
    position: &'a Value,
    field: &'a str,
}

impl<'a> Coordinate<'a> {
    fn element(node: &'a Value) -> Self {
        Self {
            position: &node["position"]["value"],
            field: "$element",
        }
    }

    fn matches(&self, location: &SourceLocation) -> bool {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = location
        else {
            return false;
        };
        if field != self.field {
            return false;
        }
        match (
            self.position["type"].as_str(),
            namespace.as_str(),
            indices.as_slice(),
        ) {
            (Some("text"), "socXml.text", [start, end]) => {
                self.decimal("start") == Some(*start) && self.decimal("end") == Some(*end)
            }
            (Some("cryXml"), "socXml.cryXml", [node]) => {
                self.position["node"] == *node && self.position["attribute"].is_null()
            }
            (Some("cryXml"), "socXml.cryXml", [node, attribute]) => {
                self.position["node"] == *node && self.position["attribute"] == *attribute
            }
            _ => false,
        }
    }

    fn decimal(&self, field: &str) -> Option<u64> {
        self.position[field].as_str()?.parse().ok()
    }
}
