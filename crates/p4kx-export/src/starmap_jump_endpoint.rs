// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SharedFieldOrigin, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, PhysicalEntryRef};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap;
use crate::starmap_containers::instance::{ContainerFile, ContainerInstanceKey};
use crate::starmap_containers::jump_class::EndpointClass;
use crate::starmap_containers::jump_gate::JumpGate;
use crate::starmap_containers::jump_operating::OperatingResolver;
use crate::starmap_containers::proof::{ChildOrigins, PlacementStep};
use crate::starmap_transform::Transform;
use crate::starmap_xml::AuthoredPose;

/// One physical Child occurrence of an original jump endpoint.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpEndpoint {
    #[serde(flatten)]
    route: EndpointRoute,
    #[serde(flatten)]
    pose: EndpointPose,
    gate: SourcedValue<JumpGate>,
    target_candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
    declared_partner: SourcedValue<CatalogGuid>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EndpointRoute {
    instance_id: SourcedValue<ContainerInstanceKey>,
    container_instance_id: SourcedValue<ContainerInstanceKey>,
    container: SourcedValue<PhysicalEntryRef>,
    system: SourcedValue<String>,
    starmap_record_guid: SourcedValue<CatalogGuid>,
    starmap_record_index: SourcedValue<u32>,
    starmap_type: SourcedValue<&'static str>,
    declaration: JumpChild,
    target_path: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EndpointPose {
    local_position: SourcedValue<[f64; 3]>,
    local_rotation: SourcedValue<[f64; 4]>,
    world_position: SourcedValue<[f64; 3]>,
    world_rotation: SourcedValue<[f64; 4]>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct JumpChild {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) class_guid: SourcedValue<CatalogGuid>,
    pub(crate) entity_name: SourcedValue<String>,
}

pub(crate) struct JumpOccurrence<'route> {
    pub(crate) root_entry: u32,
    pub(crate) root_inputs: &'route [SourceUse],
    pub(crate) system: &'route str,
    pub(crate) route: &'route PlacementStep,
    pub(crate) container_route: Option<&'route PlacementStep>,
    pub(crate) local: Option<AuthoredPose>,
    pub(crate) world: Option<Transform>,
    pub(crate) record_guid: CatalogGuid,
    pub(crate) class: &'route EndpointClass,
    pub(crate) child: &'route ChildOrigins,
    pub(crate) target: Option<&'route AuthoredEntryPath>,
}

pub(crate) struct GateResolution {
    pub(crate) gate: SourceValue<JumpGate>,
    pub(crate) candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
    pub(crate) proof: Vec<SourceUse>,
}

impl JumpEndpoint {
    /// Physical system root and complete Child route of this endpoint.
    #[must_use]
    pub const fn instance_id(&self) -> &SourcedValue<ContainerInstanceKey> {
        &self.route.instance_id
    }

    pub(crate) const fn system(&self) -> &SourcedValue<String> {
        &self.route.system
    }

    pub(crate) const fn starmap_record_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.route.starmap_record_guid
    }

    pub(crate) const fn starmap_type(&self) -> &SourcedValue<&'static str> {
        &self.route.starmap_type
    }

    pub(crate) const fn world_position(&self) -> &SourcedValue<[f64; 3]> {
        &self.pose.world_position
    }

    pub(crate) const fn target_path(&self) -> &SourcedValue<String> {
        &self.route.target_path
    }

    pub(crate) const fn target_candidates(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>> {
        &self.target_candidates
    }

    /// Physical DataForge index of the Starmap object classifying this endpoint.
    #[must_use]
    pub const fn starmap_record_index(&self) -> &SourcedValue<u32> {
        &self.route.starmap_record_index
    }

    /// Original annotation of the inspected partner declaration scope.
    #[must_use]
    pub const fn declared_partner(&self) -> &SourcedValue<CatalogGuid> {
        &self.declared_partner
    }

    pub(super) fn with_operating(
        mut self,
        resolver: &mut OperatingResolver<'_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        self.gate = self.gate.try_map(|gate| gate.with_operating(resolver))?;
        Ok(self)
    }

    pub(crate) fn partner_scope_complete(&self) -> bool {
        self.declared_partner.origin().source_set_complete()
    }
}

impl JumpOccurrence<'_> {
    pub(crate) fn project(
        self,
        capture: &EntryCapture<'_>,
        file: &ContainerFile,
        GateResolution {
            gate,
            candidates,
            proof,
        }: GateResolution,
    ) -> Result<JumpEndpoint, CatalogError> {
        let target_proof = proof.as_slice();
        let sources = capture.source_uses();
        let mut inputs = self.root_inputs.to_vec();
        let mut complete = true;
        for step in self.route.steps() {
            complete &= step.selection(&mut inputs)?;
        }
        inputs.extend(self.class.inputs.iter().cloned());
        let shared = sources.shared_origin(&inputs, complete);
        let partner_uses = match &gate {
            SourceValue::Present(gate) => target_proof
                .iter()
                .chain(gate.partner_evidence())
                .cloned()
                .collect(),
            _ => target_proof.to_vec(),
        };
        let readable = !matches!(gate, SourceValue::Unreadable | SourceValue::Invalid);
        let partner_complete = match &gate {
            SourceValue::Present(gate) => gate.partner_scope_complete(),
            SourceValue::Absent => true,
            _ => false,
        };
        let partner = if partner_complete {
            SourceValue::Absent
        } else {
            SourceValue::Unreadable
        };
        Ok(JumpEndpoint {
            route: self.route_fields(capture, file, &shared, &inputs, complete)?,
            pose: self.pose_fields(sources, &shared)?,
            gate: sources.value_with_shared_origin(gate, &shared, target_proof, readable)?,
            target_candidates: candidates,
            declared_partner: sources.value_with_shared_origin(
                partner,
                &shared,
                &partner_uses,
                partner_complete,
            )?,
        })
    }
    fn route_fields(
        &self,
        capture: &EntryCapture<'_>,
        file: &ContainerFile,
        shared: &SharedFieldOrigin,
        inputs: &[SourceUse],
        complete: bool,
    ) -> Result<EndpointRoute, CatalogError> {
        let sources = capture.source_uses();
        let record = self.child.record_selector()?;
        let route_key = |route| ContainerInstanceKey::from_route(self.root_entry, route);
        let target_path = match self.target {
            Some(path) => path
                .retain_in(capture)?
                .into_cell(sources)
                .map(|value| value.as_str().to_owned()),
            None => match self.child.unusable_external_name()? {
                Some(name) => sources.value(
                    SourceValue::Invalid,
                    &[self.child.external_selector()?, name],
                    complete,
                ),
                None => sources.value(SourceValue::Absent, inputs, complete),
            },
        };
        Ok(EndpointRoute {
            instance_id: sources.value_with_shared_origin(
                SourceValue::Present(route_key(Some(self.route))),
                shared,
                &[],
                true,
            )?,
            container_instance_id: sources.value_with_shared_origin(
                SourceValue::Present(route_key(self.container_route)),
                shared,
                &[],
                true,
            )?,
            container: file.reference().clone(),
            system: sources.value(
                SourceValue::Present(self.system.to_owned()),
                self.root_inputs,
                true,
            ),
            starmap_record_guid: sources.value(
                SourceValue::Present(self.record_guid),
                &[record],
                true,
            ),
            starmap_record_index: sources.value(
                SourceValue::Present(self.class.index),
                std::slice::from_ref(&self.class.index_input),
                true,
            ),
            starmap_type: sources.value(
                SourceValue::Present(self.class.name),
                &self.class.inputs,
                true,
            ),
            declaration: self.child.jump_declaration(sources)?,
            target_path,
        })
    }

    fn pose_fields(
        &self,
        sources: &SourceUses,
        shared: &SharedFieldOrigin,
    ) -> Result<EndpointPose, CatalogError> {
        let world_position = self.world.map_or(SourceValue::Invalid, |world| {
            SourceValue::Present(world.position.map(starmap::millimetres))
        });
        let world_rotation = self.world.map_or(SourceValue::Invalid, |world| {
            SourceValue::Present(world.rotation.parts())
        });
        let rotation_uses = self
            .route
            .steps()
            .map(|step| step.rotation(SourceUseRole::Conversion))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EndpointPose {
            local_position: sources.value(
                local(self.local, |value| value.position),
                &[self.route.position(SourceUseRole::Payload)?],
                true,
            ),
            local_rotation: sources.value(
                local(self.local, |value| value.rotation),
                &[self.route.rotation(SourceUseRole::Payload)?],
                true,
            ),
            world_position: sources.value_with_shared_origin(
                world_position,
                shared,
                &self.route.position_uses()?,
                true,
            )?,
            world_rotation: sources.value_with_shared_origin(
                world_rotation,
                shared,
                &rotation_uses,
                true,
            )?,
        })
    }
}

fn local<T>(
    pose: Option<AuthoredPose>,
    select: impl FnOnce(AuthoredPose) -> Option<T>,
) -> SourceValue<T> {
    pose.map_or(SourceValue::Invalid, |pose| {
        select(pose).map_or(SourceValue::Absent, SourceValue::Present)
    })
}
