// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use serde_json::{Value, json};

use crate::PlayerDocument;
use crate::player_bundle_wire::Row;
use crate::soc_scene_origin::SocXmlPosition;

use super::{Check, Physical, Sources};

#[path = "player_bundle_witness_projection.rs"]
mod projection;

use projection::{at, verify_branch, verify_controller, verify_option, verify_tags};

#[derive(Default)]
pub(super) struct Coverage {
    seen: BTreeSet<(u64, &'static str, u64)>,
}

pub(super) struct Document {
    physical: Physical,
    source: String,
    path: String,
    expected: Expected,
}

enum Expected {
    Controller(Value),
    Adb {
        options: Vec<Value>,
        branches: Vec<Value>,
        tag_definition: Option<String>,
    },
    Loadout(Vec<Option<String>>),
    Tags {
        names: Vec<Value>,
        version: Option<String>,
        imports: Vec<Option<String>>,
    },
}

impl Document {
    pub(super) fn observe(document: &PlayerDocument<'_>) -> io::Result<Option<Self>> {
        let xml = SourceDocument::parse(document.bytes, SourceReadOptions::default())
            .map_err(|_| io::Error::other("accepted original XML failed to reparse"))?;
        let root = xml.root();
        let expected = match root.name() {
            "ControllerDef" => Expected::Controller(controller(root)?),
            "Loadout" if root.namespace().is_none() => Expected::Loadout(loadout_items(root)),
            "AnimDB" => Expected::Adb {
                options: options(root)?,
                branches: branches(root)?,
                tag_definition: single_path(root, "TagDef"),
            },
            "TagDefinition" => Expected::Tags {
                names: tag_names(root)?,
                version: single_path(root, "version"),
                imports: children(root, "Imports")
                    .flat_map(|section| children(section, "Import"))
                    .map(|node| single_path(node, "filename"))
                    .collect(),
            },
            _ => return Ok(None),
        };
        Ok(Some(Self {
            physical: Physical::of(document.original),
            source: document.source.as_str().to_owned(),
            path: document.original.entry().path().to_owned(),
            expected,
        }))
    }
}

impl Document {
    pub(super) fn path(&self) -> &str {
        &self.path
    }

    pub(super) fn adb(&self) -> Option<(&[Value], Option<&str>)> {
        let Expected::Adb {
            options,
            tag_definition,
            ..
        } = &self.expected
        else {
            return None;
        };
        Some((options, tag_definition.as_deref()))
    }

    pub(super) fn loadout(&self) -> Option<&[Option<String>]> {
        let Expected::Loadout(items) = &self.expected else {
            return None;
        };
        Some(items)
    }

    pub(super) fn tag_definition(&self) -> Option<TagDefinition<'_>> {
        let Expected::Tags {
            names,
            version,
            imports,
        } = &self.expected
        else {
            return None;
        };
        Some(TagDefinition {
            names,
            version: version.as_deref(),
            imports,
        })
    }

    pub(super) fn attribute_field(
        &self,
        value: &Value,
        original: &Value,
        field: &str,
        sources: &Sources<'_>,
    ) -> io::Result<()> {
        if value["sourceUsesRefs"].as_array().map(Vec::len) != Some(1) {
            return Err(io::Error::other(
                "an ADB token needs only its authored attribute",
            ));
        }
        projection::verify_coordinate(value, &original[0], field, self, sources)
    }
}

pub(super) struct TagDefinition<'a> {
    pub(super) names: &'a [Value],
    pub(super) version: Option<&'a str>,
    pub(super) imports: &'a [Option<String>],
}

fn loadout_items(root: SourceElement<'_>) -> Vec<Option<String>> {
    let mut result = Vec::new();
    let mut pending = children(root, "Items").collect::<Vec<_>>();
    pending.reverse();
    while let Some(node) = pending.pop() {
        let is_group = node.name() == "Items";
        result.push((!is_group).then(|| single_path(node, "itemName")).flatten());
        let start = pending.len();
        pending.extend(children(node, if is_group { "Item" } else { "Items" }));
        pending[start..].reverse();
    }
    result
}

fn single_path(node: SourceElement<'_>, field: &str) -> Option<String> {
    let mut matches = node
        .attributes()
        .filter(|attribute| attribute.name() == field && attribute.namespace().is_none());
    let value = matches.next()?.value();
    matches.next().is_none().then(|| value.to_owned())
}

impl Coverage {
    pub(super) fn verify(
        &mut self,
        row: &Row<'_>,
        originals: &BTreeMap<u64, Document>,
        sources: &Sources<'_>,
    ) -> io::Result<()> {
        let kind = match row {
            Row::ControllerTopology { .. } => "controller",
            Row::AdbTagSelectors { .. } => "options",
            Row::AdbBranchSelectors { .. } => "branches",
            Row::TagDefinitionDocument { .. } => "definitions",
            _ => return Ok(()),
        };
        let wire = serde_json::to_value(row)?;
        let value = wire.get("value").unwrap_or(&wire);
        let index = value["document"]
            .as_u64()
            .ok_or_else(|| io::Error::other("missing document index"))?;
        let original = originals
            .get(&index)
            .ok_or_else(|| io::Error::other("missing physical XML original"))?;
        let local = value["index"].as_u64().unwrap_or_default();
        if !self.seen.insert((index, kind, local)) {
            return Err(io::Error::other("duplicate selected XML projection row"));
        }
        match (kind, &original.expected) {
            ("controller", Expected::Controller(expected)) => {
                verify_controller(value, expected, original, sources)
            }
            ("options", Expected::Adb { options, .. }) => {
                verify_option(value, at(options, local)?, original, sources)
            }
            ("branches", Expected::Adb { branches, .. }) => {
                verify_branch(value, at(branches, local)?, original, sources)
            }
            (
                "definitions",
                Expected::Tags {
                    names: expected, ..
                },
            ) => verify_tags(value, expected, original, sources),
            _ => Err(io::Error::other("selected original document role differs")),
        }
    }

    pub(super) fn finish(&self, originals: &BTreeMap<u64, Document>) -> Check {
        for (&document, original) in originals {
            match &original.expected {
                Expected::Controller(_) => self.require(document, "controller", 0)?,
                Expected::Adb {
                    options, branches, ..
                } => {
                    for (index, option) in options.iter().enumerate() {
                        if option[0].as_array().is_some_and(|attrs| !attrs.is_empty())
                            || option[1].as_array().is_some_and(|attrs| !attrs.is_empty())
                        {
                            self.require(document, "options", index as u64)?;
                        }
                    }
                    for (index, _) in branches.iter().enumerate() {
                        self.require(document, "branches", index as u64)?;
                    }
                }
                Expected::Tags { .. } | Expected::Loadout(_) => {}
            }
        }
        for required in ["controller", "options", "branches", "definitions"] {
            if !self.seen.iter().any(|(_, kind, _)| *kind == required) {
                return Err(format!("selected XML role {required} had no reconciled rows").into());
            }
        }
        Ok(())
    }

    fn require(&self, document: u64, kind: &'static str, index: u64) -> io::Result<()> {
        if !self.seen.contains(&(document, kind, index)) {
            return Err(io::Error::other(format!(
                "missing {kind} projection for admitted original"
            )));
        }
        Ok(())
    }
}

fn children<'a>(
    node: SourceElement<'a>,
    name: &'static str,
) -> impl Iterator<Item = SourceElement<'a>> {
    node.elements()
        .filter(move |node| node.namespace().is_none() && node.name() == name)
}

fn members<'a>(
    node: SourceElement<'a>,
    name: &'static str,
) -> impl Iterator<Item = SourceElement<'a>> {
    children(node, name)
        .flat_map(|section| section.elements().filter(|node| node.namespace().is_none()))
}

fn position(node: SourceElement<'_>) -> io::Result<Value> {
    Ok(serde_json::to_value(
        SocXmlPosition::element(node.position()).map_err(io::Error::other)?,
    )?)
}

fn original_attrs(node: SourceElement<'_>, field: &str) -> io::Result<Vec<Value>> {
    node.attributes()
        .filter(|attribute| attribute.namespace().is_none() && attribute.name() == field)
        .map(|attribute| {
            let position =
                SocXmlPosition::attribute(attribute.position()).map_err(io::Error::other)?;
            Ok(json!([position, attribute.value()]))
        })
        .collect()
}

fn options(root: SourceElement<'_>) -> io::Result<Vec<Value>> {
    members(root, "FragmentList")
        .flat_map(|group| children(group, "Fragment"))
        .map(|node| {
            Ok(json!([
                original_attrs(node, "Tags")?,
                original_attrs(node, "FragTags")?
            ]))
        })
        .collect()
}

fn branches(root: SourceElement<'_>) -> io::Result<Vec<Value>> {
    children(root, "SubADBs")
        .flat_map(|section| children(section, "SubADB"))
        .map(|branch| {
            let definitions = branch
                .elements()
                .enumerate()
                .filter(|(_, node)| node.namespace().is_none() && node.name() == "FragmentID")
                .map(|(index, node)| {
                    Ok(json!([
                        index,
                        position(node)?,
                        original_attrs(node, "Name")?
                    ]))
                })
                .collect::<io::Result<Vec<_>>>()?;
            Ok(json!([
                position(branch)?,
                original_attrs(branch, "Tags")?,
                definitions
            ]))
        })
        .collect()
}

fn tag_names(root: SourceElement<'_>) -> io::Result<Vec<Value>> {
    let mut tags = Vec::new();
    for node in members(root, "Tags") {
        match node.name() {
            "Tag" => tags.push(json!([position(node)?, original_attrs(node, "name")?])),
            "Group" => {
                for child in children(node, "Tag") {
                    tags.push(json!([position(child)?, original_attrs(child, "name")?]));
                }
            }
            _ => {}
        }
    }
    Ok(tags)
}

fn original_node(node: SourceElement<'_>) -> io::Result<Value> {
    let attributes = node
        .attributes()
        .map(|attribute| {
            let position =
                SocXmlPosition::attribute(attribute.position()).map_err(io::Error::other)?;
            Ok(json!([
                position,
                attribute.name(),
                attribute.namespace(),
                attribute.value()
            ]))
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(json!([position(node)?, node.name(), attributes]))
}

fn controller(root: SourceElement<'_>) -> io::Result<Value> {
    let contexts = members(root, "ScopeContextDefs")
        .map(original_node)
        .collect::<io::Result<Vec<_>>>()?;
    let scopes = members(root, "ScopeDefs")
        .map(original_node)
        .collect::<io::Result<Vec<_>>>()?;
    let fragments = members(root, "FragmentDefs")
        .map(|node| {
            let overrides = children(node, "Override")
                .map(original_node)
                .collect::<io::Result<Vec<_>>>()?;
            Ok(json!([original_node(node)?, overrides]))
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(json!([contexts, scopes, fragments]))
}
