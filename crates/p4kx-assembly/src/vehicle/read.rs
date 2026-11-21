// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::SourceValue;
use p4kx_foundation::read_text_number;

use super::aggregate_evidence::Checkpoint;
use super::{
    ITEM_TYPE_ATTRIBUTE, PartKind, VehicleAggregateEvidence, VehicleItemPort, VehicleItemType,
    VehiclePart,
};
use crate::markup::Element;
use crate::merge::port_key;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct IndexedPort {
    pub(super) path: Box<[usize]>,
    pub(super) ordinal: usize,
}

pub(super) struct PartTree {
    pub(super) parts: Vec<VehiclePart>,
    pub(super) paths: HashMap<String, IndexedPort>,
    pub(super) selectors: Option<VehicleAggregateEvidence>,
    pub(super) port_occurrences: Vec<Checkpoint>,
    pub(super) rolling_parts: bool,
    pub(super) rolling_selectors: Option<VehicleAggregateEvidence>,
}

pub(super) fn parts(
    vehicle: &Element,
    totals: Option<&mut VehicleAggregateEvidence>,
    observe_ports: bool,
) -> PartTree {
    let rolling_selectors = totals.is_some().then(VehicleAggregateEvidence::new);
    let mut reader = PartReader {
        totals,
        selectors: observe_ports.then(VehicleAggregateEvidence::new),
        rolling_observation: rolling_selectors,
        rolling_parts: false,
        found_wheel: false,
        paths: HashMap::new(),
        path: Vec::new(),
        port_count: 0,
        port_occurrences: Vec::new(),
    };
    reader.element(vehicle, "$children");
    let mut parts = Vec::new();
    for child in &vehicle.children {
        reader.element(child, "$name");
        if child.name.eq_ignore_ascii_case("Parts") {
            reader.append_parts(child, &mut parts);
        }
    }
    PartTree {
        parts,
        paths: reader.paths,
        selectors: reader.selectors,
        port_occurrences: reader.port_occurrences,
        rolling_parts: reader.rolling_parts,
        rolling_selectors: reader.rolling_observation,
    }
}

struct PartReader<'scope> {
    totals: Option<&'scope mut VehicleAggregateEvidence>,
    selectors: Option<VehicleAggregateEvidence>,
    rolling_observation: Option<VehicleAggregateEvidence>,
    rolling_parts: bool,
    found_wheel: bool,
    paths: HashMap<String, IndexedPort>,
    path: Vec<usize>,
    port_count: usize,
    port_occurrences: Vec<Checkpoint>,
}

impl PartReader<'_> {
    fn rolling_field(&mut self, element: &Element, field: &str) {
        if let Some(selectors) = self.rolling_observation() {
            if element.attribute(field).is_some() && element.origins.evidence(field).is_none() {
                selectors.issue(super::VehicleAggregateIssue::OriginalEvidenceUnavailable);
            } else {
                selectors.field(element, field);
            }
        }
    }

    const fn rolling_observation(&mut self) -> Option<&mut VehicleAggregateEvidence> {
        if self.found_wheel {
            None
        } else {
            self.rolling_observation.as_mut()
        }
    }
    fn element(&mut self, element: &Element, field: &str) {
        if let Some(totals) = &mut self.totals {
            totals.element(element, field);
        }
        if let Some(selectors) = &mut self.selectors {
            selectors.element(element, field);
        }
        if let Some(selectors) = self.rolling_observation() {
            selectors.element(element, field);
        }
    }

    fn append_parts(&mut self, container: &Element, parts: &mut Vec<VehiclePart>) {
        self.element(container, "$children");
        for child in &container.children {
            self.element(child, "$name");
            if child.name.eq_ignore_ascii_case("Part") {
                self.path.push(parts.len());
                parts.extend(self.part(child));
                self.path.pop();
            } else if child.name.eq_ignore_ascii_case("Parts") {
                self.append_parts(child, parts);
            }
        }
    }

    fn part(&mut self, element: &Element) -> Option<VehiclePart> {
        if let Some(totals) = &mut self.totals {
            totals.field(element, "skipPart");
        }
        if let Some(selectors) = &mut self.selectors {
            selectors.field(element, "skipPart");
        }
        self.rolling_field(element, "skipPart");
        if is_set(element.attribute("skipPart")) {
            return None;
        }
        let name = element.attribute("name").unwrap_or_default();
        let class = element.attribute("class").unwrap_or_default();
        let kind = kind_of(class);
        self.rolling_field(element, "class");
        self.rolling_parts |= matches!(kind, PartKind::Wheel | PartKind::Tread);
        self.found_wheel |= kind == PartKind::Wheel;
        let port = self.item_port(element, name);
        let mut children = Vec::new();
        self.append_parts(element, &mut children);
        Some(VehiclePart {
            name_proof: element.port_name.clone(),
            name: name.to_owned(),
            class: class.to_owned(),
            kind,
            mass: number(element.attribute("mass")),
            damage_max: number(element.attribute("damageMax")),
            detach_ratio: number(element.attribute("detachRatio")),
            destroy_ratio: destroy_ratio(element),
            attributes: element.attributes.clone(),
            origins: element.origins.clone(),
            item_port: port,
            tread_wheels: if kind == PartKind::Tread {
                tread_wheels(element)
            } else {
                Vec::new()
            },
            children,
        })
    }

    fn item_port(&mut self, part: &Element, name: &str) -> Option<VehicleItemPort> {
        let source = if let Some(selectors) = &mut self.selectors {
            selectors.element(part, "$children");
            let source = part.children.iter().find(|child| {
                selectors.element(child, "$name");
                child.name.eq_ignore_ascii_case("ItemPort")
            })?;
            selectors.field(part, "name");
            source
        } else {
            part.child("ItemPort")?
        };
        let ordinal = self.port_count;
        self.port_count += 1;
        if let Some(selectors) = &self.selectors {
            self.port_occurrences.push(selectors.checkpoint());
        }
        self.paths
            .entry(port_key(name))
            .or_insert_with(|| IndexedPort {
                path: self.path.as_slice().into(),
                ordinal,
            });
        Some(item_port(source))
    }
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
    let item_types = read_types(element);
    VehicleItemPort {
        min_size: size(element.attribute("minSize")),
        max_size: size(element.attribute("maxSize")),
        flags: element.attribute("flags").map(str::to_owned),
        id: element.attribute("id").map(str::to_owned),
        display_name: element.attribute("display_name").map(str::to_owned),
        port_tags: element.attribute("portTags").map(str::to_owned),
        required_tags: element.attribute("requiredTags").map(str::to_owned),
        types: item_types
            .iter()
            .map(|kind| kind.item_type.clone())
            .collect(),
        item_types,
        origins: element.origins.clone(),
        light_attachments: super::light_ports::attachments(element),
    }
}

fn read_types(port: &Element) -> Vec<VehicleItemType> {
    let Some(collection) = port.child("Types") else {
        return Vec::new();
    };
    let collection_site = collection.origins.shared_element();
    collection
        .children_named("Type")
        .filter_map(|element| {
            Some(VehicleItemType {
                item_type: element.attribute(ITEM_TYPE_ATTRIBUTE)?.to_owned(),
                subtypes: element
                    .attribute("subtypes")
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|subtype| !subtype.is_empty())
                    .map(str::to_owned)
                    .collect(),
                assignment: element.origins.evidence(ITEM_TYPE_ATTRIBUTE).cloned(),
                collection: collection_site.clone(),
                declaration: element.origins.shared_element(),
            })
        })
        .collect()
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

fn size(value: Option<&str>) -> SourceValue<u32> {
    value.map_or(SourceValue::Absent, |value| {
        value
            .trim()
            .parse()
            .map_or(SourceValue::Invalid, SourceValue::Present)
    })
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
