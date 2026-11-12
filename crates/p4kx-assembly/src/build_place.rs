// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Bone, Model, Transform};

use crate::build::issue;
use crate::build_models::Loaded;
use crate::creature_rig;
use crate::flatten::Draft;
use crate::placement::{bone_transform, node_transform, offset};
use crate::{
    Issue, IssueKind, Part, Placement, PortDefinition, PortNode, PortPlacementSource, PortTransform,
};

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
) -> (Transform, Placement, Option<PortPlacementSource>) {
    let mut unreported = Vec::new();
    let issues = if draft.hidden {
        &mut unreported
    } else {
        issues
    };
    let Some(parent) = draft.parent.and_then(|parent| parts.get(parent)) else {
        return (Transform::IDENTITY, Placement::Root, None);
    };
    if let Some(gear) = draft.gear {
        let (local, placement) = place_gear(draft, gear.bone(), parent, issues);
        return (local, placement, None);
    }
    let Some(port) = draft.port else {
        return (Transform::IDENTITY, Placement::Origin, None);
    };
    let port_definition = port.definition();
    let definition = port_definition.and_then(PortDefinition::transform);
    let helper = definition
        .map(PortTransform::helper)
        .filter(|helper| !helper.is_empty());
    let on_rig = || {
        rig.zip(helper)
            .and_then(|(rig, helper)| creature_rig::skin_port_anchor(rig, parent, helper))
    };
    let (placement, base) = if port_definition.is_some_and(PortDefinition::is_skin) {
        match prop.then(on_rig).flatten() {
            Some(found) => found,
            None => {
                return (
                    Transform::IDENTITY,
                    Placement::Skin,
                    PortPlacementSource::skin(port),
                );
            }
        }
    } else {
        match anchor(parent.model(), helper, port) {
            (Placement::Helper(helper), base) => (Placement::Helper(helper), base),
            other => on_rig().unwrap_or(other),
        }
    };
    if let Some(helper) = helper
        && placement == Placement::Origin
        && parent.model().is_some()
    {
        let helper = helper.to_owned();
        issues.push(issue(draft, IssueKind::HelperNotFound { helper }));
    }
    let (local, offset_helper) = with_offset(base, definition, model);
    let source = PortPlacementSource::anchor(port, &placement, definition, offset_helper);
    (local, placement, source)
}

fn with_offset(
    base: Transform,
    definition: Option<&PortTransform>,
    model: Option<&Loaded>,
) -> (Transform, bool) {
    let Some(definition) = definition else {
        return (base, false);
    };
    let local = base.then(&offset(definition.position(), definition.rotation()));
    let inner = (!definition.offset_helper().is_empty())
        .then(|| {
            let child = model?;
            node_transform(&child.model, definition.offset_helper())?.inverse()
        })
        .flatten();
    inner.map_or((local, false), |inner| (local.then(&inner), true))
}

/// Returns the transform of a landing gear: the node `bone` of its Entity's model.
fn place_gear(
    draft: &Draft<'_>,
    bone: &str,
    parent: &Part,
    issues: &mut Vec<Issue>,
) -> (Transform, Placement) {
    let Some(parent_model) = parent.model() else {
        return (Transform::IDENTITY, Placement::Origin);
    };
    if let Some(transform) = node_transform(parent_model, bone) {
        return (transform, Placement::Helper(bone.to_owned()));
    }
    let helper = bone.to_owned();
    issues.push(issue(draft, IssueKind::HelperNotFound { helper }));
    (Transform::IDENTITY, Placement::Origin)
}

/// Finds the node of the parent's model a port is anchored to: its helper as a node or as a bone
/// of the skeleton, else a node named like the port.
fn anchor(parent: Option<&Model>, helper: Option<&str>, port: &PortNode) -> (Placement, Transform) {
    let Some(parent) = parent else {
        return (Placement::Origin, Transform::IDENTITY);
    };
    if let Some(helper) = helper
        && let Some(transform) =
            node_transform(parent, helper).or_else(|| bone_transform(parent, helper))
    {
        return (Placement::Helper(helper.to_owned()), transform);
    }
    node_transform(parent, port.name())
        .map_or((Placement::Origin, Transform::IDENTITY), |transform| {
            (Placement::PortNode, transform)
        })
}
