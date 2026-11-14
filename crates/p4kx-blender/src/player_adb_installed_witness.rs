// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use p4kx_assembly::BodyVariant;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, CancelToken, PhysicalSourceRef};
use serde_json::{Value, json};

use crate::soc_scene_origin::SocXmlPosition;
use crate::{PlayerDocument, PlayerDocumentSink, PlayerSourceCapture};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Witness(BTreeMap<usize, Original>);

struct Original {
    source: PhysicalSourceRef,
    fragments: Vec<Value>,
}

impl PlayerDocumentSink for Witness {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        let original = SourceDocument::parse(document.bytes, SourceReadOptions::default())
            .map_err(|_| io::Error::other("accepted XML did not reparse"))?;
        let root = original.root();
        if root.name() != "AnimDB" || root.namespace().is_some() {
            return Ok(());
        }
        let fragments = observe(root).map_err(io::Error::other)?;
        if self
            .0
            .insert(
                document.index,
                Original {
                    source: document.source,
                    fragments,
                },
            )
            .is_some()
        {
            return Err(io::Error::other("duplicate original ADB document"));
        }
        Ok(())
    }
}

#[test]
#[ignore = "selected Player original members in installed Archive; set P4KX_DATA_P4K"]
fn selected_player_adb_authored_declarations_match_installed_originals() -> Check {
    let path = std::env::var_os(p4kx_testkit::REAL_ARCHIVE_VAR)
        .map(PathBuf::from)
        .ok_or("input Archive environment required")?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let mut witness = Witness::default();
    let captured = PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male, BodyVariant::Female],
        &CancelToken::new(),
        &mut witness,
    )
    .map_err(|_| "selected Player source capture failed")?;
    verify(&serde_json::to_value(&captured.adb_declarations)?, &witness)?;
    Ok(())
}

fn observe(root: SourceElement<'_>) -> io::Result<Vec<Value>> {
    let mut inputs = Vec::new();
    for (list, listing) in root.elements().enumerate() {
        if listing.name() != "FragmentList" || listing.namespace().is_some() {
            continue;
        }
        for (group, parent) in listing.elements().enumerate() {
            if parent.namespace().is_some() {
                continue;
            }
            for (option, node) in parent.elements().enumerate() {
                if node.name() == "Fragment" && node.namespace().is_none() {
                    inputs.push(json!([
                        list,
                        group,
                        option,
                        parent.name(),
                        position(parent)?,
                        position(node)?,
                        attributes(node, "GUID")?,
                        layers(node)?,
                    ]));
                }
            }
        }
    }
    Ok(inputs)
}

fn layers(node: SourceElement<'_>) -> io::Result<Vec<Value>> {
    node.elements()
        .enumerate()
        .filter(|(_, layer)| layer.name() == "AnimLayer" && layer.namespace().is_none())
        .map(|(layer_ordinal, layer)| {
            let animations = layer
                .elements()
                .enumerate()
                .filter(|(_, row)| row.name() == "Animation" && row.namespace().is_none())
                .map(|(ordinal, animation)| {
                    Ok(json!([
                        ordinal,
                        position(animation)?,
                        attributes(animation, "name")?
                    ]))
                })
                .collect::<io::Result<Vec<_>>>()?;
            Ok(json!([layer_ordinal, position(layer)?, animations]))
        })
        .collect()
}

fn position(node: SourceElement<'_>) -> io::Result<Value> {
    let position = SocXmlPosition::element(node.position()).map_err(io::Error::other)?;
    Ok(serde_json::to_value(position)?)
}

fn attributes(node: SourceElement<'_>, name: &str) -> io::Result<Vec<Value>> {
    node.attributes()
        .filter(|a| a.namespace().is_none() && a.name() == name)
        .map(|a| {
            let position = SocXmlPosition::attribute(a.position()).map_err(io::Error::other)?;
            Ok(json!([position, a.value()]))
        })
        .collect()
}

fn verify(product: &Value, witness: &Witness) -> Check {
    let rows = product.as_array().ok_or("ADB row array")?;
    if rows.len() != witness.0.len() || rows.is_empty() {
        return Err("source/document coverage disagrees".into());
    }
    let mut layers_seen = false;
    for row in rows {
        let index = serde_json::from_value::<usize>(row["document"].clone())?;
        let expected = witness
            .0
            .get(&index)
            .ok_or("exact selected original document")?;
        if serde_json::to_value(&expected.source)? != row["source"] {
            return Err("source identity disagrees".into());
        }
        let mut actual = Vec::new();
        for option in row["fragments"].as_array().ok_or("ADB options")? {
            let layers = projected_layers(option)?;
            layers_seen |= !layers.is_empty();
            actual.push(json!([
                option["listOrdinal"],
                option["groupOrdinal"],
                option["optionOrdinal"],
                option["name"]["value"],
                option["groupPosition"]["value"],
                option["position"]["value"],
                projected_attrs(&option["guid"])?,
                layers,
            ]));
        }
        if actual != expected.fragments {
            return Err(
                "authored option, layer, animation or attribute order/coordinates differ".into(),
            );
        }
    }
    if !layers_seen {
        return Err("installed sample contains no directly authored animation layers".into());
    }
    Ok(())
}

fn projected_layers(option: &Value) -> Check<Vec<Value>> {
    option["layers"]
        .as_array()
        .ok_or_else(|| "ADB layers".into())
        .and_then(|layers| {
            layers
                .iter()
                .map(|layer| {
                    let animations = layer["animations"]
                        .as_array()
                        .ok_or("ADB animations")?
                        .iter()
                        .map(|animation| {
                            Ok(json!([
                                animation["ordinal"],
                                animation["position"]["value"],
                                projected_attrs(&animation["name"])?,
                            ]))
                        })
                        .collect::<Check<Vec<_>>>()?;
                    Ok(json!([
                        layer["ordinal"],
                        layer["position"]["value"],
                        animations
                    ]))
                })
                .collect()
        })
}

fn projected_attrs(selection: &Value) -> Check<Vec<Value>> {
    let rows = selection["occurrences"]
        .as_array()
        .ok_or("source occurrences")?;
    let state = match rows.len() {
        0 => "absent",
        1 => "present",
        _ => "invalid",
    };
    if selection["state"] != state || selection["sourceSetComplete"] != true {
        return Err("sourced selector state".into());
    }
    Ok(rows
        .iter()
        .map(|row| json!([row["position"], row["value"]["value"]]))
        .collect())
}
