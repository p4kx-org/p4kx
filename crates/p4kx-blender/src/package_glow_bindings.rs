// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{GlowSubset, Part};
use p4kx_extraction::{
    InputScope, ModelArchiveSource, SourceInventory, SourceLocation, SourceScope, SourceUseRole,
    SourceUses, SourceValue, SourcedValue,
};
use p4kx_model::ModelFile;
use schemars::JsonSchema;
use serde::Serialize;

use crate::{Manifest, Mesh, PackageResult};

use super::error;

#[derive(Serialize, JsonSchema)]
pub(super) struct DrawBinding {
    lod: u32,
    model_node: usize,
    source_node_id: u16,
    subset: usize,
    first_index: usize,
    index_count: usize,
    node_name: SourcedValue<String>,
    material: Surface,
}

#[derive(Serialize, JsonSchema)]
struct Surface {
    source_slot: u16,
    resolves: bool,
    shader: SourcedValue<String>,
    no_draw: SourcedValue<bool>,
    packaged_material: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct CandidateNode {
    node: usize,
    name: SourcedValue<String>,
}

pub(super) struct Evidence<'a> {
    part: &'a Part,
    mesh: Option<&'a Mesh>,
    uses: SourceUses,
    primary: Option<SourceScope>,
    node_sources: Option<&'a p4kx_model::ModelNodeSources>,
    material: Option<SourceScope>,
    primary_complete: bool,
}

impl<'a> Evidence<'a> {
    pub(super) fn of(
        part: &'a Part,
        model_sources: &'a [ModelArchiveSource],
        manifest: &'a Manifest,
        node: usize,
    ) -> PackageResult<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let model = model_sources
            .iter()
            .find(|source| source.file() == ModelFile::Model);
        let primary = model.and_then(ModelArchiveSource::origin);
        let material = part.material_archive_origins().library();
        let mesh = manifest
            .nodes
            .get(node)
            .and_then(|row| row.mesh.as_ref())
            .and_then(|id| manifest.meshes.iter().find(|mesh| &mesh.id == id));
        Ok(Self {
            part,
            mesh,
            primary: scope(&uses, primary)?,
            node_sources: model.and_then(ModelArchiveSource::node_sources),
            material: scope(&uses, material)?,
            primary_complete: primary
                .is_some_and(|entry| entry.ancestors().all(|step| step.is_some())),
            uses,
        })
    }

    pub(super) fn finish(self) -> PackageResult<SourceInventory> {
        self.uses.freeze().map_err(error)
    }

    pub(super) fn bind(&self, row: GlowSubset) -> PackageResult<DrawBinding> {
        let name = self.node_name(row.node)?;
        Ok(DrawBinding {
            lod: row.lod,
            model_node: row.node,
            source_node_id: row.source_node_id,
            subset: row.subset,
            first_index: row.indices.start,
            index_count: row.indices.len(),
            node_name: name,
            material: self.surface(row.material)?,
        })
    }

    pub(super) fn candidate(&self, node: usize) -> PackageResult<CandidateNode> {
        Ok(CandidateNode {
            node,
            name: self.node_name(node)?,
        })
    }

    fn node_name(&self, node: usize) -> PackageResult<SourcedValue<String>> {
        let value = self
            .part
            .model()
            .and_then(|model| model.nodes.get(node))
            .map_or(SourceValue::Absent, |node| {
                SourceValue::Present(node.name.clone())
            });
        let source = self
            .node_sources
            .and_then(|sources| sources.node(node))
            .and_then(|source| Some((source.chunk(), source.name()?)));
        let Some((scope, (chunk, span))) = self.primary.as_ref().zip(source) else {
            return Ok(self.uses.value(value, &[], false));
        };
        let used = scope
            .at(
                SourceLocation::Subresource {
                    namespace: "modelNode".to_owned(),
                    indices: vec![chunk as u64, node as u64, span.start, span.end],
                    field: "name".to_owned(),
                },
                SourceUseRole::Selector,
            )
            .map_err(error)?;
        Ok(self.uses.value(value, &[used], self.primary_complete))
    }

    fn material_value<T>(
        &self,
        value: Option<T>,
        source_slot: u16,
        fields: &[&str],
    ) -> PackageResult<SourcedValue<T>> {
        let value = value.map_or(SourceValue::Absent, SourceValue::Present);
        let Some(scope) = &self.material else {
            return Ok(self.uses.value(value, &[], false));
        };
        let Some(library) = self.part.materials().library() else {
            return Ok(self.uses.value(value, &[], false));
        };
        if library.is_empty_slot(source_slot) || library.material(source_slot).is_none() {
            return Ok(self.uses.value(value, &[], false));
        }
        let slot = if library.single {
            Vec::new()
        } else {
            vec![u64::from(source_slot)]
        };
        let mut uses = Vec::with_capacity(fields.len());
        for field in fields {
            uses.push(
                scope
                    .at(
                        SourceLocation::Subresource {
                            namespace: "material.xml".to_owned(),
                            indices: slot.clone(),
                            field: (*field).to_owned(),
                        },
                        SourceUseRole::Payload,
                    )
                    .map_err(error)?,
            );
        }
        Ok(self.uses.value(value, &uses, false))
    }

    fn surface(&self, row: p4kx_assembly::GlowSubsetMaterial) -> PackageResult<Surface> {
        let source_slot = row.source_slot;
        let packaged_material = self
            .mesh
            .and_then(|mesh| {
                mesh.materials
                    .iter()
                    .find(|entry| entry.source_material_ids.contains(&source_slot))
            })
            .map(|entry| entry.index);
        Ok(Surface {
            source_slot,
            resolves: row.resolves,
            shader: self.material_value(row.shader, source_slot, &["Shader"])?,
            no_draw: self.material_value(row.no_draw, source_slot, &["MtlFlags", "Shader"])?,
            packaged_material,
        })
    }
}

fn scope(
    uses: &SourceUses,
    entry: Option<&p4kx_extraction::EntryOrigin>,
) -> PackageResult<Option<SourceScope>> {
    entry
        .map(|entry| {
            uses.observe(
                p4kx_extraction::SourceInput::Archive(entry),
                InputScope::Payload,
            )
            .map_err(error)
        })
        .transpose()
}
