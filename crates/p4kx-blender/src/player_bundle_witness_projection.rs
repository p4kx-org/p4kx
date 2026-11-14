// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use serde_json::{Value, json};

use crate::soc_scene_origin::SocXmlPosition;

use super::{Document, Sources};

fn projected_branch(value: &Value) -> io::Result<Value> {
    let definitions = values(&value["fragmentDefinitions"])?
        .iter()
        .map(|row| {
            Ok(json!([
                row["ordinal"],
                row["position"]["value"],
                projected_attrs(&row["name"])?
            ]))
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(json!([
        value["position"]["value"],
        projected_attrs(&value["globalTags"]["attribute"])?,
        definitions
    ]))
}

pub(super) fn verify_branch(
    value: &Value,
    expected: &Value,
    original: &Document,
    sources: &Sources<'_>,
) -> io::Result<()> {
    equal(&projected_branch(value)?, expected, "ADB branch occurrence")?;
    verify_position(&value["position"], original, sources)?;
    verify_attributes(&value["globalTags"]["attribute"], "Tags", original, sources)?;
    for fragment in values(&value["fragmentDefinitions"])? {
        verify_position(&fragment["position"], original, sources)?;
        verify_attributes(&fragment["name"], "Name", original, sources)?;
    }
    Ok(())
}

pub(super) fn verify_controller(
    value: &Value,
    expected: &Value,
    original: &Document,
    sources: &Sources<'_>,
) -> io::Result<()> {
    let contexts = values(&value["contexts"])?
        .iter()
        .map(projected_node)
        .collect::<io::Result<Vec<_>>>()?;
    let scopes = values(&value["scopes"])?
        .iter()
        .map(|row| projected_node(&row["declaration"]))
        .collect::<io::Result<Vec<_>>>()?;
    let fragments = values(&value["fragments"])?
        .iter()
        .map(|row| {
            let overrides = values(&row["overrides"])?
                .iter()
                .map(|row| projected_node(&row["declaration"]))
                .collect::<io::Result<Vec<_>>>()?;
            Ok(json!([projected_node(&row["declaration"])?, overrides]))
        })
        .collect::<io::Result<Vec<_>>>()?;
    equal(
        &json!([contexts, scopes, fragments]),
        expected,
        "ControllerDef authored source",
    )?;
    for node in values(&value["contexts"])?.iter().chain(
        values(&value["scopes"])?
            .iter()
            .map(|row| &row["declaration"]),
    ) {
        verify_position(&node["position"], original, sources)?;
    }
    Ok(())
}

fn projected_node(node: &Value) -> io::Result<Value> {
    let attributes = values(&node["attributes"])?;
    Ok(json!([
        node["position"]["value"],
        node["name"]["value"],
        attributes
            .iter()
            .map(|attribute| {
                json!([
                    attribute["position"],
                    attribute["name"],
                    attribute["namespace"],
                    attribute["value"]["value"]
                ])
            })
            .collect::<Vec<_>>()
    ]))
}

pub(super) fn verify_option(
    value: &Value,
    expected: &Value,
    original: &Document,
    sources: &Sources<'_>,
) -> io::Result<()> {
    for (field, selector, index) in [("Tags", "global", 0), ("FragTags", "fragment", 1)] {
        let proof = &value[selector]["attribute"];
        equal(
            &projected_attrs(proof)?,
            at(values(expected)?, index)?,
            "ADB tag selector occurrences",
        )?;
        verify_attributes(proof, field, original, sources)?;
    }
    Ok(())
}

pub(super) fn verify_tags(
    value: &Value,
    expected: &[Value],
    original: &Document,
    sources: &Sources<'_>,
) -> io::Result<()> {
    if value["source"] != original.source {
        return Err(io::Error::other("TagDef physical source differs"));
    }
    let mut tags = Vec::new();
    for section in values(&value["declaration"]["sections"])? {
        for member in values(&section["members"])? {
            match member["type"].as_str() {
                Some("tag") => tags.push(projected_tag(member, original, sources)?),
                Some("group") => {
                    for tag in values(&member["tags"])? {
                        tags.push(projected_tag(tag, original, sources)?);
                    }
                }
                _ => return Err(io::Error::other("unsupported typed TagDef member")),
            }
        }
    }
    if tags != expected {
        return Err(io::Error::other(
            "TagDef authored tag names or coordinates differ",
        ));
    }
    Ok(())
}

fn projected_tag(tag: &Value, original: &Document, sources: &Sources<'_>) -> io::Result<Value> {
    verify_position(&tag["position"], original, sources)?;
    verify_attributes(&tag["name"], "name", original, sources)?;
    Ok(json!([
        tag["position"]["value"],
        projected_attrs(&tag["name"])?
    ]))
}

fn verify_attributes(
    attribute: &Value,
    name: &str,
    original: &Document,
    sources: &Sources<'_>,
) -> io::Result<()> {
    for occurrence in values(&attribute["occurrences"])? {
        verify_coordinate(
            &occurrence["value"],
            &occurrence["position"],
            name,
            original,
            sources,
        )?;
    }
    Ok(())
}

fn verify_position(value: &Value, original: &Document, sources: &Sources<'_>) -> io::Result<()> {
    verify_coordinate(value, &value["value"], "$element", original, sources)
}

fn verify_coordinate(
    value: &Value,
    position: &Value,
    field: &str,
    original: &Document,
    sources: &Sources<'_>,
) -> io::Result<()> {
    let coordinate = match position["type"].as_str() {
        Some("text") => SocXmlPosition::Text {
            start: serde_json::from_value(position["start"].clone())?,
            end: serde_json::from_value(position["end"].clone())?,
        },
        Some("cryXml") => SocXmlPosition::CryXml {
            node: serde_json::from_value(position["node"].clone())?,
            attribute: serde_json::from_value(position["attribute"].clone())?,
        },
        _ => return Err(io::Error::other("missing original XML position")),
    };
    sources.field(
        value,
        &original.physical,
        &coordinate.location(None, field.into()),
    )
}

pub(super) fn values(value: &Value) -> io::Result<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| io::Error::other("original-order array unavailable"))
}

pub(super) fn at(rows: &[Value], index: u64) -> io::Result<&Value> {
    rows.get(usize::try_from(index).map_err(io::Error::other)?)
        .ok_or_else(|| io::Error::other("unexpected original occurrence index"))
}

pub(super) fn equal(actual: &Value, expected: &Value, what: &str) -> io::Result<()> {
    if actual != expected {
        return Err(io::Error::other(format!("{what} differs")));
    }
    Ok(())
}
fn projected_attrs(attribute: &Value) -> io::Result<Value> {
    let occurrences = attribute["occurrences"]
        .as_array()
        .ok_or_else(|| io::Error::other("missing attribute occurrences"))?;
    Ok(json!(
        occurrences
            .iter()
            .map(|row| json!([row["position"], row["value"]["value"]]))
            .collect::<Vec<_>>()
    ))
}
