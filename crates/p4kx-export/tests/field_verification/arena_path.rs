// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use serde_json::Value;

use crate::catalog_v3_consumer as consumer;

use super::error::{self, Error, Kind};
use super::path::MAX_VALUES;

struct Edge<'arena> {
    reference: &'arena Value,
    selected: bool,
}

pub(super) fn select<'arena>(
    arena: &'arena Value,
    owner: &str,
    name: &str,
    output: &mut Vec<&'arena Value>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<()> {
    if arena.is_null() {
        return Ok(());
    }
    let values = array(arena, "values")?;
    if values.len() > MAX_VALUES {
        return Err(Error::new(Kind::Bounds, "authored arena exceeds the projection bound").into());
    }
    let mut stack = Vec::new();
    push_fields(&mut stack, array(arena, "roots")?, None, cancelled)?;
    let mut visited = HashSet::new();
    while let Some(edge) = stack.pop() {
        error::check(cancelled)?;
        let index = index(edge.reference)?;
        if !visited.insert(index) {
            return Err(
                Error::new(Kind::Input, "authored arena repeats an owned occurrence").into(),
            );
        }
        let node = values
            .get(index)
            .ok_or("authored arena valueIndex is outside its arena")?;
        let cell = field(node, "value")?;
        let Some(payload) = consumer::source_value(cell)? else {
            continue;
        };
        if edge.selected {
            if payload.get("type").and_then(Value::as_str) != Some("recordReference") {
                return Err(Error::new(
                    Kind::Mapping,
                    "reviewed arena field is not a Record Reference",
                )
                .into());
            }
            if output.len() >= MAX_VALUES {
                return Err(Error::new(Kind::Bounds, "projection result exceeds its bound").into());
            }
            output.push(payload);
        }
        push_children(&mut stack, payload, owner, name, cancelled)?;
    }
    if visited.len() != values.len() {
        return Err(Error::new(Kind::Input, "authored arena has unreachable values").into());
    }
    Ok(())
}

fn push_children<'arena>(
    stack: &mut Vec<Edge<'arena>>,
    payload: &'arena Value,
    owner: &str,
    name: &str,
    cancelled: &impl Fn() -> bool,
) -> super::Result<()> {
    match payload.get("type").and_then(Value::as_str) {
        Some("struct") => {
            let kind = consumer::present(field(payload, "sourceType")?)?
                .as_str()
                .ok_or("authored arena concrete owner type")?;
            push_fields(
                stack,
                array(payload, "fields")?,
                (kind == owner).then_some(name),
                cancelled,
            )
        }
        Some("array") => {
            for reference in array(payload, "elements")?.iter().rev() {
                error::check(cancelled)?;
                push(
                    stack,
                    Edge {
                        reference,
                        selected: false,
                    },
                )?;
            }
            Ok(())
        }
        Some(_) => Ok(()),
        None => Err(Error::new(Kind::Input, "authored arena node has no payload type").into()),
    }
}

fn push_fields<'arena>(
    stack: &mut Vec<Edge<'arena>>,
    fields: &'arena [Value],
    selected_name: Option<&str>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<()> {
    for source in fields.iter().rev() {
        error::check(cancelled)?;
        let name = consumer::present(field(source, "name")?)?
            .as_str()
            .ok_or("authored arena field name")?;
        push(
            stack,
            Edge {
                reference: field(source, "value")?,
                selected: Some(name) == selected_name,
            },
        )?;
    }
    Ok(())
}

fn push<'arena>(stack: &mut Vec<Edge<'arena>>, edge: Edge<'arena>) -> super::Result<()> {
    if stack.len() >= MAX_VALUES {
        return Err(Error::new(Kind::Bounds, "authored arena exceeds the projection bound").into());
    }
    stack.push(edge);
    Ok(())
}

fn index(reference: &Value) -> super::Result<usize> {
    let text = field(reference, "valueIndex")?
        .as_str()
        .ok_or("authored arena decimal valueIndex")?;
    if text.is_empty()
        || !text.bytes().all(|digit| digit.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(Error::new(Kind::Input, "invalid authored arena decimal valueIndex").into());
    }
    Ok(text.parse()?)
}

fn field<'arena>(value: &'arena Value, key: &str) -> super::Result<&'arena Value> {
    value
        .get(key)
        .ok_or_else(|| format!("authored arena is missing {key}").into())
}

fn array<'arena>(value: &'arena Value, key: &str) -> super::Result<&'arena [Value]> {
    field(value, key)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("authored arena {key} must be an array").into())
}
