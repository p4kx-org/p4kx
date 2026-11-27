// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{
    InputScope, PhysicalSourceRef, SourceEncryption, SourceEncryptionState, SourceInventory,
    SourceLocation, SourceUseDescription, SourceUseRole,
};
use serde_json::Value;

use crate::soc_scene_origin::SocXmlPosition;
use crate::{PlayerDocument, PlayerDocumentSink};

#[derive(Default)]
pub(super) struct Originals(BTreeMap<usize, Original>);

struct Original {
    source: PhysicalSourceRef,
    entry: usize,
    fragments: Vec<Fragment>,
}

pub(super) struct Fragment {
    pub(super) coordinate: [usize; 3],
    pub(super) animations: Vec<Animation>,
}

pub(super) struct Animation {
    pub(super) document: usize,
    pub(super) fragment: usize,
    pub(super) layer: usize,
    pub(super) ordinal: usize,
    pub(super) layer_position: SourceLocation,
    pub(super) position: SourceLocation,
    pub(super) names: Vec<Name>,
    source: PhysicalSourceRef,
    encryption: SourceEncryption,
}

pub(super) struct Name {
    pub(super) spelling: String,
    pub(super) position: SourceLocation,
}

struct Input<'a> {
    document: usize,
    source: &'a PhysicalSourceRef,
    encryption: SourceEncryption,
}

impl PlayerDocumentSink for Originals {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        let parsed = SourceDocument::parse(document.bytes, SourceReadOptions::default())
            .map_err(|_| io::Error::other("cannot reparse accepted Player XML"))?;
        let root = parsed.root();
        if root.name() != "AnimDB" || root.namespace().is_some() {
            return Ok(());
        }
        let input = Input {
            document: document.index,
            source: &document.source,
            encryption: document.original.encryption(),
        };
        let fragments = input.fragments(root)?;
        let original = Original {
            source: document.source,
            entry: document.original.entry().index(),
            fragments,
        };
        if self.0.insert(document.index, original).is_some() {
            return Err(io::Error::other("duplicate accepted ADB document"));
        }
        Ok(())
    }
}

impl Originals {
    pub(super) fn fragment(&self, document: usize, index: usize) -> io::Result<&Fragment> {
        self.0
            .get(&document)
            .and_then(|original| original.fragments.get(index))
            .ok_or_else(|| io::Error::other("unobserved original ADB fragment"))
    }

    pub(super) fn fragment_count(&self) -> usize {
        self.0
            .values()
            .map(|original| original.fragments.len())
            .sum()
    }

    pub(super) fn animations(&self) -> impl Iterator<Item = &Animation> {
        self.0
            .values()
            .flat_map(|original| &original.fragments)
            .flat_map(|fragment| &fragment.animations)
    }
}

impl Input<'_> {
    fn fragments(&self, root: SourceElement<'_>) -> io::Result<Vec<Fragment>> {
        let mut fragments = Vec::new();
        for (list, list_node) in children(root, "FragmentList") {
            for (group, group_node) in list_node.elements().enumerate() {
                if group_node.namespace().is_some() {
                    continue;
                }
                for (option, option_node) in children(group_node, "Fragment") {
                    let animations = self.animations(fragments.len(), option_node)?;
                    fragments.push(Fragment {
                        coordinate: [list, group, option],
                        animations,
                    });
                }
            }
        }
        Ok(fragments)
    }

    fn animations(&self, fragment: usize, option: SourceElement<'_>) -> io::Result<Vec<Animation>> {
        let mut original = Vec::new();
        for (layer, layer_node) in children(option, "AnimLayer") {
            let layer_position = node_site(layer_node)?;
            for (animation, animation_node) in children(layer_node, "Animation") {
                let names = animation_node
                    .attributes()
                    .filter(|attribute| {
                        attribute.namespace().is_none() && attribute.name() == "name"
                    })
                    .map(|attribute| {
                        let position = SocXmlPosition::attribute(attribute.position())
                            .map_err(io::Error::other)?;
                        Ok(Name {
                            spelling: attribute.value().to_owned(),
                            position: position.location(None, "name".to_owned()),
                        })
                    })
                    .collect::<io::Result<_>>()?;
                original.push(Animation {
                    document: self.document,
                    fragment,
                    layer,
                    ordinal: animation,
                    layer_position: layer_position.clone(),
                    position: node_site(animation_node)?,
                    names,
                    source: self.source.clone(),
                    encryption: self.encryption,
                });
            }
        }
        Ok(original)
    }
}

fn children<'doc, 'name>(
    node: SourceElement<'doc>,
    name: &'name str,
) -> impl Iterator<Item = (usize, SourceElement<'doc>)> + use<'doc, 'name> {
    node.elements()
        .enumerate()
        .filter(move |(_, child)| child.namespace().is_none() && child.name() == name)
}

fn node_site(node: SourceElement<'_>) -> io::Result<SourceLocation> {
    let position = SocXmlPosition::element(node.position()).map_err(io::Error::other)?;
    Ok(position.location(None, "$element".to_owned()))
}

pub(super) struct Receipts<'a>(Vec<&'a SourceUseDescription>);

impl<'a> Receipts<'a> {
    pub(super) fn new(inventory: &'a SourceInventory, originals: &Originals) -> io::Result<Self> {
        for original in originals.0.values() {
            let source = inventory
                .iter_sources()
                .find(|source| source.id() == &original.source)
                .ok_or_else(|| io::Error::other("missing physical source for accepted ADB"))?;
            let wire = serde_json::to_value(source)?;
            if number(&wire, "entryIndex")? != original.entry {
                return Err(io::Error::other(
                    "ADB source receipt identifies a different physical Entry",
                ));
            }
        }
        Ok(Self(inventory.iter_uses().collect()))
    }

    pub(super) fn name(&self, field: &Value, original: &Animation, name: &Name) -> io::Result<()> {
        self.xml(field, original, &name.position, SourceUseRole::Selector)
    }

    pub(super) fn xml(
        &self,
        field: &Value,
        original: &Animation,
        location: &SourceLocation,
        role: SourceUseRole,
    ) -> io::Result<()> {
        let expected = match original.encryption {
            SourceEncryption::Encrypted => SourceEncryptionState::Encrypted,
            SourceEncryption::Unencrypted => SourceEncryptionState::Unencrypted,
            _ => SourceEncryptionState::Unknown,
        };
        let mut exact = false;
        let mut encrypted = false;
        let complete = field
            .get("sourceSetComplete")
            .and_then(Value::as_bool)
            .ok_or_else(|| io::Error::other("missing explicit source completeness"))?;
        let mut unknown = !complete;
        for reference in array(field, "sourceUsesRefs")? {
            let used = self.lookup(string(reference)?)?;
            exact |= used.source() == &original.source
                && used.scope() == InputScope::Payload
                && used.role() == role
                && used.location() == location
                && used.effective_encryption() == expected;
            encrypted |= used.effective_encryption() == SourceEncryptionState::Encrypted;
            unknown |= used.effective_encryption() == SourceEncryptionState::Unknown;
        }
        let marker = if encrypted {
            Some(true)
        } else if unknown {
            None
        } else {
            Some(false)
        };
        let marker_matches = field.get("containsEncryptedSource").is_some_and(|stored| {
            marker.map_or_else(
                || stored.is_null(),
                |marker| stored.as_bool() == Some(marker),
            )
        });
        if !exact || !marker_matches {
            return Err(io::Error::other(
                "field original XML coordinate or encryption differs",
            ));
        }
        Ok(())
    }

    pub(super) fn caf_path(&self, ids: &[String]) -> io::Result<()> {
        for id in ids {
            let site = self.lookup(id)?;
            if let SourceLocation::Subresource {
                namespace,
                indices,
                field,
            } = site.location()
                && namespace == "animation-byte-range"
                && field == "cafImagePath"
                && matches!(indices.as_slice(), [start, end] if start < end)
                && site.role() == SourceUseRole::Payload
            {
                return Ok(());
            }
        }
        Err(io::Error::other("missing original physical CAF path bytes"))
    }

    fn lookup(&self, id: &str) -> io::Result<&SourceUseDescription> {
        let ordinal = self
            .0
            .binary_search_by(|used| used.id().as_str().cmp(id))
            .map_err(|_| io::Error::other("field use does not resolve in source inventory"))?;
        self.0
            .get(ordinal)
            .copied()
            .ok_or_else(|| io::Error::other("missing original field receipt"))
    }
}

pub(super) fn includes<'id>(
    field: &Value,
    expected: impl IntoIterator<Item = &'id str>,
) -> io::Result<()> {
    let uses = array(field, "sourceUsesRefs")?;
    if expected
        .into_iter()
        .all(|id| uses.iter().any(|reference| reference.as_str() == Some(id)))
    {
        Ok(())
    } else {
        Err(io::Error::other(
            "alias candidate omits original CAF or rule evidence",
        ))
    }
}

pub(super) fn references(field: &Value) -> io::Result<Vec<String>> {
    array(field, "sourceUsesRefs")?
        .iter()
        .map(|value| string(value).map(str::to_owned))
        .collect()
}

pub(super) fn array<'a>(value: &'a Value, member: &str) -> io::Result<&'a [Value]> {
    value[member]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| io::Error::other("expected original array"))
}

pub(super) fn member<'a>(value: &'a Value, name: &str) -> io::Result<&'a str> {
    string(&value[name])
}

fn string(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .ok_or_else(|| io::Error::other("expected sourced text"))
}

pub(super) fn number(value: &Value, member: &str) -> io::Result<usize> {
    value[member]
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| io::Error::other("expected original coordinate"))
}
