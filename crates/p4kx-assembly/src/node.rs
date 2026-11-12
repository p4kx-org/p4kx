// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_dataforge::Record;

use crate::entities::describe;
use crate::geometry_choice::GeometryChoice;
use crate::merge::port_key;
use crate::node_body::is_head_type;
use crate::node_inputs::NodeInputs;
use crate::node_notes;
use crate::port::PortDefinition;
use crate::resolver::LoadoutResolver;
use crate::resolver_observation::{self, LoadoutReadEvent};
use crate::spec::{EntitySpec, EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::vehicle::flags_contain;
use crate::walk::Walk;
use crate::{
    Attachment, EntityNode, LoadoutError, LoadoutReferenceSelection, LoadoutTargetGuid,
    LoadoutXmlClassUse, PortConstraints, PortNode, PortSource, PortVisibility, Target, Unresolved,
    VehicleImplementation,
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
    xml_use: Option<&'s LoadoutXmlClassUse>,
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
        if self.observation_stopped() {
            return shell(spec, attached_vehicle, false);
        }
        if self.stop_at_docked && attached_vehicle {
            walk.stopped_here();
            return shell(spec, attached_vehicle, false);
        }
        let mut issues = Vec::new();
        let mut ports = Vec::new();
        let Some(inputs) = self.node_inputs(spec, overrides, &mut issues) else {
            return shell(spec, attached_vehicle, false);
        };
        let NodeInputs {
            vehicle,
            landing_gear,
            entries,
        } = inputs;
        let shared = shared_port_flags(spec);
        for pending in pending_ports(spec, &entries, vehicle.as_deref()) {
            if self.observation_stopped() || (walk.cancellable && self.stopped()) {
                break;
            }
            if !walk.reserve() {
                if resolver_observation::notify(
                    self.observation.as_deref(),
                    LoadoutReadEvent::InventoryIncomplete,
                )
                .is_err()
                {
                    issues.push(LoadoutError::ObservationStopped);
                    break;
                }
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
            node.attachment = self.attach(pending.target, pending.loadout, pending.xml_use, walk);
            if matches!(node.attachment, Attachment::Empty) {
                node.nested = self.nested_ports(pending.loadout, walk, issues);
            }
        }
        if !self.observation_stopped() {
            self.choose_sub_geometry(&mut node, vehicle, shared);
        }
        node
    }

    /// Resolves what one port holds.
    pub(crate) fn attach(
        &mut self,
        target: &TargetSpec,
        loadout: Option<&LoadoutSpec>,
        xml_use: Option<&LoadoutXmlClassUse>,
        walk: &mut Walk,
    ) -> Attachment {
        let failure = |named, error| unresolved(named, error, target);
        if self.observation_stopped() {
            return failure(Target::Unreadable, LoadoutError::ObservationStopped);
        }
        let named = match target {
            TargetSpec::None => return Attachment::Empty,
            TargetSpec::Invalid(reason) => {
                return failure(
                    Target::Unreadable,
                    LoadoutError::InvalidReference { reason: *reason },
                );
            }
            TargetSpec::Reference(reference) => Target::Reference(reference.guid()),
            TargetSpec::ClassName(name) => Target::ClassName(name.value().to_owned()),
        };
        let Some(record) = self.record_of(target) else {
            let error = match target {
                TargetSpec::Reference(reference) => LoadoutError::DanglingReference {
                    guid: LoadoutTargetGuid::from_reference(reference),
                },
                _ => LoadoutError::UnknownClass {
                    name: class_text(target),
                },
            };
            return failure(named, error);
        };
        let selected = match self.selected_target(target, &record) {
            Ok(selected) => selected,
            Err(error) => return failure(named, error),
        };
        let guid = selected.guid();
        if let Some(error) = walk.limit(guid) {
            if matches!(error, LoadoutError::TooDeep)
                && resolver_observation::notify(
                    self.observation.as_deref(),
                    LoadoutReadEvent::InventoryIncomplete,
                )
                .is_err()
            {
                return failure(named, LoadoutError::ObservationStopped);
            }
            return failure(named, error.with_target(&selected, xml_use));
        }
        let spec = self.spec_record(&record);
        let spec = match spec.as_ref() {
            Ok(spec) => spec,
            Err(error) => return failure(named, error.clone().with_target(&selected, xml_use)),
        };
        walk.ancestors.push(guid);
        let mut child = self.node(spec, loadout, walk);
        child.attachment_target = Some(selected.with_xml_use(xml_use));
        walk.ancestors.pop();
        if !self.observation_stopped()
            && let Some(ambiguity) = self.ambiguity(target)
        {
            child.issues.insert(0, ambiguity);
        }
        Attachment::Entity(Box::new(child))
    }

    fn selected_target(
        &self,
        target: &TargetSpec,
        record: &Record<'_>,
    ) -> Result<LoadoutTargetGuid, LoadoutError> {
        match target {
            TargetSpec::ClassName(class) => {
                if let (Some(observation), Some(selection)) =
                    (self.observation.as_deref(), class.selection())
                {
                    observation.class_record(selection, record)?;
                }
                Ok(LoadoutTargetGuid::from_class(record.guid(), class))
            }
            TargetSpec::Reference(reference) => Ok(LoadoutTargetGuid::from_reference(reference)),
            _ => Ok(LoadoutTargetGuid::unbound(record.guid())),
        }
    }

    /// Returns the ambiguity of a class name that several Entities have, if it is one.
    fn ambiguity(&self, target: &TargetSpec) -> Option<LoadoutError> {
        let TargetSpec::ClassName(name) = target else {
            return None;
        };
        let candidates = self.entities.class_candidates(name.value());
        (candidates.len() > 1).then(|| LoadoutError::AmbiguousClass {
            name: name.value().to_owned(),
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
            xml_use: None,
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
        xml_use: entry.xml_use.as_ref(),
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
            xml_use: None,
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
        record_index: spec.record_index,
        attachable_component: spec.attachable_component,
        attachment_target: None,
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
        landing_system_component: spec
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.landing_system.is_some())
            .map(|vehicle| vehicle.component),
        docked_vehicle,
        loadout_file: match &spec.loadout {
            Some(LoadoutSpec::Xml(path)) => Some(Arc::clone(path.text())),
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
        TargetSpec::ClassName(name) => name.value().to_owned(),
        _ => String::new(),
    }
}

fn unresolved(target: Target, error: LoadoutError, source: &TargetSpec) -> Attachment {
    let error = match source {
        TargetSpec::Reference(reference) => error.with_reference(reference),
        _ => error,
    };
    let reference: Option<LoadoutReferenceSelection> = source.reference_selection().cloned();
    Attachment::Unresolved(Unresolved {
        target,
        error,
        reference,
    })
}
