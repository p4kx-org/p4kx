// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use serde_json::{Map, Value};

use super::schema_navigation::escape;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Address {
    pub(super) wire: String,
    pub(super) schema: String,
}

impl Address {
    fn property(&self, name: &str) -> Self {
        let name = escape(name);
        Self {
            wire: format!("{}/{name}", self.wire),
            schema: format!("{}/properties/{name}", self.schema),
        }
    }

    fn items(&self) -> Self {
        Self {
            wire: format!("{}/[]", self.wire),
            schema: format!("{}/items", self.schema),
        }
    }

    fn edge(&self, segment: &str) -> Self {
        Self {
            wire: self.wire.clone(),
            schema: format!("{}/{segment}", self.schema),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Inventory {
    pub(super) fields: BTreeSet<Address>,
    pub(super) recursion: BTreeSet<Address>,
}

pub(super) fn released_schema() -> Result<Value> {
    Ok(serde_json::from_str(include_str!(
        "../fixtures/catalog-v0.4.2.schema.json"
    ))?)
}

impl Inventory {
    pub(super) fn collect(schema: &Value) -> Result<Self> {
        let mut inventory = Self::default();
        let root = Address {
            wire: String::new(),
            schema: String::new(),
        };
        inventory.visit(schema, schema, &root, &mut Vec::new())?;
        Ok(inventory)
    }

    fn visit<'schema>(
        &mut self,
        schema: &'schema Value,
        node: &'schema Value,
        address: &Address,
        active: &mut Vec<&'schema Value>,
    ) -> Result {
        if active.iter().any(|prior| std::ptr::eq(*prior, node)) {
            self.recursion.insert(address.clone());
            return Ok(());
        }
        if node.is_boolean() {
            return Ok(());
        }
        let node_fields = node
            .as_object()
            .ok_or_else(|| format!("invalid schema at #{}", address.schema))?;
        for field in node_fields.keys() {
            check_keyword(field, address)?;
        }
        active.push(node);
        self.visit_reference(schema, node_fields, address, active)?;
        self.visit_children(schema, node_fields, address, active)?;
        active.pop();
        Ok(())
    }

    fn visit_reference<'schema>(
        &mut self,
        schema: &'schema Value,
        fields: &'schema Map<String, Value>,
        address: &Address,
        active: &mut Vec<&'schema Value>,
    ) -> Result {
        let Some(reference) = fields.get("$ref") else {
            return Ok(());
        };
        let reference = reference
            .as_str()
            .and_then(|reference| reference.strip_prefix('#'))
            .filter(|pointer| pointer.starts_with('/'))
            .ok_or_else(|| format!("expected local reference at #{}", address.schema))?;
        let target = schema
            .pointer(reference)
            .ok_or_else(|| format!("unresolved schema reference #{reference}"))?;
        self.visit(schema, target, &address.edge("$ref"), active)
    }

    fn visit_children<'schema>(
        &mut self,
        schema: &'schema Value,
        fields: &'schema Map<String, Value>,
        address: &Address,
        active: &mut Vec<&'schema Value>,
    ) -> Result {
        if let Some(properties) = fields.get("properties") {
            let properties = properties
                .as_object()
                .ok_or_else(|| format!("invalid properties at #{}", address.schema))?;
            for (name, child) in properties {
                let property = address.property(name);
                self.visit(schema, child, &property, active)?;
                self.fields.insert(property);
            }
        }
        if let Some(items) = fields.get("items") {
            self.visit(schema, items, &address.items(), active)?;
        }
        for keyword in ["allOf", "anyOf", "oneOf"] {
            let Some(branches) = fields.get(keyword) else {
                continue;
            };
            let branches = branches
                .as_array()
                .ok_or_else(|| format!("invalid {keyword} at #{}", address.schema))?;
            for (index, branch) in branches.iter().enumerate() {
                let branch_address = address.edge(&format!("{keyword}/{index}"));
                self.visit(schema, branch, &branch_address, active)?;
            }
        }
        Ok(())
    }
}

fn check_keyword(keyword: &str, address: &Address) -> Result {
    match keyword {
        "$ref" | "$defs" | "$schema" | "allOf" | "anyOf" | "oneOf" | "properties" | "items"
        | "const" | "description" | "format" | "maxItems" | "minimum" | "minItems" | "pattern"
        | "required" | "title" | "type" | "x-gameText" | "x-values" => Ok(()),
        _ => Err(format!(
            "unsupported schema keyword {keyword} at #{}",
            address.schema
        )
        .into()),
    }
}
