// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_foundation::read_text_number;

use super::{PartKind, VehicleItemPort, VehicleItemType, VehiclePart};
use crate::markup::Element;
use crate::merge::port_key;

/// Reads the parts below the `Parts` element of a `Vehicle` element; parts marked `skipPart`
/// are left out with everything below them.
pub(super) fn parts(vehicle: &Element) -> Vec<VehiclePart> {
    vehicle
        .children_named("Parts")
        .flat_map(part_list)
        .collect()
}

fn part_list(container: &Element) -> Vec<VehiclePart> {
    container
        .children
        .iter()
        .flat_map(|child| {
            if child.name.eq_ignore_ascii_case("Part") {
                part(child).into_iter().collect()
            } else if child.name.eq_ignore_ascii_case("Parts") {
                part_list(child)
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn part(element: &Element) -> Option<VehiclePart> {
    if is_set(element.attribute("skipPart")) {
        return None;
    }
    let class = element.attribute("class").unwrap_or_default();
    let kind = kind_of(class);
    Some(VehiclePart {
        name_proof: element.port_name.clone(),
        name: element.attribute("name").unwrap_or_default().to_owned(),
        class: class.to_owned(),
        kind,
        mass: number(element.attribute("mass")),
        damage_max: number(element.attribute("damageMax")),
        detach_ratio: number(element.attribute("detachRatio")),
        destroy_ratio: destroy_ratio(element),
        attributes: element.attributes.clone(),
        origins: element.origins.clone(),
        item_port: element.child("ItemPort").map(item_port),
        tread_wheels: if kind == PartKind::Tread {
            tread_wheels(element)
        } else {
            Vec::new()
        },
        children: part_list(element),
    })
}

const fn kind_of(class: &str) -> PartKind {
    if class.eq_ignore_ascii_case("ItemPort") {
        PartKind::ItemPort
    } else if class.eq_ignore_ascii_case("Tread") {
        PartKind::Tread
    } else if class.eq_ignore_ascii_case("SubPartWheel") {
        PartKind::Wheel
    } else {
        PartKind::Other
    }
}

fn item_port(element: &Element) -> VehicleItemPort {
    VehicleItemPort {
        min_size: size(element.attribute("minSize")),
        max_size: size(element.attribute("maxSize")),
        flags: element.attribute("flags").unwrap_or_default().to_owned(),
        id: element.attribute("id").map(str::to_owned),
        display_name: element
            .attribute("display_name")
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        port_tags: element.attribute("portTags").unwrap_or_default().to_owned(),
        required_tags: element
            .attribute("requiredTags")
            .unwrap_or_default()
            .to_owned(),
        types: type_elements(element)
            .filter_map(|kind| kind.attribute("type"))
            .map(str::to_owned)
            .collect(),
        item_types: type_elements(element)
            .filter_map(|kind| {
                Some(VehicleItemType {
                    item_type: kind.attribute("type")?.to_owned(),
                    subtypes: kind
                        .attribute("subtypes")
                        .unwrap_or_default()
                        .split(',')
                        .map(str::trim)
                        .filter(|subtype| !subtype.is_empty())
                        .map(str::to_owned)
                        .collect(),
                })
            })
            .collect(),
        origins: element.origins.clone(),
        light_attachments: super::light_ports::attachments(element),
    }
}

fn type_elements(port: &Element) -> impl Iterator<Item = &Element> {
    port.child("Types")
        .into_iter()
        .flat_map(|types| types.children_named("Type"))
}

fn tread_wheels(part: &Element) -> Vec<String> {
    part.child("Tread")
        .and_then(|tread| tread.child("Wheels"))
        .into_iter()
        .flat_map(|wheels| wheels.children_named("Wheel"))
        .filter_map(|wheel| wheel.attribute("partName"))
        .map(str::to_owned)
        .collect()
}

/// Returns whether an attribute holds a true flag: `1` or `true`.
fn is_set(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true")
    })
}

fn size(value: Option<&str>) -> Option<u32> {
    value?.trim().parse().ok()
}

/// Returns the share of `damageMax` at which the part destroys the vehicle: the `damageRatioMin`
/// of the first `DamageBehavior` whose `Group` is named `Destroy`, 1 when it states none.
fn destroy_ratio(part: &Element) -> Option<f64> {
    let behavior = part
        .children_named("DamageBehaviors")
        .flat_map(|behaviors| behaviors.children.iter())
        .find(|behavior| {
            behavior
                .child("Group")
                .and_then(|group| group.attribute("name"))
                .is_some_and(|name| name.eq_ignore_ascii_case("Destroy"))
        })?;
    Some(number(behavior.attribute("damageRatioMin")).unwrap_or(1.0))
}

/// Reads the number of an attribute value with the shared tolerant reading; a value that is no
/// number has none.
pub(crate) fn number(value: Option<&str>) -> Option<f64> {
    read_text_number(value?).map(|number| number.value())
}

/// Returns where each item port sits, by lower-case part name: the child indices from the top
/// level down to the part. The first part of a name decides.
pub(super) fn port_paths(parts: &[VehiclePart]) -> HashMap<String, Box<[usize]>> {
    let mut map = HashMap::new();
    let mut path = Vec::new();
    collect_paths(parts, &mut path, &mut map);
    map
}

fn collect_paths(
    parts: &[VehiclePart],
    path: &mut Vec<usize>,
    map: &mut HashMap<String, Box<[usize]>>,
) {
    for (index, part) in parts.iter().enumerate() {
        path.push(index);
        if part.item_port.is_some() {
            map.entry(port_key(&part.name))
                .or_insert_with(|| path.as_slice().into());
        }
        collect_paths(&part.children, path, map);
        path.pop();
    }
}
