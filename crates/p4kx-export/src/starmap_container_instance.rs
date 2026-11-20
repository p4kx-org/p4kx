// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::proof::{ChildNames, PlacementStep, container_route_uses};
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryXml, PhysicalEntryRef, XmlNodeSource,
};
use crate::schema_fields::every_field_required;
use crate::starmap::millimetres;
use crate::starmap_transform::Transform;
use crate::starmap_xml::AuthoredPose;
use crate::{CatalogError, HealthText};

/// The physical file selected and read by the container XML parser.
pub(crate) struct ContainerFile {
    reference: SourcedValue<PhysicalEntryRef>,
    path: SourcedValue<HealthText>,
    root: XmlNodeSource,
}

impl ContainerFile {
    pub(crate) const fn reference(&self) -> &SourcedValue<PhysicalEntryRef> {
        &self.reference
    }

    pub(crate) fn from_xml(
        xml: &EntryXml<'_, '_, '_>,
        capture: &EntryCapture<'_>,
    ) -> Result<Self, CatalogError> {
        let receipt = xml.containing_entry()?;
        let sources = capture.source_uses();
        Ok(Self {
            reference: receipt.physical_reference(capture)?.into_cell(sources),
            path: receipt.physical_path()?.into_cell(sources),
            root: xml.node_source(xml.root())?,
        })
    }
}

/// A route to a single physical container occurrence within one system.
pub(crate) struct ContainerOccurrence<'route> {
    pub(crate) system: &'route SourcedValue<HealthText>,
    pub(crate) root_entry: u32,
    pub(crate) root_inputs: &'route [SourceUse],
    pub(crate) route: Option<&'route PlacementStep>,
    pub(crate) parent_route: Option<&'route PlacementStep>,
    pub(crate) declared: Option<&'route AuthoredEntryPath>,
    pub(crate) local: Option<AuthoredPose>,
    pub(crate) world: Transform,
}

/// A system root and its Child route across inline and external documents.
#[derive(Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContainerInstanceKey {
    root_entry_index: u32,
    child_node_indices: Vec<u32>,
    external_child_step_indices: Vec<usize>,
}

impl ContainerInstanceKey {
    /// Returns the original system root Entry ordinal.
    #[must_use]
    pub const fn root_entry_index(&self) -> u32 {
        self.root_entry_index
    }

    /// Returns the original document-local Child ordinals in root-to-leaf order.
    #[must_use]
    pub fn child_node_indices(&self) -> &[u32] {
        &self.child_node_indices
    }
    /// Returns positions whose Child is read from the preceding Child's external document.
    #[must_use]
    pub fn external_child_step_indices(&self) -> &[usize] {
        &self.external_child_step_indices
    }

    #[cfg(test)]
    pub(crate) fn matches_route(&self, root: u32, parent: &[u32], child: u32) -> bool {
        self.root_entry_index == root
            && self.child_node_indices.starts_with(parent)
            && self.child_node_indices.last() == Some(&child)
    }

    pub(crate) fn from_route(root_entry_index: u32, route: Option<&PlacementStep>) -> Self {
        let mut child_node_indices = Vec::new();
        let mut external_child_step_indices = Vec::new();
        for step in route.into_iter().flat_map(PlacementStep::steps) {
            if step.via.is_some() {
                external_child_step_indices.push(child_node_indices.len());
            }
            child_node_indices.push(step.child_index());
        }
        child_node_indices.reverse();
        external_child_step_indices.reverse();
        for index in &mut external_child_step_indices {
            *index = child_node_indices.len() - 1 - *index;
        }
        Self {
            root_entry_index,
            child_node_indices,
            external_child_step_indices,
        }
    }
}

/// A resolved `.socpak` occurrence with its original local and composed world transforms.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ContainerInstance {
    /// Original occurrence key.
    pub instance_id: SourcedValue<ContainerInstanceKey>,
    /// Parent occurrence key when this container has a parent.
    pub parent_instance_id: SourcedValue<ContainerInstanceKey>,
    /// Physical container descriptor that supplies this occurrence.
    pub container: SourcedValue<PhysicalEntryRef>,
    /// Canonical system name.
    #[schemars(with = "SourcedValue<String>")]
    pub system: SourcedValue<HealthText>,
    /// Original resolved container descriptor path.
    #[schemars(with = "SourcedValue<String>")]
    pub source_path: SourcedValue<HealthText>,
    /// Path authored by the parent Child declaration.
    #[schemars(with = "SourcedValue<String>")]
    pub declared_path: SourcedValue<HealthText>,
    /// Label authored on the Child declaration.
    #[schemars(extend("x-gameText" = true))]
    pub declared_label: SourcedValue<String>,
    /// Entity name authored on the Child declaration.
    pub declared_entity_name: SourcedValue<String>,
    /// Position relative to the parent occurrence.
    pub local_position: SourcedValue<[f64; 3]>,
    /// Quaternion relative to the parent occurrence.
    pub local_rotation: SourcedValue<[f64; 4]>,
    /// Composed system position.
    pub world_position: SourcedValue<[f64; 3]>,
    /// Composed system orientation.
    pub world_rotation: SourcedValue<[f64; 4]>,
}

#[cfg(test)]
impl ContainerInstance {
    pub(crate) fn source_path_for_test(&self) -> Option<&str> {
        self.source_path.value().map(HealthText::as_str)
    }

    pub(crate) const fn text_cells_for_test(&self) -> [&SourcedValue<HealthText>; 3] {
        [&self.system, &self.source_path, &self.declared_path]
    }
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
            Some(path) => path.retain_in(capture)?.into_cell(sources),
            None => sources.value(SourceValue::Absent, &inputs, complete),
        };
        let parent = self
            .declared
            .map(|_| ContainerInstanceKey::from_route(self.root_entry, self.parent_route));
        let xml_root = file.root.selected()?;
        let transform = self.transform_inputs(&xml_root)?;
        let names = ChildNames::from_route(self.route, &xml_root, sources)?;
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
            system: self.system.clone(),
            source_path: file.path.clone(),
            declared_path,
            declared_label: names.label,
            declared_entity_name: names.entity_name,
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
        let (mut inputs, complete) =
            container_route_uses(self.root_inputs, self.route, self.declared)?;
        if self.declared.is_none() {
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
