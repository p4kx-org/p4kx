// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::Value;

use crate::commodity_v3_fixture::TestResult;
use crate::gameplay_v3_original_numbers as numbers;
use crate::gameplay_v3_original_origins as origins;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support as support;

struct Frame<'a, 'f, 'j> {
    original: &'a StructValue<'f>,
    reference: &'j Value,
    path: String,
}

pub(crate) fn check(catalog: &Value, database: &DataForgeBuilder, record: u32) -> TestResult {
    source::with_tree(database, record, |tree| {
        let row = support::row(catalog, u64::from(record))?;
        let carrier = support::at(row, "/displayTransformation")?;
        let raw = tree.root().field("displayTransformation");
        let Some(root) = pointer(catalog, carrier, raw, record, "displayTransformation")? else {
            return Ok(());
        };
        let graph = support::at(carrier, "/value")?;
        walk(catalog, graph, root, record)
    })
}

fn walk(catalog: &Value, graph: &Value, root: &StructValue<'_>, record: u32) -> TestResult {
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("original occurrence vector")?;
    let mut frames = vec![Frame {
        original: root,
        reference: graph.get("root").ok_or("original root link")?,
        path: "displayTransformation".to_owned(),
    }];
    let mut next = 0_usize;
    while let Some(frame) = frames.pop() {
        if frame.reference.get("nodeIndex").and_then(Value::as_u64) != Some(u64::try_from(next)?) {
            return Err("the original occurrence preorder differs".into());
        }
        let node = nodes.get(next).ok_or("retained original occurrence")?;
        next += 1;
        let children = node_fields(catalog, node, &frame, record)?;
        frames.extend(children.into_iter().rev());
    }
    if next != nodes.len() {
        return Err("an exported occurrence has no original source counterpart".into());
    }
    Ok(())
}

fn node_fields<'a, 'f, 'j>(
    catalog: &Value,
    node: &'j Value,
    frame: &Frame<'a, 'f, 'j>,
    record: u32,
) -> TestResult<Vec<Frame<'a, 'f, 'j>>> {
    let source_type = node
        .get("sourceType")
        .ok_or("original concrete type cell")?;
    let actual = origins::state(source_type, "present")?.ok_or("original concrete type")?;
    if actual.as_str() != Some(frame.original.type_name()) {
        return Err("the original concrete source type differs".into());
    }
    let type_path = format!("{}.$type", frame.path);
    origins::body(catalog, source_type, record, &type_path, "selector")?;
    let carrier = node.get("payload").ok_or("original source payload cell")?;
    let expected = schema::TRANSFORMS
        .iter()
        .find(|(name, _)| *name == frame.original.type_name());
    let Some((_, tag)) = expected else {
        origins::state(carrier, "unsupported")?;
        origins::body(catalog, carrier, record, &type_path, "selector")?;
        return Ok(Vec::new());
    };
    let payload = origins::state(carrier, "present")?.ok_or("original selected payload")?;
    if payload.get("type").and_then(Value::as_str) != Some(*tag) {
        return Err("the declared original operation shape differs".into());
    }
    origins::body(catalog, carrier, record, &type_path, "selector")?;
    match frame.original.type_name() {
        schema::SCALE => {
            scale(catalog, payload, frame, record)?;
            Ok(Vec::new())
        }
        schema::SEQUENCE => sequence(catalog, payload, frame, record),
        _ => Ok(Vec::new()),
    }
}

fn scale(catalog: &Value, payload: &Value, frame: &Frame<'_, '_, '_>, record: u32) -> TestResult {
    let scale = payload.get("scale").ok_or("original scale cell")?;
    let raw = frame.original.field("scale");
    numbers::check(scale, raw)?;
    if !matches!(raw, Some(Raw::Error(_))) {
        let role = if raw.is_none() { "selector" } else { "payload" };
        origins::body(
            catalog,
            scale,
            record,
            &format!("{}.scale", frame.path),
            role,
        )?;
    }
    Ok(())
}

fn sequence<'a, 'f, 'j>(
    catalog: &Value,
    payload: &'j Value,
    frame: &Frame<'a, 'f, 'j>,
    record: u32,
) -> TestResult<Vec<Frame<'a, 'f, 'j>>> {
    let carrier = payload
        .get("transformations")
        .ok_or("original child-array carrier")?;
    let path = format!("{}.transformations", frame.path);
    let Some(raw) = array(
        catalog,
        carrier,
        frame.original.field("transformations"),
        record,
        &path,
    )?
    else {
        return Ok(Vec::new());
    };
    let values = support::at(carrier, "/value")?
        .as_array()
        .ok_or("retained ordered slots")?;
    if raw.len() != values.len() {
        return Err("the original slot cardinality differs".into());
    }
    let mut children = Vec::new();
    for (ordinal, (raw, carrier)) in raw.iter().zip(values).enumerate() {
        let path = format!("{path}[{ordinal}]");
        if let Some(original) = pointer(catalog, carrier, Some(raw), record, &path)? {
            children.push(Frame {
                original,
                reference: support::at(carrier, "/value")?,
                path,
            });
        }
    }
    Ok(children)
}

fn pointer<'a, 'f>(
    catalog: &Value,
    carrier: &Value,
    raw: Option<&'a Raw<'f>>,
    record: u32,
    path: &str,
) -> TestResult<Option<&'a StructValue<'f>>> {
    let (state, source) = match raw {
        None => ("absent", None),
        Some(Raw::Null) => ("null", None),
        Some(Raw::Error(_)) => ("unreadable", None),
        Some(Raw::Struct(value)) => ("present", Some(value)),
        Some(_) => ("invalid", None),
    };
    origins::state(carrier, state)?;
    if state != "unreadable" {
        origins::body(
            catalog,
            carrier,
            record,
            path,
            if state == "absent" {
                "selector"
            } else {
                "payload"
            },
        )?;
    }
    Ok(source)
}

fn array<'a, 'f>(
    catalog: &Value,
    carrier: &Value,
    raw: Option<&'a Raw<'f>>,
    record: u32,
    path: &str,
) -> TestResult<Option<&'a [Raw<'f>]>> {
    let (state, source) = match raw {
        None => ("absent", None),
        Some(Raw::Null) => ("null", None),
        Some(Raw::Error(_)) => ("unreadable", None),
        Some(Raw::Array(values)) => ("present", Some(values.as_ref())),
        Some(_) => ("invalid", None),
    };
    origins::state(carrier, state)?;
    if state != "unreadable" {
        origins::body(
            catalog,
            carrier,
            record,
            path,
            if state == "absent" {
                "selector"
            } else {
                "payload"
            },
        )?;
    }
    Ok(source)
}
