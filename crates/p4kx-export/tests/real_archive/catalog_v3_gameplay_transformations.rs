// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::Value;

use super::super as consumer;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TransformationError {
    #[error("invalid transformation source carrier: {0}")]
    Source(#[source] Box<dyn Error>),
    #[error("invalid transformation shape: {0}")]
    Shape(&'static str),
    #[error("transformation root must be local index zero")]
    Root,
    #[error("a present transformation needs at least one node")]
    Empty,
    #[error("a local node reference is outside its owning vector")]
    Range,
    #[error("a local node reference creates a cycle")]
    Cycle,
    #[error("a local node has more than one owner")]
    Ownership,
    #[error("a local node is unreachable from its own root")]
    Unreachable,
    #[error("local nodes do not follow original child-slot preorder")]
    Order,
}

#[derive(Clone, Copy)]
enum Color {
    Unseen,
    Active,
    Complete,
}

enum Frame {
    Enter(usize),
    Exit(usize),
}

pub(super) fn check(carrier: &Value) -> Result<(), TransformationError> {
    let Some(graph) = sourced(carrier)? else {
        return Ok(());
    };
    let root = index(
        graph
            .get("root")
            .ok_or(TransformationError::Shape("root"))?,
    )?;
    if root != 0 {
        return Err(TransformationError::Root);
    }
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or(TransformationError::Shape("nodes"))?;
    if nodes.is_empty() {
        return Err(TransformationError::Empty);
    }
    walk(nodes)
}

fn walk(nodes: &[Value]) -> Result<(), TransformationError> {
    let mut colors = vec![Color::Unseen; nodes.len()];
    let mut frames = vec![Frame::Enter(0)];
    let mut visited = 0_usize;
    let mut ordered = true;
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(index) => {
                match colors
                    .get(index)
                    .copied()
                    .ok_or(TransformationError::Range)?
                {
                    Color::Active => return Err(TransformationError::Cycle),
                    Color::Complete => return Err(TransformationError::Ownership),
                    Color::Unseen => {}
                }
                *colors.get_mut(index).ok_or(TransformationError::Range)? = Color::Active;
                ordered &= index == visited;
                visited += 1;
                let node = nodes.get(index).ok_or(TransformationError::Range)?;
                frames.push(Frame::Exit(index));
                for child in children(node)?.into_iter().rev() {
                    frames.push(Frame::Enter(child));
                }
            }
            Frame::Exit(index) => {
                *colors.get_mut(index).ok_or(TransformationError::Range)? = Color::Complete;
            }
        }
    }
    if visited != nodes.len() {
        return Err(TransformationError::Unreachable);
    }
    if !ordered {
        return Err(TransformationError::Order);
    }
    Ok(())
}

fn children(node: &Value) -> Result<Vec<usize>, TransformationError> {
    let name = consumer::present(
        node.get("sourceType")
            .ok_or(TransformationError::Shape("sourceType"))?,
    )
    .map_err(TransformationError::Source)?;
    if !name.is_string() {
        return Err(TransformationError::Shape("sourceType string"));
    }
    let Some(payload) = sourced(
        node.get("payload")
            .ok_or(TransformationError::Shape("payload"))?,
    )?
    else {
        return Ok(Vec::new());
    };
    match payload.get("type").and_then(Value::as_str) {
        Some("sequence") => sequence(payload),
        Some("scale") => {
            if let Some(number) = sourced(
                payload
                    .get("scale")
                    .ok_or(TransformationError::Shape("scale"))?,
            )? && !number.is_object()
            {
                return Err(TransformationError::Shape("raw numeric object"));
            }
            Ok(Vec::new())
        }
        Some(
            "emptyBase"
            | "convertFactorToNegatedPercentChange"
            | "convertFactorToPercentChange"
            | "convertValueToFactorOfBaseValue",
        ) => Ok(Vec::new()),
        _ => Err(TransformationError::Shape("closed operation tag")),
    }
}

fn sequence(payload: &Value) -> Result<Vec<usize>, TransformationError> {
    let mut children = Vec::new();
    let carrier = payload
        .get("transformations")
        .ok_or(TransformationError::Shape("transformations"))?;
    if let Some(values) = sourced(carrier)? {
        for slot in values
            .as_array()
            .ok_or(TransformationError::Shape("ordered child array"))?
        {
            if let Some(reference) = sourced(slot)? {
                children.push(index(reference)?);
            }
        }
    }
    Ok(children)
}

fn index(reference: &Value) -> Result<usize, TransformationError> {
    let index = reference
        .get("nodeIndex")
        .and_then(Value::as_u64)
        .ok_or(TransformationError::Shape("u32 nodeIndex"))?;
    let index = u32::try_from(index).map_err(|_| TransformationError::Shape("u32 nodeIndex"))?;
    usize::try_from(index).map_err(|_| TransformationError::Shape("addressable nodeIndex"))
}

fn sourced(carrier: &Value) -> Result<Option<&Value>, TransformationError> {
    consumer::source_value(carrier).map_err(TransformationError::Source)
}
