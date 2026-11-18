// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::ChildContainerPath;
use p4kx_extraction::{
    Archive, Guid, SharedFieldOrigin, SourceUse, SourceUseRole, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use super::instance::{ContainerFile, ContainerInstanceKey};
use super::proof::{PlacementStep, container_route_uses};
use crate::catalog_read::original::{
    AuthoredEntryPath, DeferredXmlNode, EntryCapture, EntryXml, PhysicalEntryRef,
    SourcedXmlAttribute,
};
use crate::entry_source::EntrySource;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap;
use crate::starmap_index::Index;
use crate::starmap_transform::Transform;
use crate::starmap_xml::{self, Root};
use crate::{CatalogError, ExportAccountingError, HealthText};

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum ContainerTargetStatus {
    Resolved,
    NotFound,
    Ambiguous,
    UnsupportedSelector,
    IncompleteDirectory,
}

/// One admitted external Child without a usable uniquely resolved target body.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct UnresolvedContainerLink {
    instance_id: SourcedValue<ContainerInstanceKey>,
    parent_instance_id: SourcedValue<ContainerInstanceKey>,
    declaring_container: SourcedValue<PhysicalEntryRef>,
    #[serde(serialize_with = "UnresolvedChild::serialize_shared")]
    #[schemars(with = "UnresolvedChild")]
    declaration: Arc<UnresolvedChild>,
    world_position: SourcedValue<[f64; 3]>,
    world_rotation: SourcedValue<[f64; 4]>,
    target_status: SourcedValue<ContainerTargetStatus>,
    target_read_status: SourcedValue<ContainerTargetReadStatus>,
    target_candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
}

/// Source attributes shared by every placement of a physical Child.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct UnresolvedChild {
    descriptor: SourcedValue<PhysicalEntryRef>,
    child_node_index: SourcedValue<u32>,
    #[schemars(with = "SourcedValue<String>")]
    declared_path: SourcedValue<HealthText>,
    starmap_record_guid: SourcedValue<CatalogGuid>,
    local_position: SourcedValue<[f64; 3]>,
    local_rotation: SourcedValue<[f64; 4]>,
    attributes: Vec<SourcedXmlAttribute>,
    #[serde(skip)]
    #[schemars(skip)]
    directory: Option<SourceUse>,
}

/// The actual consulted target body's availability after directory selection.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum ContainerTargetReadStatus {
    NotAttempted,
    Unreadable,
    Unparsed,
}

/// Original Child values awaiting the actual unique target body's classification.
pub(crate) struct DeferredChild {
    node: DeferredXmlNode,
    directory: Option<SourceUse>,
}

impl DeferredChild {
    pub(crate) fn capture(
        xml: &EntryXml<'_, '_, '_>,
        node: Root<'_>,
        directory: Option<SourceUse>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            node: xml.deferred_node(node)?,
            directory,
        })
    }

    pub(crate) fn retain(
        self,
        path: &AuthoredEntryPath,
        capture: &EntryCapture<'_>,
    ) -> Result<Arc<UnresolvedChild>, CatalogError> {
        UnresolvedChild::from_deferred(&self.node, path, self.directory, capture)
    }
}

impl UnresolvedChild {
    pub(crate) fn capture(
        xml: &EntryXml<'_, '_, '_>,
        node: Root<'_>,
        path: &AuthoredEntryPath,
        directory: Option<SourceUse>,
    ) -> Result<Arc<Self>, CatalogError> {
        Self::from_deferred(&xml.deferred_node(node)?, path, directory, xml.capture())
    }

    fn from_deferred(
        node: &DeferredXmlNode,
        path: &AuthoredEntryPath,
        directory: Option<SourceUse>,
        capture: &EntryCapture<'_>,
    ) -> Result<Arc<Self>, CatalogError> {
        let sources = capture.source_uses();
        let descriptor = node.descriptor(capture)?.into_cell(sources);
        let child_source = node.source();
        Ok(Arc::new(Self {
            descriptor,
            child_node_index: sources.value(
                SourceValue::Present(child_source.index()),
                &[child_source.selected()?],
                node.is_complete(),
            ),
            declared_path: path.retain_in(capture)?.into_cell(sources),
            starmap_record_guid: node.attribute(capture, "starMapRecord", |text| {
                text.trim().parse::<Guid>().ok().map(CatalogGuid)
            })?,
            local_position: node.attribute(capture, "pos", starmap_xml::child_position)?,
            local_rotation: node.attribute(capture, "rot", |text| {
                starmap_xml::child_rotation(text).map(|(parts, _)| parts)
            })?,
            attributes: node.attributes(capture)?,
            directory,
        }))
    }

    fn serialize_shared<S: Serializer>(
        value: &Arc<Self>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().serialize(serializer)
    }
}

pub(crate) struct UnresolvedOccurrence<'route> {
    pub(crate) root_entry: u32,
    pub(crate) root_inputs: &'route [SourceUse],
    pub(crate) route: &'route PlacementStep,
    pub(crate) parent_route: Option<&'route PlacementStep>,
    pub(crate) selector: &'route AuthoredEntryPath,
    pub(crate) declaration: &'route Arc<UnresolvedChild>,
    pub(crate) world: Option<Transform>,
    pub(crate) read_status: ContainerTargetReadStatus,
}

impl UnresolvedOccurrence<'_> {
    pub(crate) fn project(
        &self,
        archive: &Archive,
        index: &Index<'_>,
        file: &ContainerFile,
        capture: &EntryCapture<'_>,
    ) -> Result<UnresolvedContainerLink, CatalogError> {
        let targets = self.targets(archive, index, capture)?;
        let sources = capture.source_uses();
        let (inputs, complete) =
            container_route_uses(self.root_inputs, Some(self.route), Some(self.selector))?;
        let shared = sources.shared_origin(&inputs, complete);
        let route_key = |route| ContainerInstanceKey::from_route(self.root_entry, route);
        let rotation = self
            .route
            .steps()
            .map(|step| step.rotation(SourceUseRole::Conversion))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(UnresolvedContainerLink {
            instance_id: sources.value_with_shared_origin(
                SourceValue::Present(route_key(Some(self.route))),
                &shared,
                &[],
                true,
            )?,
            parent_instance_id: sources.value_with_shared_origin(
                SourceValue::Present(route_key(self.parent_route)),
                &shared,
                &[],
                true,
            )?,
            declaring_container: file.reference().clone(),
            declaration: Arc::clone(self.declaration),
            world_position: sources.value_with_shared_origin(
                world_value(self.world, |pose| pose.position.map(starmap::millimetres)),
                &shared,
                &self.route.position_uses()?,
                true,
            )?,
            world_rotation: sources.value_with_shared_origin(
                world_value(self.world, |pose| pose.rotation.parts()),
                &shared,
                &rotation,
                true,
            )?,
            target_status: targets.status,
            target_read_status: targets.read_status,
            target_candidates: targets.candidates,
        })
    }

    fn targets(
        &self,
        archive: &Archive,
        index: &Index<'_>,
        capture: &EntryCapture<'_>,
    ) -> Result<Targets, CatalogError> {
        let directory_complete = self.declaration.directory.is_some()
            && crate::export_archive_health::inventory(archive).is_none();
        let path = self.selector.as_str();
        let entries = index.candidates(path);
        let status = match entries {
            [] | [_] if !directory_complete => ContainerTargetStatus::IncompleteDirectory,
            [] if ChildContainerPath::new(path).is_none() => {
                ContainerTargetStatus::UnsupportedSelector
            }
            [] => ContainerTargetStatus::NotFound,
            [_] => ContainerTargetStatus::Resolved,
            _ => ContainerTargetStatus::Ambiguous,
        };
        let sources = capture.source_uses();
        let (mut inputs, complete) = self.selector.selector_inputs()?;
        inputs.extend(self.declaration.directory.iter().cloned());
        let shared = sources.shared_origin(&inputs, complete);
        let mut read_inputs = inputs.clone();
        let candidates =
            target_candidates(archive, index, capture, entries, &shared, &mut read_inputs)?;
        let lookup_complete = complete
            && directory_complete
            && !matches!(status, ContainerTargetStatus::UnsupportedSelector);
        Ok(Targets {
            read_status: sources.value(
                SourceValue::Present(self.read_status),
                &read_inputs,
                lookup_complete,
            ),
            status: sources.value(SourceValue::Present(status), &inputs, lookup_complete),
            candidates: sources.value(SourceValue::Present(candidates), &inputs, lookup_complete),
        })
    }
}

fn target_candidates(
    archive: &Archive,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    entries: &[usize],
    shared: &SharedFieldOrigin,
    read_inputs: &mut Vec<SourceUse>,
) -> Result<Vec<SourcedValue<PhysicalEntryRef>>, CatalogError> {
    let sources = capture.source_uses();
    entries
        .iter()
        .map(|&position| {
            let entry = index
                .entries
                .get(position)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let receipt = EntrySource::outer(capture, archive, entry)?;
            let original = receipt.receipt().physical_reference(capture)?;
            let (uses, known) = original.contributions();
            read_inputs.extend_from_slice(uses);
            Ok(sources.value_with_shared_origin(original.value().clone(), shared, uses, known)?)
        })
        .collect()
}

struct Targets {
    read_status: SourcedValue<ContainerTargetReadStatus>,
    status: SourcedValue<ContainerTargetStatus>,
    candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
}

fn world_value<T>(world: Option<Transform>, value: impl FnOnce(Transform) -> T) -> SourceValue<T> {
    world.map_or(SourceValue::Invalid, |world| {
        SourceValue::Present(value(world))
    })
}
