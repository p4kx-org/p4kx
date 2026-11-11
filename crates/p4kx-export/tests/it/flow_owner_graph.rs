// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;

/// Structural navigation through the one actual physical owner's canonical raw inventory.
pub(crate) struct Owner<'catalog> {
    pub(crate) source: &'catalog Value,
    pub(crate) values: &'catalog [Value],
}

impl<'catalog> Owner<'catalog> {
    pub(crate) fn new(definition: &'catalog Value) -> TestResult<Self> {
        let source = definition
            .get("source")
            .ok_or("one physical owner source inventory")?;
        let values = raw::array(source, "values")?;
        let mut previous = None;
        for node in values {
            let ordinal = ordinal(node)?;
            assert!(
                previous.is_none_or(|previous| previous < ordinal),
                "one actual occurrence per owner, in strictly increasing original DFS order"
            );
            previous = Some(ordinal);
        }
        Ok(Self { source, values })
    }

    pub(crate) fn follow(&self, reference: &Value) -> TestResult<&'catalog Value> {
        let index = decimal(reference, "valueIndex")?;
        self.values
            .get(index)
            .ok_or_else(|| "reference resolves inside this physical owner".into())
    }

    pub(crate) fn field(&self, reference: &Value) -> TestResult<&'catalog Value> {
        match reference.get("type").and_then(Value::as_str) {
            Some("root") => raw::array(self.source, "roots")?
                .get(decimal(reference, "rootFieldIndex")?)
                .ok_or_else(|| "actual owner root field".into()),
            Some("field") => {
                let parent =
                    self.follow(reference.get("parent").ok_or("typed actual parent link")?)?;
                raw::array(raw::payload(parent, "struct")?, "fields")?
                    .get(decimal(reference, "fieldIndex")?)
                    .ok_or_else(|| "actual field ordinal within original parent".into())
            }
            _ => Err("closed SourceFieldRef representation".into()),
        }
    }

    pub(crate) fn reference(&self, original_ordinal: u64) -> TestResult<Value> {
        let mut matches = Vec::new();
        for (index, node) in self.values.iter().enumerate() {
            if ordinal(node)? == original_ordinal {
                matches.push(index);
            }
        }
        assert_eq!(
            matches.len(),
            1,
            "one actual source ordinal has one owner-local cell"
        );
        let index = matches
            .first()
            .ok_or("actual original occurrence is in this owner")?;
        Ok(json!({"valueIndex":index.to_string()}))
    }

    pub(crate) fn position(&self, reference: &Value) -> TestResult<(Option<u64>, u64)> {
        match reference.get("type").and_then(Value::as_str) {
            Some("root") => {
                let field = self.field(reference)?;
                let position = field
                    .get("sourcePosition")
                    .ok_or("original cut-root position")?;
                let parent = match position.get("parentOrdinal") {
                    Some(Value::Null) => None,
                    Some(Value::String(text)) => Some(number(text)?),
                    _ => return Err("required nullable original parent ordinal".into()),
                };
                let index = position
                    .get("fieldIndex")
                    .and_then(Value::as_str)
                    .ok_or("original field index")?;
                Ok((parent, number(index)?))
            }
            Some("field") => {
                let parent =
                    self.follow(reference.get("parent").ok_or("actual parent reference")?)?;
                let index = reference
                    .get("fieldIndex")
                    .and_then(Value::as_str)
                    .ok_or("actual field ordinal")?;
                self.field(reference)?;
                Ok((Some(ordinal(parent)?), number(index)?))
            }
            _ => Err("closed SourceFieldRef representation".into()),
        }
    }
    pub(crate) fn members(&self, configuration: &Value) -> TestResult<Vec<&'catalog Value>> {
        let mut pending = Vec::new();
        let declaration = configuration
            .get("declaration")
            .ok_or("required declaration binding")?;
        match declaration.get("state").and_then(Value::as_str) {
            Some("present") => pending.push(
                self.field(
                    declaration
                        .get("field")
                        .ok_or("actual present source field navigation")?,
                )?
                .get("value")
                .ok_or("one actual field value reference")?,
            ),
            Some("absent") => {}
            _ => return Err("present declaration and genuine absence remain distinct".into()),
        }
        for selected in raw::array(configuration, "referencedFields")? {
            pending.push(
                self.field(selected)?
                    .get("value")
                    .ok_or("selected field value")?,
            );
        }
        let mut indices = BTreeSet::new();
        while let Some(reference) = pending.pop() {
            let index = decimal(reference, "valueIndex")?;
            if !indices.insert(index) {
                continue;
            }
            let node = self.follow(reference)?;
            if node.pointer("/value/state") != Some(&json!("present")) {
                continue;
            }
            match node.pointer("/value/value/type").and_then(Value::as_str) {
                Some("struct") => {
                    for field in raw::array(raw::payload(node, "struct")?, "fields")? {
                        pending.push(
                            field
                                .get("value")
                                .ok_or("every actual original child link")?,
                        );
                    }
                }
                Some("array") => pending.extend(raw::elements(node)?.iter()),
                Some(_) => {}
                None => return Err("present original values have a typed payload".into()),
            }
        }
        indices
            .into_iter()
            .map(|index| {
                self.values
                    .get(index)
                    .ok_or_else(|| "owner-local membership resolves".into())
            })
            .collect()
    }
}

pub(crate) fn ordinal(node: &Value) -> TestResult<u64> {
    let text = node
        .get("sourceOrdinal")
        .and_then(Value::as_str)
        .ok_or("actual original source ordinal")?;
    let number: u64 = text.parse()?;
    assert_eq!(
        text,
        number.to_string(),
        "canonical exact source ordinal encoding"
    );
    Ok(number)
}

fn decimal(reference: &Value, key: &str) -> TestResult<usize> {
    let text = reference
        .get(key)
        .and_then(Value::as_str)
        .ok_or("decimal structural reference")?;
    let number: u64 = text.parse()?;
    assert_eq!(
        text,
        number.to_string(),
        "canonical DecimalU64 structural encoding"
    );
    Ok(usize::try_from(number)?)
}

fn number(text: &str) -> TestResult<u64> {
    let number: u64 = text.parse()?;
    assert_eq!(
        text,
        number.to_string(),
        "canonical exact original position encoding"
    );
    Ok(number)
}
