// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Schema-driven exchange filtering for the real canonical text contract.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use jsonschema::Validator;
use serde_json::{Map, Value};

use crate::resource_origin_fixture::TestResult;

pub(super) fn public_projection(catalog: &Value, schema: &Value) -> TestResult<Value> {
    let context = SchemaWalkContext::new(schema);
    project(&context, &[schema], catalog)?.ok_or_else(|| "catalog root is game text".into())
}

fn schema_nodes<'s>(
    context: &SchemaWalkContext<'s>,
    seeds: &[&'s Value],
    value: &Value,
) -> TestResult<Vec<&'s Value>> {
    fn expand<'s>(
        context: &SchemaWalkContext<'s>,
        node: &'s Value,
        value: &Value,
        seen: &mut BTreeSet<&'s str>,
        output: &mut Vec<&'s Value>,
    ) -> TestResult {
        output.push(node);
        if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
            let pointer = reference
                .strip_prefix('#')
                .ok_or("nonlocal schema reference")?;
            let target = context
                .root
                .pointer(pointer)
                .ok_or("missing schema reference")?;
            if seen.insert(reference) {
                expand(context, target, value, seen, output)?;
            }
        }
        if let Some(branches) = node.get("allOf").and_then(Value::as_array) {
            for branch in branches {
                expand(context, branch, value, seen, output)?;
            }
        }
        for keyword in ["anyOf", "oneOf"] {
            if let Some(branches) = node.get(keyword).and_then(Value::as_array) {
                for branch in branches {
                    if context.applies(branch, value)? {
                        expand(context, branch, value, seen, output)?;
                    }
                }
            }
        }
        if let Some(condition) = node.get("if") {
            let branch = if context.applies(condition, value)? {
                "then"
            } else {
                "else"
            };
            if let Some(selected) = node.get(branch) {
                expand(context, selected, value, seen, output)?;
            }
        }
        Ok(())
    }
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for seed in seeds {
        expand(context, seed, value, &mut seen, &mut output)?;
    }
    Ok(output)
}

struct SchemaWalkContext<'s> {
    root: &'s Value,
    validators: RefCell<BTreeMap<String, Validator>>,
}

impl<'s> SchemaWalkContext<'s> {
    const fn new(root: &'s Value) -> Self {
        Self {
            root,
            validators: RefCell::new(BTreeMap::new()),
        }
    }

    fn applies(&self, condition: &Value, value: &Value) -> TestResult<bool> {
        let key = serde_json::to_string(condition)?;
        {
            let validators = self.validators.borrow();
            if let Some(validator) = validators.get(&key) {
                return Ok(validator.is_valid(value));
            }
        }
        let mut document = serde_json::json!({ "allOf": [condition] });
        let object = document
            .as_object_mut()
            .ok_or("condition schema is not an object")?;
        for keyword in ["$schema", "$defs"] {
            if let Some(context) = self.root.get(keyword) {
                object.insert(keyword.to_owned(), context.clone());
            }
        }
        let validator = jsonschema::validator_for(&document)?;
        let matches = validator.is_valid(value);
        self.validators.borrow_mut().insert(key, validator);
        Ok(matches)
    }
}

fn children<'s>(schemas: &[&'s Value], name: &str) -> Vec<&'s Value> {
    schemas
        .iter()
        .filter_map(|schema| {
            schema
                .get("properties")
                .and_then(|properties| properties.get(name))
                .or_else(|| {
                    schema
                        .get("additionalProperties")
                        .filter(|additional| additional.is_object())
                })
        })
        .collect()
}

fn items<'s>(schemas: &[&'s Value]) -> Vec<&'s Value> {
    schemas
        .iter()
        .filter_map(|schema| schema.get("items"))
        .collect()
}

fn project(
    context: &SchemaWalkContext<'_>,
    seeds: &[&Value],
    value: &Value,
) -> TestResult<Option<Value>> {
    let schemas = schema_nodes(context, seeds, value)?;
    if schemas
        .iter()
        .any(|schema| schema.get("x-gameText") == Some(&Value::Bool(true)))
    {
        return Ok(None);
    }
    let value = match value {
        Value::Object(fields) => {
            let mut output = Map::new();
            for (name, child) in fields {
                if let Some(child) = project(context, &children(&schemas, name), child)? {
                    output.insert(name.clone(), child);
                }
            }
            Value::Object(output)
        }
        Value::Array(values) => {
            let mut output = Vec::new();
            let items = items(&schemas);
            for child in values {
                if let Some(child) = project(context, &items, child)? {
                    output.push(child);
                }
            }
            Value::Array(output)
        }
        _ => value.clone(),
    };
    Ok(Some(value))
}

pub(super) fn reference_closure(catalog: &Value, schema: &Value) -> TestResult<[usize; 2]> {
    let names = target_ids(catalog, "publicNames")?;
    let game_texts = target_ids(catalog, "gameTexts")?;
    let mut counts = [0, 0];
    let context = SchemaWalkContext::new(schema);
    visit_refs(
        &context,
        &[schema],
        catalog,
        &names,
        &game_texts,
        &mut counts,
    )?;
    Ok(counts)
}

fn target_ids<'a>(catalog: &'a Value, domain: &str) -> TestResult<BTreeSet<&'a str>> {
    let Some(rows) = catalog.get("texts").and_then(|texts| texts.get(domain)) else {
        return Ok(BTreeSet::new());
    };
    let mut ids = BTreeSet::new();
    for row in rows.as_array().ok_or("text domain is not an array")? {
        let id = row
            .pointer("/id/value")
            .and_then(Value::as_str)
            .ok_or("text row lacks ID")?;
        if !ids.insert(id) {
            return Err("duplicate canonical text ID".into());
        }
    }
    Ok(ids)
}

fn visit_refs(
    context: &SchemaWalkContext<'_>,
    seeds: &[&Value],
    value: &Value,
    names: &BTreeSet<&str>,
    game_texts: &BTreeSet<&str>,
    counts: &mut [usize; 2],
) -> TestResult {
    let schemas = schema_nodes(context, seeds, value)?;
    if let Some(id) = value.as_str() {
        for ((reference, targets), count) in [
            ("#/$defs/PublicNameRef", names),
            ("#/$defs/GameTextRef", game_texts),
        ]
        .into_iter()
        .zip(counts.iter_mut())
        {
            if schemas
                .iter()
                .any(|schema| schema.get("$ref").and_then(Value::as_str) == Some(reference))
            {
                if !targets.contains(id) {
                    return Err("typed text reference leaves its canonical domain".into());
                }
                *count += 1;
            }
        }
    }
    match value {
        Value::Object(fields) => {
            for (name, child) in fields {
                visit_refs(
                    context,
                    &children(&schemas, name),
                    child,
                    names,
                    game_texts,
                    counts,
                )?;
            }
        }
        Value::Array(values) => {
            let items = items(&schemas);
            for child in values {
                visit_refs(context, &items, child, names, game_texts, counts)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{SchemaWalkContext, public_projection};
    use crate::resource_origin_fixture::TestResult;

    #[test]
    fn condition_uses_the_instance_and_document_definitions() -> TestResult {
        let schema = json!({
            "$defs": {
                "Present": {"const": "present"},
                "Protected": {"type": "string", "x-gameText": true}
            },
            "properties": {"tag": {"type": "string"}, "content": true},
            "allOf": [{
                "if": {
                    "properties": {"tag": {"$ref": "#/$defs/Present"}},
                    "required": ["tag"]
                },
                "then": {"properties": {"content": {"$ref": "#/$defs/Protected"}}},
                "else": {"properties": {"content": {"type": "null"}}}
            }]
        });
        assert_eq!(
            public_projection(&json!({"tag": "present", "content": "private"}), &schema)?,
            json!({"tag": "present"}),
            "the selected protected branch is traversed through its reference"
        );
        assert_eq!(
            public_projection(&json!({"tag": "absent", "content": null}), &schema)?,
            json!({"tag": "absent", "content": null}),
            "the unselected protected branch does not hide the else value"
        );
        Ok(())
    }

    #[test]
    fn an_unselected_union_branch_does_not_hide_public_data() -> TestResult {
        let schema = json!({
            "oneOf": [
                {"type": "string"},
                {"type": "integer", "x-gameText": true}
            ]
        });
        assert_eq!(
            public_projection(&json!("public"), &schema)?,
            json!("public"),
            "union alternatives are selected by the real validator"
        );
        assert!(
            public_projection(&json!(7), &schema).is_err(),
            "a selected protected root is refused"
        );
        Ok(())
    }

    #[test]
    fn compiled_conditions_are_reused_with_distinct_instance_results() -> TestResult {
        let schema = json!({"$defs": {"Present": {"const": "present"}}});
        let context = SchemaWalkContext::new(&schema);
        assert!(
            context.applies(&json!({"$ref": "#/$defs/Present"}), &json!("present"))?,
            "the first instance satisfies the referenced condition"
        );
        assert!(
            !context.applies(&json!({"$ref": "#/$defs/Present"}), &json!("absent"))?,
            "reusing the validator still evaluates the next instance"
        );
        assert_eq!(
            context.validators.borrow().len(),
            1,
            "equal condition schemas share one compiled validator within a walk"
        );
        Ok(())
    }
}
