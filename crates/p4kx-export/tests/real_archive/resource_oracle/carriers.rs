// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "dcb_binding.rs"]
mod dcb_binding;
#[path = "source_coordinate.rs"]
mod source_coordinate;

use std::collections::{BTreeSet, HashMap};

use p4kx_extraction::{DataForgeOrigin, SourceEncryption, SourceUseRole};
use serde_json::Value;

use self::dcb_binding::dcb_source;
use self::source_coordinate::Coordinate;
use super::{Failure, Result};

const MAX_USES: usize = 262_144;
const MAX_VALUE_NODES: usize = 65_536;
const MAX_DEPTH: usize = 32;

pub(super) struct Uses<'catalog> {
    rows: HashMap<&'catalog str, &'catalog Value>,
    dcb: &'catalog str,
    marker: Value,
    roles: [Value; 3],
}

impl<'catalog> Uses<'catalog> {
    pub(super) fn new(catalog: &'catalog Value, origin: &DataForgeOrigin) -> Result<Self> {
        let dcb = dcb_source(catalog, origin)?;
        let rows = catalog
            .pointer("/provenance/uses")
            .and_then(Value::as_array)
            .ok_or_else(|| Failure::check("use-inventory"))?;
        if rows.len() > MAX_USES {
            return Err(Failure::check("use-inventory-budget"));
        }
        let mut mapped = HashMap::new();
        for row in rows {
            let id = row["id"].as_str().ok_or_else(|| Failure::check("use-id"))?;
            if mapped.insert(id, row).is_some() {
                return Err(Failure::check("duplicate-use-id"));
            }
        }
        let marker = match origin.encryption() {
            SourceEncryption::Encrypted => Value::Bool(true),
            SourceEncryption::Unencrypted => Value::Bool(false),
            _ => Value::Null,
        };
        Ok(Self {
            rows: mapped,
            dcb,
            marker,
            roles: contributing_roles()?,
        })
    }

    pub(super) fn record_index(&self, row: &Value) -> Result<u32> {
        let carrier = row
            .get("guid")
            .ok_or_else(|| Failure::check("resource-guid-origin"))?;
        if carrier["state"] != "present" || !self.valid_origin(carrier)? {
            return Err(Failure::check("resource-guid-origin"));
        }
        let references = carrier["sourceUsesRefs"]
            .as_array()
            .ok_or_else(|| Failure::check("resource-guid-origin"))?;
        let mut indices = BTreeSet::new();
        for reference in references {
            let observation = self.get(reference)?;
            if observation["source"] == self.dcb
                && observation["role"] == "payload"
                && observation["scope"] == "payload"
            {
                let (index, coordinate) = source_coordinate::read(observation)?;
                if coordinate == Coordinate::Header("guid") {
                    indices.insert(index);
                }
            }
        }
        let mut indices = indices.into_iter();
        let first = indices
            .next()
            .ok_or_else(|| Failure::check("physical-resource-owner"))?;
        if indices.next().is_some() {
            return Err(Failure::check("ambiguous-resource-owner"));
        }
        Ok(first)
    }

    fn get(&self, reference: &Value) -> Result<&'catalog Value> {
        reference
            .as_str()
            .and_then(|id| self.rows.get(id).copied())
            .ok_or_else(|| Failure::check("source-use-reference-closure"))
    }

    fn valid_origin(&self, carrier: &Value) -> Result<bool> {
        if carrier.get("containsEncryptedSource") != Some(&self.marker)
            || carrier.get("sourceSetComplete").and_then(Value::as_bool) != Some(true)
        {
            return Ok(false);
        }
        let Some(references) = carrier.get("sourceUsesRefs").and_then(Value::as_array) else {
            return Ok(false);
        };
        if references.is_empty() {
            return Ok(false);
        }
        let mut distinct = BTreeSet::new();
        for reference in references {
            let observation = self.get(reference)?;
            if !distinct.insert(reference.as_str())
                || observation["source"] != self.dcb
                || observation["scope"] != "payload"
                || !self.roles.contains(&observation["role"])
            {
                return Ok(false);
            }
            let _coordinate = source_coordinate::read(observation)?;
        }
        Ok(true)
    }
}

fn contributing_roles() -> Result<[Value; 3]> {
    let encode = |role| {
        serde_json::to_value(role).map_err(|error| Failure::source("source-role-contract", error))
    };
    Ok([
        encode(SourceUseRole::Payload)?,
        encode(SourceUseRole::Selector)?,
        encode(SourceUseRole::Conversion)?,
    ])
}

pub(super) struct Carriers<'catalog> {
    row: &'catalog Value,
    owner: u32,
    fields: HashMap<(u32, Coordinate<'catalog>), Vec<&'catalog Value>>,
}

impl<'catalog> Carriers<'catalog> {
    pub(super) fn new(row: &'catalog Value, uses: &Uses<'catalog>, owner: u32) -> Result<Self> {
        let mut index = Self {
            row,
            owner,
            fields: HashMap::new(),
        };
        index.walk(row, uses, 0, &mut 0)?;
        Ok(index)
    }

    fn walk(
        &mut self,
        value: &'catalog Value,
        uses: &Uses<'catalog>,
        depth: usize,
        visited: &mut usize,
    ) -> Result<()> {
        if depth > MAX_DEPTH || *visited >= MAX_VALUE_NODES {
            return Err(Failure::check("carrier-walk-budget"));
        }
        *visited += 1;
        self.register(value, uses)?;
        match value {
            Value::Object(object) => {
                for child in object.values() {
                    self.walk(child, uses, depth + 1, visited)?;
                }
            }
            Value::Array(array) => {
                for child in array {
                    self.walk(child, uses, depth + 1, visited)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn register(&mut self, value: &'catalog Value, uses: &Uses<'catalog>) -> Result<()> {
        if !uses.valid_origin(value)? {
            return Ok(());
        }
        let references = value["sourceUsesRefs"]
            .as_array()
            .ok_or_else(|| Failure::check("field-source-uses"))?;
        for reference in references {
            let observation = uses.get(reference)?;
            let (owner, field) = source_coordinate::read(observation)?;
            self.fields.entry((owner, field)).or_default().push(value);
        }
        Ok(())
    }

    pub(super) fn root(&self, name: &str) -> Result<&'catalog Value> {
        let carrier = self
            .row
            .get(name)
            .ok_or_else(|| Failure::check("canonical-root-field"))?;
        self.require(carrier, name)?;
        Ok(carrier)
    }

    pub(super) fn require(&self, carrier: &Value, path: &str) -> Result<()> {
        self.require_at(carrier, self.owner, path)
    }

    pub(super) fn require_at(&self, carrier: &Value, owner: u32, path: &str) -> Result<()> {
        if self
            .fields
            .get(&(owner, Coordinate::Body(path)))
            .is_some_and(|values| {
                values
                    .iter()
                    .any(|candidate| std::ptr::eq(*candidate, carrier))
            })
        {
            return Ok(());
        }
        Err(Failure::check("bound-field-origin"))
    }

    pub(super) fn require_header_at(&self, carrier: &Value, owner: u32, field: &str) -> Result<()> {
        if self
            .fields
            .get(&(owner, Coordinate::Header(field)))
            .is_some_and(|values| {
                values
                    .iter()
                    .any(|candidate| std::ptr::eq(*candidate, carrier))
            })
        {
            return Ok(());
        }
        Err(Failure::check("bound-header-origin"))
    }

    pub(super) fn child(
        &self,
        parent: &'catalog Value,
        path: &str,
        name: &str,
    ) -> Result<&'catalog Value> {
        let wanted = format!("{path}.{name}");
        let mut object =
            super::compare::present(parent).ok_or_else(|| Failure::check("owned-parent-state"))?;
        for _ in 0..=MAX_DEPTH {
            if let Some(carrier) = object.get(name) {
                self.require(carrier, &wanted)?;
                return Ok(carrier);
            }
            if let Some(payload) = object.get("payload") {
                self.require(payload, &format!("{path}.$type"))?;
                object = super::compare::present(payload)
                    .ok_or_else(|| Failure::check("owned-payload-state"))?;
            } else if let Some(value) = object.get("value") {
                self.require(value, path)?;
                object = super::compare::present(value)
                    .ok_or_else(|| Failure::check("owned-inline-state"))?;
            } else if let Some(reference) = object.get("reference") {
                self.require(reference, &wanted)?;
                return Ok(reference);
            } else {
                return Err(Failure::check("canonical-child-field"));
            }
        }
        Err(Failure::check("canonical-child-depth"))
    }

    pub(super) fn source_type(
        &self,
        parent: &'catalog Value,
        path: &str,
    ) -> Result<&'catalog Value> {
        let carrier = super::compare::present(parent)
            .and_then(|value| value.get("sourceType"))
            .ok_or_else(|| Failure::check("canonical-source-type"))?;
        self.require(carrier, &format!("{path}.$type"))?;
        Ok(carrier)
    }

    pub(super) fn array(&self, path: &str, length: usize) -> Option<&'catalog Vec<Value>> {
        super::compare::present(self.root(path).ok()?)?
            .as_array()
            .filter(|array| array.len() == length)
    }

    pub(super) fn contains(&self, path: &str, carrier: &Value) -> bool {
        self.require(carrier, path).is_ok()
    }
}

#[cfg(test)]
#[path = "carriers_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "source_identity_tests.rs"]
mod source_identity_tests;
