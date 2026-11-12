// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::proof::PlacementStep;
use crate::CatalogError;
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryXml, PhysicalEntryRef, XmlNodeSource,
};
use crate::schema_fields::every_field_required;
use crate::starmap::millimetres;
use crate::starmap_transform::Transform;
use crate::starmap_xml::AuthoredPose;

/// The physical file selected and read by the container XML parser.
pub(crate) struct ContainerFile {
    reference: SourcedValue<PhysicalEntryRef>,
    path: SourcedValue<String>,
    root: XmlNodeSource,
}

impl ContainerFile {
    pub(crate) fn from_xml(
        xml: &EntryXml<'_, '_, '_>,
        capture: &EntryCapture<'_>,
    ) -> Result<Self, CatalogError> {
        let receipt = xml.containing_entry()?;
        let sources = capture.source_uses();
        Ok(Self {
            reference: receipt.physical_reference(capture)?.into_cell(sources),
            path: receipt
                .physical_path()?
                .into_cell(sources)
                .map(|value| value.as_str().to_owned()),
            root: xml.node_source(xml.root())?,
        })
    }
}

/// A route to a single physical container occurrence within one system.
pub(crate) struct ContainerOccurrence<'route> {
    pub(crate) system: &'route str,
    pub(crate) root_entry: u32,
    pub(crate) root_inputs: &'route [SourceUse],
    pub(crate) route: Option<&'route PlacementStep>,
    pub(crate) parent_route: Option<&'route PlacementStep>,
    pub(crate) declared: Option<&'route AuthoredEntryPath>,
    pub(crate) local: Option<AuthoredPose>,
    pub(crate) world: Transform,
}

/// The system root and document-local Child ordinals in root-to-leaf order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ContainerInstanceKey {
    root_entry_index: u32,
    child_node_indices: Vec<u32>,
}

impl ContainerInstanceKey {
    fn from_route(root_entry_index: u32, route: Option<&PlacementStep>) -> Self {
        let mut child_node_indices: Vec<u32> = route
            .into_iter()
            .flat_map(PlacementStep::steps)
            .map(PlacementStep::child_index)
            .collect();
        child_node_indices.reverse();
        Self {
            root_entry_index,
            child_node_indices,
        }
    }
}

/// A resolved `.socpak` occurrence with its original local and composed world transforms.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContainerInstance {
    instance_id: SourcedValue<ContainerInstanceKey>,
    parent_instance_id: SourcedValue<ContainerInstanceKey>,
    container: SourcedValue<PhysicalEntryRef>,
    system: SourcedValue<String>,
    source_path: SourcedValue<String>,
    declared_path: SourcedValue<String>,
    local_position: SourcedValue<[f64; 3]>,
    local_rotation: SourcedValue<[f64; 4]>,
    world_position: SourcedValue<[f64; 3]>,
    world_rotation: SourcedValue<[f64; 4]>,
}

impl ContainerOccurrence<'_> {
    pub(crate) fn project(
        self,
        file: &ContainerFile,
        capture: &EntryCapture<'_>,
    ) -> Result<ContainerInstance, CatalogError> {
        let sources = capture.source_uses();
        let (inputs, complete) = self.selection(file)?;
        let shared = sources.shared_origin(&inputs, complete);
        let local_pos = self.local.and_then(|pose| pose.position);
        let local_rot = self.local.and_then(|pose| pose.rotation);
        let declared_path = match self.declared {
            Some(path) => path
                .retain_in(capture)?
                .into_cell(sources)
                .map(|value| value.as_str().to_owned()),
            None => sources.value(SourceValue::Absent, &inputs, complete),
        };
        let parent = self
            .declared
            .map(|_| ContainerInstanceKey::from_route(self.root_entry, self.parent_route));
        let xml_root = file.root.selected()?;
        let transform = self.transform_inputs(&xml_root)?;
        Ok(ContainerInstance {
            instance_id: sources.value_with_shared_origin(
                SourceValue::Present(ContainerInstanceKey::from_route(
                    self.root_entry,
                    self.route,
                )),
                &shared,
                &[xml_root],
                true,
            )?,
            parent_instance_id: sources.value_with_shared_origin(
                state(parent),
                &shared,
                &[],
                true,
            )?,
            container: file.reference.clone(),
            system: sources.value(
                SourceValue::Present(self.system.to_owned()),
                self.root_inputs,
                true,
            ),
            source_path: file.path.clone(),
            declared_path,
            local_position: sources.value(state(local_pos), &[transform.local_position], true),
            local_rotation: sources.value(state(local_rot), &[transform.local_rotation], true),
            world_position: sources.value_with_shared_origin(
                SourceValue::Present(self.world.position.map(millimetres)),
                &shared,
                &transform.world_position,
                true,
            )?,
            world_rotation: sources.value_with_shared_origin(
                SourceValue::Present(self.world.rotation.parts()),
                &shared,
                &transform.world_rotation,
                true,
            )?,
        })
    }

    fn transform_inputs(&self, root: &SourceUse) -> Result<TransformInputs, CatalogError> {
        let Some(last) = self.route else {
            return Ok(TransformInputs {
                local_position: root.clone(),
                local_rotation: root.clone(),
                world_position: Vec::new(),
                world_rotation: Vec::new(),
            });
        };
        Ok(TransformInputs {
            local_position: last.position(SourceUseRole::Payload)?,
            local_rotation: last.rotation(SourceUseRole::Payload)?,
            world_position: last.position_uses()?,
            world_rotation: last
                .steps()
                .map(|step| step.rotation(SourceUseRole::Conversion))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn selection(&self, file: &ContainerFile) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        let mut inputs = self.root_inputs.to_vec();
        let mut complete = true;
        for step in self.route.into_iter().flat_map(PlacementStep::steps) {
            complete &= step.selection(&mut inputs)?;
        }
        if let Some(declared) = self.declared {
            let (selectors, known) = declared.selector_inputs()?;
            inputs.extend(selectors);
            complete &= known;
        } else {
            inputs.push(file.root.selected()?);
        }
        Ok((inputs, complete))
    }
}

struct TransformInputs {
    local_position: SourceUse,
    local_rotation: SourceUse,
    world_position: Vec<SourceUse>,
    world_rotation: Vec<SourceUse>,
}

fn state<T>(value: Option<T>) -> SourceValue<T> {
    value.map_or(SourceValue::Absent, SourceValue::Present)
}
