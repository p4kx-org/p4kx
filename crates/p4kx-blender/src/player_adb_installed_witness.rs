// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;

use p4kx_assembly::BodyVariant;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, CancelToken, PhysicalSourceRef, SourceEncryption};
use serde_json::{Value, json};

use crate::soc_scene_origin::SocXmlPosition;
use crate::{PlayerDocument, PlayerDocumentSink, PlayerSourceCapture};

#[path = "player_adb_installed_witness_receipts.rs"]
mod receipts;

use self::receipts::{DocumentReceipts, Sources};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Witness(BTreeMap<usize, Original>);

struct Original {
    source: PhysicalSourceRef,
    encryption: SourceEncryption,
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
                    encryption: document.original.encryption(),
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
    let path = p4kx_testkit::real_archive_path()?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let mut witness = Witness::default();
    let captured = PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male, BodyVariant::Female],
        &CancelToken::new(),
        &mut witness,
    )
    .map_err(|_| "selected Player source capture failed")?;
    verify(&captured, witness)
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
        .map(|(ordinal, layer)| observe_layer(ordinal, layer))
        .collect()
}

fn observe_layer(layer_ordinal: usize, layer: SourceElement<'_>) -> io::Result<Value> {
    let mut blends = Vec::new();
    let mut animations = Vec::new();
    let mut preceding_blend = None;
    for (ordinal, node) in layer.elements().enumerate() {
        if node.namespace().is_some() {
            continue;
        }
        match node.name() {
            "Blend" => {
                preceding_blend = Some(ordinal);
                blends.push(json!([
                    ordinal,
                    position(node)?,
                    attributes(node, "Duration")?,
                    attributes(node, "StartTime")?,
                    attributes(node, "ExitTime")?
                ]));
            }
            "Animation" => animations.push(json!([
                ordinal,
                position(node)?,
                attributes(node, "name")?,
                attributes(node, "flags")?,
                preceding_blend
            ])),
            _ => {}
        }
    }
    Ok(json!([layer_ordinal, position(layer)?, animations, blends]))
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

fn verify(capture: &PlayerSourceCapture, mut witness: Witness) -> Check {
    if capture.adb_declarations.len() != witness.0.len() || witness.0.is_empty() {
        return Err("source/document coverage disagrees".into());
    }
    let sources = Sources::new(&capture.source_inventory, witness.0.values());
    let mut coverage = Coverage::default();
    for declaration in &capture.adb_declarations {
        let expected = witness
            .0
            .remove(&declaration.document)
            .ok_or("exact selected original document")?;
        if expected.source != declaration.source {
            return Err("source identity disagrees".into());
        }
        if declaration.fragments.len() != expected.fragments.len() {
            return Err("authored ADB option count differs".into());
        }
        let receipts = sources.document(&expected);
        for (fragment, original) in declaration.fragments.iter().zip(&expected.fragments) {
            let option = serde_json::to_value(fragment)?;
            if projected_option(&option, &receipts, &mut coverage)? != *original {
                return Err(
                    "authored ADB option or layer order, value or coordinates differ".into(),
                );
            }
        }
    }
    if !coverage.preceding_blend || !coverage.authored_flags {
        return Err("installed sample needs an ordered blend relation and authored flags".into());
    }
    Ok(())
}

#[derive(Default)]
struct Coverage {
    preceding_blend: bool,
    authored_flags: bool,
}

fn projected_option(
    option: &Value,
    receipts: &DocumentReceipts<'_>,
    coverage: &mut Coverage,
) -> Check<Value> {
    let layers = option["layers"]
        .as_array()
        .ok_or("ADB layers")?
        .iter()
        .map(|layer| projected_layer(layer, receipts, coverage))
        .collect::<Check<Vec<_>>>()?;
    Ok(json!([
        option["listOrdinal"],
        option["groupOrdinal"],
        option["optionOrdinal"],
        option["name"]["value"],
        option["groupPosition"]["value"],
        option["position"]["value"],
        receipts.attributes(option, "guid", "GUID")?,
        layers,
    ]))
}

fn projected_layer(
    layer: &Value,
    receipts: &DocumentReceipts<'_>,
    coverage: &mut Coverage,
) -> Check<Value> {
    let blends = layer["blends"].as_array().ok_or("ADB blend nodes")?;
    let projected_blends = blends
        .iter()
        .map(|blend| {
            receipts.position(blend)?;
            Ok(json!([
                blend["ordinal"],
                blend["position"]["value"],
                receipts.attributes(blend, "duration", "Duration")?,
                receipts.attributes(blend, "startTime", "StartTime")?,
                receipts.attributes(blend, "exitTime", "ExitTime")?
            ]))
        })
        .collect::<Check<Vec<_>>>()?;
    let animations = layer["animations"]
        .as_array()
        .ok_or("ADB animation nodes")?
        .iter()
        .map(|animation| {
            receipts.position(animation)?;
            let preceding = receipts.preceding_blend(layer, animation, blends)?;
            let flags = receipts.attributes(animation, "flags", "flags")?;
            coverage.preceding_blend |= preceding.is_some();
            coverage.authored_flags |= !flags.is_empty();
            Ok(json!([
                animation["ordinal"],
                animation["position"]["value"],
                receipts.attributes(animation, "name", "name")?,
                flags,
                preceding
            ]))
        })
        .collect::<Check<Vec<_>>>()?;
    Ok(json!([
        layer["ordinal"],
        layer["position"]["value"],
        animations,
        projected_blends
    ]))
}
