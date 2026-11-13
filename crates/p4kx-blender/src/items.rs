// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::package_light_placements::PlacedLight;
use p4kx_assembly::{Assembly, INTERIOR_NODE, Interior, InteriorPart, InteriorPartKind, Part};
use p4kx_extraction::{EntryOrigin, ModelArchiveSource};
use p4kx_gltf::{NodeTransform, SceneLight, light_node_transform, node_transform};
use p4kx_model::{MaterialStatus, Model, TintPalette, Transform};

use crate::manifest::{EntityRef, NodeKind};

/// What a node draws: a model with its material library and tint palette.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Drawn<'a> {
    pub(crate) model: &'a Model,
    pub(crate) model_sources: &'a [ModelArchiveSource],
    pub(crate) materials: &'a MaterialStatus,
    pub(crate) material_entry: Option<&'a EntryOrigin>,
    pub(crate) palette: Option<&'a TintPalette>,
    pub(crate) label: &'a str,
}

/// One node of the scene, before the meshes are numbered: a part of the Assembly, or a part of
/// its Interior.
#[derive(Debug)]
pub(crate) struct Item<'a> {
    pub(crate) kind: NodeKind,
    pub(crate) name: &'a str,
    pub(crate) path: String,
    pub(crate) parent: Option<usize>,
    pub(crate) transform: NodeTransform,
    pub(crate) entity: Option<EntityRef>,
    pub(crate) geometry: Option<&'a str>,
    pub(crate) drawn: Option<Drawn<'a>>,
    pub(crate) light: Option<SceneLight>,
    pub(crate) light_placement: Option<PlacedLight<'a>>,
}

/// Lists the nodes of `assembly`, parts first, then the bone attachments of the parts, then its
/// Interior if it has one: a group `Interior` below each Entity that names containers, below it
/// the containers, below those their `.soc` files, models, props and lights. A node comes after
/// its parent.
pub(crate) fn items(assembly: &Assembly) -> Vec<Item<'_>> {
    let mut items: Vec<Item<'_>> = assembly.parts().iter().map(part_item).collect();
    push_attachments(&mut items, assembly);
    let Some(interior) = assembly.interior() else {
        return items;
    };
    let mut group: Option<(usize, usize)> = None;
    for container in interior.containers() {
        let node = match group {
            Some((part, node)) if part == container.part() => node,
            _ => {
                let owner = assembly.parts().get(container.part());
                let path = join(owner.map_or("", Part::path), INTERIOR_NODE);
                let node = items.len();
                items.push(group_item(
                    INTERIOR_NODE,
                    owner.map(|_| container.part()),
                    path,
                ));
                group = Some((container.part(), node));
                node
            }
        };
        push_parts(
            &mut items,
            container.interior(),
            Some(node),
            container.placement(),
            Some((container.part(), container.reference())),
        );
    }
    items
}

/// Returns the Entity reference of `root`, empty when there is no root part.
pub(crate) fn entity_ref(root: Option<&Part>) -> EntityRef {
    root.map_or_else(
        || EntityRef {
            class_name: String::new(),
            guid: String::new(),
            classification: None,
        },
        |part| EntityRef {
            class_name: part.class_name().to_owned(),
            guid: part.guid().to_string(),
            classification: None,
        },
    )
}

/// Lists the nodes of an Interior on its own: the container is the root, below it its `.soc`
/// files, models, props, lights and nested containers. A node comes after its parent.
pub(crate) fn interior_items(interior: &Interior) -> Vec<Item<'_>> {
    let mut items = Vec::with_capacity(interior.parts().len());
    push_parts(&mut items, interior, None, &Transform::IDENTITY, None);
    items
}

fn push_parts<'a>(
    items: &mut Vec<Item<'a>>,
    interior: &'a Interior,
    group: Option<usize>,
    placement: &Transform,
    owner: Option<(usize, &'a p4kx_assembly::ObjectContainerRef)>,
) {
    let base = items.len();
    for part in interior.parts() {
        let parent = part.parent().map(|index| base + index).or(group);
        let path = join(
            parent
                .and_then(|index| items.get(index))
                .map_or("", |item| &item.path),
            part.name(),
        );
        let local = if part.parent().is_none() {
            placement
        } else {
            part.local()
        };
        let mut item = interior_item(part, parent, path, local);
        item.light_placement = owner.and_then(|(owner, reference)| {
            Some(PlacedLight {
                owner,
                container: base,
                reference,
                part,
                light: part.light()?,
                placement: part.light_placement()?,
            })
        });
        items.push(item);
    }
}

/// Lists the bone attachments of the parts, each below its part.
fn push_attachments<'a>(items: &mut Vec<Item<'a>>, assembly: &'a Assembly) {
    for (index, attached) in assembly.attached_models() {
        let part = assembly.parts().get(index);
        items.push(Item {
            kind: NodeKind::Object,
            name: attached.name(),
            path: join(part.map_or("", Part::path), attached.name()),
            parent: Some(index),
            transform: node_transform(attached.local()),
            entity: None,
            geometry: Some(attached.geometry()),
            drawn: Some(Drawn {
                model: attached.model(),
                model_sources: attached.model_archive_sources(),
                materials: attached.materials(),
                material_entry: attached.material_archive_origins().library(),
                palette: part.and_then(Part::palette),
                label: attached.name(),
            }),
            light: None,
            light_placement: None,
        });
    }
}

fn part_item(part: &Part) -> Item<'_> {
    let gear = part.is_landing_gear();
    Item {
        kind: if gear {
            NodeKind::Object
        } else {
            NodeKind::Entity
        },
        name: part.name(),
        path: part.path().to_owned(),
        parent: part.parent(),
        transform: node_transform(part.local()),
        entity: (!gear).then(|| EntityRef {
            class_name: part.class_name().to_owned(),
            guid: part.guid().to_string(),
            classification: None,
        }),
        geometry: part.geometry(),
        drawn: part.model().map(|model| Drawn {
            model,
            model_sources: part.model_archive_sources(),
            materials: part.materials(),
            material_entry: part.material_archive_origins().library(),
            palette: part.palette(),
            label: part.class_name(),
        }),
        light: None,
        light_placement: None,
    }
}

fn interior_item<'a>(
    part: &'a InteriorPart,
    parent: Option<usize>,
    path: String,
    local: &Transform,
) -> Item<'a> {
    let light = part.scene_light();
    let kind = match part.kind() {
        InteriorPartKind::Model | InteriorPartKind::Prop => NodeKind::Object,
        InteriorPartKind::Light if light.is_some() => NodeKind::Light,
        _ => NodeKind::Container,
    };
    Item {
        kind,
        name: part.name(),
        path,
        parent,
        transform: if light.is_some() {
            light_node_transform(local)
        } else {
            node_transform(local)
        },
        entity: None,
        geometry: part.model_path(),
        drawn: part.model().map(|model| Drawn {
            model,
            model_sources: part.model_archive_sources(),
            materials: part.materials(),
            material_entry: part.material_archive_origins().library(),
            palette: part.palette(),
            label: part.name(),
        }),
        light,
        light_placement: None,
    }
}

fn group_item(name: &str, parent: Option<usize>, path: String) -> Item<'_> {
    Item {
        kind: NodeKind::Container,
        name,
        path,
        parent,
        transform: node_transform(&Transform::IDENTITY),
        entity: None,
        geometry: None,
        drawn: None,
        light: None,
        light_placement: None,
    }
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}
