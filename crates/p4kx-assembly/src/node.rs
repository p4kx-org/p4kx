// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use crate::entities::describe;
use crate::geometry_choice::GeometryChoice;
use crate::merge::{merge_by_port, port_key};
use crate::node_body::is_head_type;
use crate::node_notes;
use crate::port::PortDefinition;
use crate::resolver::LoadoutResolver;
use crate::spec::{EntitySpec, EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::vehicle::flags_contain;
use crate::walk::Walk;
use crate::{
    Attachment, EntityNode, LoadoutError, PortConstraints, PortNode, PortSource, PortVisibility,
    Target, Unresolved, VehicleImplementation,
};

/// Returns the flags `spec`'s port container gives all its ports when they choose a sub-geometry:
/// those of a head, which name the character the face parts belong to; none for any other Entity.
fn shared_port_flags(spec: &EntitySpec) -> &[String] {
    if is_head_type(&spec.attach_type) {
        &spec.port_flags
    } else {
        &[]
    }
}

/// One port of an Entity before it is resolved.
struct Pending<'s> {
    name: &'s str,
    source: PortSource,
    definition: Option<Arc<PortDefinition>>,
    target: &'s TargetSpec,
    loadout: Option<&'s LoadoutSpec>,
    origin: Option<&'s LoadoutOrigin>,
}

impl LoadoutResolver<'_> {
    /// Builds the node of an Entity whose own Loadout is overridden per port by `overrides`.
    pub(crate) fn node(
        &mut self,
        spec: &EntitySpec,
        overrides: Option<&LoadoutSpec>,
        walk: &mut Walk,
    ) -> EntityNode {
        let attached_vehicle = spec.docked_vehicle && !walk.is_root();
        if self.stop_at_docked && attached_vehicle {
            walk.stopped_here();
            return shell(spec, attached_vehicle, false);
        }
        let mut issues = Vec::new();
        let mut ports = Vec::new();
        let vehicle = self.vehicle_with_issues(spec, &mut issues);
        let landing_gear = self.landing_gear_of(spec, &mut issues);
        let entries = self.merged_entries(spec, overrides, &mut issues);
        let shared = shared_port_flags(spec);
        for pending in pending_ports(spec, &entries, vehicle.as_deref()) {
            if walk.cancellable && self.stopped() {
                break;
            }
            if !walk.reserve() {
                issues.push(LoadoutError::TooManyPorts);
                break;
            }
            walk.enter(pending.name);
            let mut port = self.port_node(pending, walk, &mut issues, vehicle.as_deref(), shared);
            walk.leave();
            port.visibility = visibility_of(&port, vehicle.as_deref());
            ports.push(port);
        }
        let (issues, notes) = node_notes::split(issues);
        EntityNode {
            ports,
            issues,
            notes,
            vehicle,
            landing_gear,
            ..shell(spec, attached_vehicle, true)
        }
    }

    /// Returns the vehicle implementation of `spec`, pushing its issues to `issues`.
    fn vehicle_with_issues(
        &mut self,
        spec: &EntitySpec,
        issues: &mut Vec<LoadoutError>,
    ) -> Option<Arc<VehicleImplementation>> {
        match self.vehicle_of(spec) {
            Some(Ok(vehicle)) => {
                issues.extend(vehicle.issues.iter().cloned());
                Some(vehicle)
            }
            Some(Err(error)) => {
                issues.push(error);
                None
            }
            None => None,
        }
    }

    fn port_node(
        &mut self,
        pending: Pending<'_>,
        walk: &mut Walk,
        issues: &mut Vec<LoadoutError>,
        vehicle: Option<&VehicleImplementation>,
        shared: &[String],
    ) -> PortNode {
        let constraints = pending
            .definition
            .as_deref()
            .and_then(PortConstraints::of_definition)
            .or_else(|| {
                vehicle
                    .and_then(|vehicle| vehicle.item_port(pending.name))
                    .and_then(PortConstraints::of_vehicle)
            })
            .map(Arc::new);
        let mut node = PortNode {
            name: pending.name.to_owned(),
            source: pending.source,
            definition: pending.definition,
            constraints,
            attachment: Attachment::Empty,
            nested: Vec::new(),
            visibility: PortVisibility::default(),
            origin: pending.origin.cloned(),
        };
        if !self.change(&mut node, walk) {
            node.attachment = self.attach(pending.target, pending.loadout, walk);
            if matches!(node.attachment, Attachment::Empty) {
                node.nested = self.nested_ports(pending.loadout, walk, issues);
            }
        }
        self.choose_sub_geometry(&mut node, vehicle, shared);
        node
    }

    /// Returns the entries of the Entity's own Loadout, each replaced in place by the entry of
    /// the same port in `overrides`, with the other overriding entries appended.
    fn merged_entries(
        &mut self,
        spec: &EntitySpec,
        overrides: Option<&LoadoutSpec>,
        issues: &mut Vec<LoadoutError>,
    ) -> Vec<(EntrySpec, PortSource)> {
        let mut entries = Vec::new();
        match self.entries(spec.loadout.as_ref()) {
            Ok(own) => {
                issues.extend(own.issues.iter().cloned());
                entries.extend(
                    own.list
                        .iter()
                        .map(|entry| (entry.clone(), PortSource::Loadout)),
                );
            }
            Err(error) => issues.push(error),
        }
        if overrides.is_some() {
            match self.entries(overrides) {
                Ok(given) => {
                    issues.extend(given.issues.iter().cloned());
                    let given = given.list.iter().map(|e| (e.clone(), PortSource::Override));
                    merge_by_port(&mut entries, given, |(entry, _)| &entry.port);
                }
                Err(error) => issues.push(error),
            }
        }
        entries
    }

    /// Resolves what one port holds.
    pub(crate) fn attach(
        &mut self,
        target: &TargetSpec,
        loadout: Option<&LoadoutSpec>,
        walk: &mut Walk,
    ) -> Attachment {
        let named = match target {
            TargetSpec::None => return Attachment::Empty,
            TargetSpec::Invalid(reason) => {
                return unresolved(
                    Target::Unreadable,
                    LoadoutError::InvalidReference { reason: *reason },
                );
            }
            TargetSpec::Reference(guid) => Target::Reference(*guid),
            TargetSpec::ClassName(name) => Target::ClassName(name.clone()),
        };
        let Some(record) = self.record_of(target) else {
            let error = match target {
                TargetSpec::Reference(guid) => LoadoutError::DanglingReference { guid: *guid },
                _ => LoadoutError::UnknownClass {
                    name: class_text(target),
                },
            };
            return unresolved(named, error);
        };
        let guid = record.guid();
        if let Some(error) = walk.limit(guid) {
            return unresolved(named, error);
        }
        let spec = self.spec(guid);
        let spec = match spec.as_ref() {
            Ok(spec) => spec,
            Err(error) => return unresolved(named, error.clone()),
        };
        walk.ancestors.push(guid);
        let mut child = self.node(spec, loadout, walk);
        walk.ancestors.pop();
        if let Some(ambiguity) = self.ambiguity(target) {
            child.issues.insert(0, ambiguity);
        }
        Attachment::Entity(Box::new(child))
    }

    /// Returns the ambiguity of a class name that several Entities have, if it is one.
    fn ambiguity(&self, target: &TargetSpec) -> Option<LoadoutError> {
        let TargetSpec::ClassName(name) = target else {
            return None;
        };
        let candidates = self.entities.class_candidates(name);
        (candidates.len() > 1).then(|| LoadoutError::AmbiguousClass {
            name: name.clone(),
            candidates: candidates.iter().map(describe).collect(),
        })
    }
}

/// Returns the ports of an Entity in order: the merged Loadout entries, then the defined ports
/// no entry names, then the item ports only the vehicle implementation declares, each part once
/// even where several share a name.
fn pending_ports<'s>(
    spec: &'s EntitySpec,
    entries: &'s [(EntrySpec, PortSource)],
    vehicle: Option<&'s VehicleImplementation>,
) -> impl Iterator<Item = Pending<'s>> {
    let named: HashSet<String> = entries
        .iter()
        .map(|(entry, _)| port_key(&entry.port))
        .collect();
    let mut taken = named.clone();
    taken.extend(spec.ports.iter().map(|port| port_key(&port.name)));
    let declared = vehicle
        .into_iter()
        .flat_map(VehicleImplementation::item_ports)
        .filter(move |part| !taken.contains(&port_key(part.name())))
        .map(|part| Pending {
            name: part.name(),
            source: PortSource::Empty,
            definition: None,
            target: &TargetSpec::None,
            loadout: None,
            origin: None,
        });
    let from_entries = entries.iter().map(|(entry, source)| Pending {
        name: &entry.port,
        source: *source,
        definition: spec
            .port(&entry.port)
            .map(|port| Arc::clone(&port.definition)),
        target: &entry.target,
        loadout: entry.loadout.as_ref(),
        origin: Some(&entry.origin),
    });
    let defined = spec
        .ports
        .iter()
        .filter(move |port| !named.contains(&port_key(&port.name)))
        .map(|port| Pending {
            name: &port.name,
            source: if port.default_item == TargetSpec::None {
                PortSource::Unset
            } else {
                PortSource::PortDefault
            },
            definition: Some(Arc::clone(&port.definition)),
            target: &port.default_item,
            loadout: None,
            origin: (port.default_item != TargetSpec::None).then_some(&spec.origin),
        });
    from_entries.chain(defined).chain(declared)
}

/// Returns the visibility of `port`: the verdict of the vehicle implementation's part of that
/// name, else [`PortVisibility::Invisible`] when the port's own definition carries the flag
/// `invisible`.
fn visibility_of(port: &PortNode, vehicle: Option<&VehicleImplementation>) -> PortVisibility {
    let from_vehicle = vehicle.map_or(PortVisibility::Unspecified, |vehicle| {
        vehicle.port_visibility(&port.name)
    });
    if from_vehicle != PortVisibility::Unspecified {
        return from_vehicle;
    }
    let flagged = port
        .definition
        .as_deref()
        .is_some_and(|definition| flags_contain(definition.flags(), "invisible"));
    if flagged {
        PortVisibility::Invisible
    } else {
        PortVisibility::Unspecified
    }
}

/// Returns the node of `spec` without ports, issues, vehicle implementation and landing gear.
fn shell(spec: &EntitySpec, docked_vehicle: bool, expanded: bool) -> EntityNode {
    EntityNode {
        guid: spec.guid,
        record_path: spec.record_path.clone(),
        name: spec.name.clone(),
        ports: Vec::new(),
        issues: Vec::new(),
        notes: Vec::new(),
        geometry: spec.geometry.clone(),
        fallback: None,
        choice: GeometryChoice::Main,
        own_skin: None,
        vehicle: None,
        landing_gear: Arc::from([]),
        docked_vehicle,
        loadout_file: match &spec.loadout {
            Some(LoadoutSpec::Xml(path)) => Some(Arc::clone(path)),
            _ => None,
        },
        expanded,
        attach_type: spec.attach_type.clone(),
        attach_sub_type: spec.attach_sub_type.clone(),
        size: spec.size,
        container: spec.container,
        object_containers: Arc::clone(&spec.object_containers),
        zones: Arc::clone(&spec.zones.chunks),
        animation_database: spec.animation_database.clone(),
        hidden_ports: Arc::clone(&spec.zones.hidden_ports),
    }
}

fn class_text(target: &TargetSpec) -> String {
    match target {
        TargetSpec::ClassName(name) => name.clone(),
        _ => String::new(),
    }
}

const fn unresolved(target: Target, error: LoadoutError) -> Attachment {
    Attachment::Unresolved(Unresolved { target, error })
}
