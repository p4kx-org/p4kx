// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, PortDefinition, PortNode, PortSource, PortTransform,
    Target,
};
use serde::Serialize;

use crate::entity_ignored::IncompatibleJson;
use crate::entity_outfit::OutfitsJson;
use crate::entity_vehicle::{LandingGearJson, VehicleJson, visibility_code};

/// The counts of a Loadout tree's ports by what they hold.
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub(crate) struct Counts {
    pub(crate) ports: usize,
    pub(crate) attached: usize,
    pub(crate) empty: usize,
    pub(crate) unresolved: usize,
    pub(crate) docked_vehicles: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub(crate) excluded: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub(crate) replaced: usize,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes a reference"
)]
const fn is_zero(count: &usize) -> bool {
    *count == 0
}

impl Counts {
    pub(crate) fn of(node: &EntityNode) -> Self {
        let mut counts = Self::default();
        for (_, port) in node.walk() {
            counts.ports = counts.ports.saturating_add(1);
            if let Attachment::Entity(child) = port.attachment()
                && child.is_docked_vehicle()
            {
                counts.docked_vehicles = counts.docked_vehicles.saturating_add(1);
            }
            match port.source() {
                PortSource::Excluded => counts.excluded = counts.excluded.saturating_add(1),
                PortSource::Replaced => counts.replaced = counts.replaced.saturating_add(1),
                _ => {}
            }
            let count = match port.attachment() {
                Attachment::Empty => &mut counts.empty,
                Attachment::Entity(_) => &mut counts.attached,
                Attachment::Unresolved(_) => &mut counts.unresolved,
            };
            *count = count.saturating_add(1);
        }
        counts
    }
}

/// The JSON shape of `p4kx entity loadout`.
#[derive(Serialize)]
pub(crate) struct LoadoutJson<'a> {
    pub(crate) archive: String,
    pub(crate) entry: &'a str,
    pub(crate) entity: NodeJson<'a>,
    #[serde(rename = "outfit", skip_serializing_if = "Option::is_none")]
    pub(crate) outfits: Option<OutfitsJson>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    pub(crate) ignored: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) incompatible: Vec<IncompatibleJson<'a>>,
    #[serde(flatten)]
    pub(crate) counts: Counts,
}

/// The JSON shape of an Entity in the tree.
#[derive(Serialize)]
pub(crate) struct NodeJson<'a> {
    guid: String,
    record_path: &'a str,
    name: &'a str,
    class_name: &'a str,
    docked_vehicle: bool,
    expanded: bool,
    issues: Vec<ErrorJson<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<ErrorJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vehicle: Option<VehicleJson<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    landing_gear: Vec<LandingGearJson<'a>>,
    ports: Vec<PortJson<'a>>,
}

impl<'a> From<&'a EntityNode> for NodeJson<'a> {
    fn from(node: &'a EntityNode) -> Self {
        Self {
            guid: node.guid().to_string(),
            record_path: node.record_path(),
            name: node.name(),
            class_name: node.class_name(),
            docked_vehicle: node.is_docked_vehicle(),
            expanded: node.is_expanded(),
            issues: node
                .issues()
                .iter()
                .map(|error| ErrorJson::new(error, None))
                .collect(),
            notes: node
                .notes()
                .iter()
                .map(|error| ErrorJson::new(error, None))
                .collect(),
            vehicle: node.vehicle().map(VehicleJson::from),
            landing_gear: node
                .landing_gear()
                .iter()
                .map(LandingGearJson::from)
                .collect(),
            ports: node.ports().iter().map(PortJson::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct PortJson<'a> {
    name: &'a str,
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<&'static str>,
    definition: Option<DefinitionJson<'a>>,
    entity: Option<NodeJson<'a>>,
    error: Option<ErrorJson<'a>>,
    nested: Vec<Self>,
}

impl<'a> From<&'a PortNode> for PortJson<'a> {
    fn from(port: &'a PortNode) -> Self {
        let (entity, error) = match port.attachment() {
            Attachment::Empty => (None, None),
            Attachment::Entity(node) => (Some(NodeJson::from(node.as_ref())), None),
            Attachment::Unresolved(unresolved) => (
                None,
                Some(ErrorJson::new(
                    unresolved.error(),
                    Some(unresolved.target()),
                )),
            ),
        };
        Self {
            name: port.name(),
            source: port.source().code(),
            visibility: visibility_code(port.visibility()),
            definition: port.definition().map(DefinitionJson::from),
            entity,
            error,
            nested: port.nested().iter().map(PortJson::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct ErrorJson<'a> {
    kind: &'static str,
    reason: String,
    reference: Option<String>,
    class_name: Option<&'a str>,
}

impl<'a> ErrorJson<'a> {
    fn new(error: &LoadoutError, target: Option<&'a Target>) -> Self {
        let (reference, class_name) = match target {
            Some(Target::Reference(guid)) => (Some(guid.to_string()), None),
            Some(Target::ClassName(name)) => (None, Some(name.as_str())),
            _ => (None, None),
        };
        Self {
            kind: error.kind(),
            reason: error.to_string(),
            reference,
            class_name,
        }
    }
}

#[derive(Serialize)]
struct DefinitionJson<'a> {
    display_name: &'a str,
    flags: &'a str,
    min_size: Option<u32>,
    max_size: Option<u32>,
    types: Vec<TypeJson<'a>>,
    transform: Option<TransformJson<'a>>,
}

impl<'a> From<&'a PortDefinition> for DefinitionJson<'a> {
    fn from(definition: &'a PortDefinition) -> Self {
        Self {
            display_name: definition.display_name(),
            flags: definition.flags(),
            min_size: definition.min_size(),
            max_size: definition.max_size(),
            types: definition
                .types()
                .iter()
                .map(|item_type| TypeJson {
                    name: item_type.name(),
                    sub_types: item_type.sub_types(),
                })
                .collect(),
            transform: definition.transform().map(TransformJson::from),
        }
    }
}

#[derive(Serialize)]
struct TypeJson<'a> {
    #[serde(rename = "type")]
    name: &'a str,
    sub_types: &'a [String],
}

#[derive(Serialize)]
struct TransformJson<'a> {
    attachment: &'a str,
    helper: &'a str,
    offset_helper: &'a str,
    position: [f32; 3],
    rotation: [f32; 3],
}

impl<'a> From<&'a PortTransform> for TransformJson<'a> {
    fn from(transform: &'a PortTransform) -> Self {
        Self {
            attachment: transform.attachment(),
            helper: transform.helper(),
            offset_helper: transform.offset_helper(),
            position: transform.position(),
            rotation: transform.rotation(),
        }
    }
}
