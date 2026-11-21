// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDocument, SourceElement};
use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Mannequin {
    declared_fragments: Vec<Fragment>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Fragment {
    position: SocXmlPosition,
    name: SourcedValue<String>,
    scopes: SourcedValue<String>,
    animations: Vec<Animation>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Animation {
    position: SocXmlPosition,
    name: SourcedValue<String>,
    physical_clip: SourcedValue<PhysicalClip>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum PhysicalClip {
    Unresolved,
}

pub(crate) fn database(
    document: &SourceDocument<'_>,
    origin: &XmlOrigin<'_>,
    names: &[(String, SourceUse)],
) -> Result<Option<Mannequin>, SocSceneError> {
    let root = document.root();
    if root.name() != "AnimDB" || root.namespace().is_some() {
        return Ok(None);
    }
    let mut matches = Vec::new();
    for section in children(root, "FragmentList") {
        for node in section.elements().filter(|node| node.namespace().is_none()) {
            let selected: Vec<_> = names
                .iter()
                .filter(|(name, _)| name == node.name())
                .map(|(_, use_)| use_.clone())
                .collect();
            if !selected.is_empty() {
                matches.push(Fragment::read(node, origin, &selected, true)?);
            }
        }
    }
    Ok(Some(Mannequin {
        declared_fragments: matches,
    }))
}

pub(crate) fn controller(
    document: &SourceDocument<'_>,
    origin: &XmlOrigin<'_>,
    names: &[(String, SourceUse)],
) -> Result<Option<Mannequin>, SocSceneError> {
    let root = document.root();
    if root.name() != "ControllerDef" || root.namespace().is_some() {
        return Ok(None);
    }
    let mut matches = Vec::new();
    let mut pending = Vec::new();
    push_children(&mut pending, root);
    while let Some(node) = pending.pop() {
        let selected: Vec<_> = names
            .iter()
            .filter(|(name, _)| name == node.name())
            .map(|(_, use_)| use_.clone())
            .collect();
        if !selected.is_empty() {
            matches.push(Fragment::read(node, origin, &selected, false)?);
        }
        push_children(&mut pending, node);
    }
    Ok(Some(Mannequin {
        declared_fragments: matches,
    }))
}

fn push_children<'document>(
    pending: &mut Vec<SourceElement<'document>>,
    node: SourceElement<'document>,
) {
    let start = pending.len();
    pending.extend(node.elements().filter(|node| node.namespace().is_none()));
    if let Some(children) = pending.get_mut(start..) {
        children.reverse();
    }
}

impl Fragment {
    fn read(
        node: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        selectors: &[SourceUse],
        includes_animations: bool,
    ) -> Result<Self, SocSceneError> {
        let mut uses = selectors.to_vec();
        uses.push(origin.node(node, SourceUseRole::Payload)?);
        let name = origin
            .sources
            .value(SourceValue::Present(node.name().to_owned()), &uses, true);
        let scopes = string_attribute(node, "scopes", origin)?;
        let animations = if includes_animations {
            children(node, "Fragment")
                .flat_map(|fragment| children(fragment, "AnimLayer"))
                .flat_map(|layer| children(layer, "Animation"))
                .map(|animation| Animation::read(animation, origin, &uses))
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };
        Ok(Self {
            position: position(node)?,
            name,
            scopes,
            animations,
        })
    }
}

impl Animation {
    fn read(
        node: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        selectors: &[SourceUse],
    ) -> Result<Self, SocSceneError> {
        let name = string_attribute(node, "name", origin)?;
        let mut uses = selectors.to_vec();
        uses.push(origin.node(node, SourceUseRole::Selector)?);
        for attribute in node
            .attributes()
            .filter(|attr| attr.namespace().is_none() && attr.name() == "name")
        {
            uses.push(origin.attribute(&attribute, SourceUseRole::Selector)?.1);
        }
        let result = match name.state() {
            p4kx_extraction::SourceValueState::Present => {
                SourceValue::Present(PhysicalClip::Unresolved)
            }
            p4kx_extraction::SourceValueState::Absent => SourceValue::Absent,
            _ => SourceValue::Invalid,
        };
        Ok(Self {
            position: position(node)?,
            name,
            physical_clip: origin.sources.value(result, &uses, false),
        })
    }
}

pub(crate) fn string_attribute(
    node: SourceElement<'_>,
    name: &str,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<String>, SocSceneError> {
    let attributes = || {
        node.attributes()
            .filter(|attr| attr.namespace().is_none() && attr.name() == name)
    };
    let mut matched = attributes();
    let value = match (matched.next(), matched.next()) {
        (None, _) => SourceValue::Absent,
        (Some(value), None) => SourceValue::Present(value.value().to_owned()),
        _ => SourceValue::Invalid,
    };
    origin.field(node, attributes(), value)
}

pub(crate) fn children<'document>(
    parent: SourceElement<'document>,
    name: &'static str,
) -> impl Iterator<Item = SourceElement<'document>> {
    parent
        .elements()
        .filter(move |node| node.namespace().is_none() && node.name() == name)
}
