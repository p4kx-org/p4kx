// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, PhysicalEntryRef};
use crate::schema_fields::every_field_required;
use crate::starmap::millimetres;
use crate::starmap_containers::instance::{ContainerFile, ContainerInstanceKey};
use crate::starmap_containers::location_declaration::ChildLocation;
use crate::starmap_containers::proof::PlacementStep;
use crate::starmap_transform::Transform;
use crate::starmap_xml::AuthoredPose;

/// One typed Child reached through a physical container route.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationInstance {
    instance_id: SourcedValue<ContainerInstanceKey>,
    parent_location_id: SourcedValue<ContainerInstanceKey>,
    container_instance_id: SourcedValue<ContainerInstanceKey>,
    container: SourcedValue<PhysicalEntryRef>,
    local_position: SourcedValue<[f64; 3]>,
    local_rotation: SourcedValue<[f64; 4]>,
    world_position: SourcedValue<[f64; 3]>,
    world_rotation: SourcedValue<[f64; 4]>,
    declaration: ChildLocation,
}

pub(crate) struct LocationOccurrence<'route> {
    pub(crate) root_entry: u32,
    pub(crate) root_inputs: &'route [SourceUse],
    pub(crate) route: &'route PlacementStep,
    pub(crate) parent_location: Option<&'route PlacementStep>,
    pub(crate) container_route: Option<&'route PlacementStep>,
    pub(crate) local: AuthoredPose,
    pub(crate) world: Transform,
}

impl LocationOccurrence<'_> {
    pub(crate) fn project(
        self,
        location: &ChildLocation,
        file: &ContainerFile,
        capture: &EntryCapture<'_>,
    ) -> Result<LocationInstance, CatalogError> {
        let sources = capture.source_uses();
        let mut inputs = self.root_inputs.to_vec();
        let mut complete = true;
        for step in self.route.steps() {
            complete &= step.selection(&mut inputs)?;
        }
        let shared = sources.shared_origin(&inputs, complete);
        let position = self.route.position(SourceUseRole::Payload)?;
        let rotation = self.route.rotation(SourceUseRole::Payload)?;
        let world_rotations = self
            .route
            .steps()
            .map(|step| step.rotation(SourceUseRole::Conversion))
            .collect::<Result<Vec<_>, _>>()?;
        let route_key = |route| ContainerInstanceKey::from_route(self.root_entry, route);
        Ok(LocationInstance {
            instance_id: sources.value_with_shared_origin(
                SourceValue::Present(route_key(Some(self.route))),
                &shared,
                &[],
                true,
            )?,
            parent_location_id: sources.value_with_shared_origin(
                self.parent_location.map_or(SourceValue::Absent, |parent| {
                    SourceValue::Present(route_key(Some(parent)))
                }),
                &shared,
                &[],
                true,
            )?,
            container_instance_id: sources.value_with_shared_origin(
                SourceValue::Present(route_key(self.container_route)),
                &shared,
                &[],
                true,
            )?,
            container: file.reference().clone(),
            local_position: sources.value(state(self.local.position), &[position], true),
            local_rotation: sources.value(state(self.local.rotation), &[rotation], true),
            world_position: sources.value_with_shared_origin(
                SourceValue::Present(self.world.position.map(millimetres)),
                &shared,
                &self.route.position_uses()?,
                true,
            )?,
            world_rotation: sources.value_with_shared_origin(
                SourceValue::Present(self.world.rotation.parts()),
                &shared,
                &world_rotations,
                true,
            )?,
            declaration: location.clone(),
        })
    }
}

fn state<T>(value: Option<T>) -> SourceValue<T> {
    value.map_or(SourceValue::Absent, SourceValue::Present)
}
