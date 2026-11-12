// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Bone, Model, Transform};

use crate::build::issue;
use crate::build_models::Loaded;
use crate::creature_rig;
use crate::flatten::Draft;
use crate::model_placement_sources::{ModelAnchor, ModelPlacement};
use crate::placement::{self, BonePlacement, NodePlacement, offset};
use crate::{
    Issue, IssueKind, Part, Placement, PortDefinition, PortNode, PortPlacementSource, PortTransform,
};

pub(crate) struct PlacedPart {
    pub(crate) local: Transform,
    pub(crate) kind: Placement,
    pub(crate) port: Option<PortPlacementSource>,
    pub(crate) nodes: ModelPlacement,
}

impl PlacedPart {
    fn unanchored(kind: Placement, port: Option<PortPlacementSource>) -> Self {
        Self {
            local: Transform::IDENTITY,
            kind,
            port,
            nodes: ModelPlacement::default(),
        }
    }
}

struct Anchor {
    kind: Placement,
    transform: Transform,
    source: ModelAnchor,
}

impl Anchor {
    const fn origin() -> Self {
        Self {
            kind: Placement::Origin,
            transform: Transform::IDENTITY,
            source: ModelAnchor::None,
        }
    }

    const fn node(kind: Placement, placement: NodePlacement) -> Self {
        Self {
            kind,
            transform: placement.transform,
            source: ModelAnchor::Node(placement.node),
        }
    }

    const fn bone(kind: Placement, placement: BonePlacement) -> Self {
        Self {
            kind,
            transform: placement.transform,
            source: ModelAnchor::Bone(placement.selection),
        }
    }

    fn with_port_offset(
        self,
        port: &PortNode,
        definition: Option<&PortTransform>,
        model: Option<&Loaded>,
    ) -> PlacedPart {
        let (local, offset) = with_offset(self.transform, definition, model);
        let source = PortPlacementSource::anchor(port, &self.kind, definition, offset.is_some());
        PlacedPart {
            local,
            kind: self.kind,
            port: source,
            nodes: ModelPlacement::new(self.source, offset),
        }
    }
}

/// Returns whether the parent of the part is a skin part.
pub(crate) fn under_skin(draft: &Draft<'_>, parts: &[Part]) -> bool {
    draft
        .parent
        .and_then(|parent| parts.get(parent))
        .is_some_and(|parent| parent.placement == Placement::Skin)
}

/// Returns the transform of a part to its parent's and how it was found. `rig` is the skeleton of
/// a character root that the item ports of its skins anchor to, and `prop` whether the part is a
/// prop: an Entity that is no character item and has a model without a skinned mesh, which has no
/// weights to follow the skeleton.
///
/// A part on a skin port is drawn in the bind pose of its parent, unless it is a prop. A prop
/// hangs on the bone its port names, and sits in the bind pose only where no bone is found.
pub(crate) fn place(
    draft: &Draft<'_>,
    parts: &[Part],
    model: Option<&Loaded>,
    rig: Option<&[Bone]>,
    prop: bool,
    issues: &mut Vec<Issue>,
) -> PlacedPart {
    let mut unreported = Vec::new();
    let issues = if draft.hidden {
        &mut unreported
    } else {
        issues
    };
    let Some(parent) = draft.parent.and_then(|parent| parts.get(parent)) else {
        return PlacedPart::unanchored(Placement::Root, None);
    };
    if let Some(gear) = draft.gear {
        let gear = place_gear(draft, gear.bone(), parent, issues);
        return PlacedPart {
            local: gear.transform,
            kind: gear.kind,
            port: None,
            nodes: ModelPlacement::new(gear.source, None),
        };
    }
    let Some(port) = draft.port else {
        return PlacedPart::unanchored(Placement::Origin, None);
    };
    let port_definition = port.definition();
    let definition = port_definition.and_then(PortDefinition::transform);
    let helper = definition
        .map(PortTransform::helper)
        .filter(|helper| !helper.is_empty());
    let on_rig = || {
        rig.zip(helper)
            .and_then(|(rig, helper)| creature_rig::skin_port_anchor(rig, parent, helper))
            .map(|(kind, transform)| Anchor {
                kind,
                transform,
                source: ModelAnchor::None,
            })
    };
    let anchored = if port_definition.is_some_and(PortDefinition::is_skin) {
        match prop.then(on_rig).flatten() {
            Some(found) => found,
            None => {
                return PlacedPart::unanchored(Placement::Skin, PortPlacementSource::skin(port));
            }
        }
    } else {
        let model_anchor = anchor(parent.model(), helper, port);
        if matches!(model_anchor.kind, Placement::Helper(_)) {
            model_anchor
        } else {
            on_rig().unwrap_or(model_anchor)
        }
    };
    if let Some(helper) = helper
        && anchored.kind == Placement::Origin
        && parent.model().is_some()
    {
        let helper = helper.to_owned();
        issues.push(issue(draft, IssueKind::HelperNotFound { helper }));
    }
    anchored.with_port_offset(port, definition, model)
}

fn with_offset(
    base: Transform,
    definition: Option<&PortTransform>,
    model: Option<&Loaded>,
) -> (Transform, Option<usize>) {
    let Some(definition) = definition else {
        return (base, None);
    };
    let local = base.then(&offset(definition.position(), definition.rotation()));
    let inner = (!definition.offset_helper().is_empty())
        .then(|| {
            let child = model?;
            let found = placement::node_placement(&child.model, definition.offset_helper())?;
            Some((found.transform.inverse()?, found.node))
        })
        .flatten();
    inner.map_or((local, None), |(inner, node)| {
        (local.then(&inner), Some(node))
    })
}

/// Returns the transform of a landing gear: the node `bone` of its Entity's model.
fn place_gear(draft: &Draft<'_>, bone: &str, parent: &Part, issues: &mut Vec<Issue>) -> Anchor {
    let Some(parent_model) = parent.model() else {
        return Anchor::origin();
    };
    if let Some(found) = placement::node_placement(parent_model, bone) {
        return Anchor::node(Placement::Helper(bone.to_owned()), found);
    }
    let helper = bone.to_owned();
    issues.push(issue(draft, IssueKind::HelperNotFound { helper }));
    Anchor::origin()
}

/// Finds the node of the parent's model a port is anchored to: its helper as a node or as a bone
/// of the skeleton, else a node named like the port.
fn anchor(parent: Option<&Model>, helper: Option<&str>, port: &PortNode) -> Anchor {
    let Some(parent) = parent else {
        return Anchor::origin();
    };
    if let Some(helper) = helper {
        if let Some(found) = placement::node_placement(parent, helper) {
            return Anchor::node(Placement::Helper(helper.to_owned()), found);
        }
        if let Some(found) = placement::bone_placement(parent, helper) {
            return Anchor::bone(Placement::Helper(helper.to_owned()), found);
        }
    }
    placement::node_placement(parent, port.name()).map_or_else(Anchor::origin, |found| {
        Anchor::node(Placement::PortNode, found)
    })
}
