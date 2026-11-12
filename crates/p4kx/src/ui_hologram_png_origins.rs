// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io;
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_assembly::{Assembly, AttachedModel, Part};
use p4kx_extraction::model::{Model, ModelFile};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, EntryOrigin, FieldOrigin, FileReceipt,
    ModelArchiveSource, OriginArtifactBinding, OriginOutputAddress, SourceLocation, SourceUse,
    SourceUseRole, write_origin_companion,
};
use p4kx_render::{
    AssemblyPieceSource, AssemblyScene, MaterialSuppressionEffect, MaterialSuppressionReason,
    Picture, PlacedPieceInput,
};

use super::material_origins::{self, MaterialInputs};
use super::origins::CameraInput;

use super::{HologramInputs, Projection};
use super::{bone_origins, cdf_origins, gear_origins, node_origins};
use crate::json;

pub(super) struct Published {
    pub(super) artifact: FieldOrigin,
    pub(super) scene: FieldOrigin,
    pub(super) bytes: u64,
    pub(super) receipt: String,
}

pub(super) fn publish(
    picture: &Picture,
    output: &Path,
    (protection, cancel): (&ArchiveWriteProtection, &CancelToken),
    original: &HologramInputs<'_>,
    (assembly, scene, visible): (&Assembly, &AssemblyScene, &[usize]),
) -> Result<Published> {
    let uses = original.projection.uses();
    let SceneInputs {
        placed: mut sources,
        suppressed_ranges,
        drawn_geometry,
    } = scene_inputs(original.projection, assembly, scene, visible)?;
    let scene = uses.origin(&sources, false);
    sources.extend(suppressed_ranges);
    sources.extend(drawn_geometry);
    sources.extend(
        original
            .settings
            .iter()
            .flat_map(CameraInput::sources)
            .cloned(),
    );
    let artifact = uses.origin(&sources, false);
    let inventory = uses.freeze()?;
    let receipt =
        FileReceipt::new_with_cancel(output, Some(protection), Some(protection), cancel.clone())
            .map_err(|source| write_error(output, source))?;
    let name = receipt
        .path()
        .file_name()
        .context("missing hologram receipt name")?;
    let mut byte_length = None;
    receipt
        .publish(
            |writer| picture.write_png(writer).map_err(io::Error::other),
            |writer, binding, ()| {
                if let OriginArtifactBinding::File { digest, .. } = binding {
                    byte_length = Some(digest.byte_length());
                }
                write_origin_companion(
                    writer,
                    binding,
                    &inventory,
                    [(&OriginOutputAddress::OpaquePayload, &artifact)],
                )
            },
        )
        .map_err(|source| write_error(output, source))?;
    Ok(Published {
        artifact,
        scene,
        bytes: byte_length.context("missing hologram artifact length")?,
        receipt: json::path(Path::new(name)),
    })
}

fn write_error(output: &Path, source: io::Error) -> ConversionError {
    ConversionError::Write {
        path: output.to_path_buf(),
        source,
    }
}

struct SceneInputs {
    placed: Vec<SourceUse>,
    suppressed_ranges: Vec<SourceUse>,
    drawn_geometry: Vec<SourceUse>,
}

struct PieceInputs<'a> {
    model: Option<&'a Model>,
    models: &'a [ModelArchiveSource],
    definition: Option<&'a EntryOrigin>,
    material: MaterialInputs<'a>,
    ancestry: usize,
}

impl PieceInputs<'_> {
    fn project(
        &self,
        projection: &Projection,
        assembly: &Assembly,
        piece: &PlacedPieceInput<'_>,
        drawn: bool,
        sources: &mut SceneInputs,
    ) -> Result<()> {
        let role = if drawn {
            SourceUseRole::Payload
        } else {
            SourceUseRole::Conversion
        };
        projection.authored_selector(
            self.material.owner.and_then(Part::geometry_selector),
            &mut sources.placed,
        )?;
        model_inputs(
            projection,
            self.models,
            self.definition,
            role,
            &mut sources.placed,
        )?;
        if let Some(owner) = self.material.owner {
            cdf_origins::model(projection, owner, &mut sources.placed)?;
        }
        self.material
            .selected(projection, piece.material_slots(), &mut sources.placed)?;
        self.mesh_nodes(projection, piece, drawn, sources)?;
        self.gear_pose(projection, piece, drawn, sources)?;
        if let (Some(owner), Some((ordinal, _))) = (
            assembly.parts().get(self.ancestry),
            self.material.attachment,
        ) {
            cdf_origins::attachment(projection, owner, ordinal, &mut sources.placed)?;
        }
        Ok(())
    }

    fn gear_pose(
        &self,
        projection: &Projection,
        piece: &PlacedPieceInput<'_>,
        drawn: bool,
        sources: &mut SceneInputs,
    ) -> Result<()> {
        let Some(part) = self.material.owner else {
            return Ok(());
        };
        gear_origins::pose(
            projection,
            part,
            piece.bound_gear_keys(),
            &mut sources.placed,
        )?;
        if drawn {
            gear_origins::pose(
                projection,
                part,
                piece.drawn_gear_keys(),
                &mut sources.drawn_geometry,
            )?;
        }
        Ok(())
    }

    fn mesh_nodes(
        &self,
        projection: &Projection,
        piece: &PlacedPieceInput<'_>,
        drawn: bool,
        sources: &mut SceneInputs,
    ) -> Result<()> {
        node_origins::mesh(
            projection,
            self.model,
            self.models,
            piece.bound_mesh_nodes(),
            &mut sources.placed,
        )?;
        if drawn {
            node_origins::mesh(
                projection,
                self.model,
                self.models,
                piece.drawn_mesh_nodes(),
                &mut sources.drawn_geometry,
            )?;
        }
        Ok(())
    }
}

fn piece_inputs<'a>(
    assembly: &'a Assembly,
    attachments: &[(usize, usize, &'a AttachedModel)],
    piece: AssemblyPieceSource,
) -> Option<PieceInputs<'a>> {
    match piece {
        AssemblyPieceSource::Part(index) => {
            let part = assembly.parts().get(index)?;
            Some(PieceInputs {
                model: part.model(),
                models: part.model_archive_sources(),
                definition: part.model_definition_origin(),
                material: MaterialInputs {
                    archive: part.material_archive_origins(),
                    owner: Some(part),
                    attachment: None,
                },
                ancestry: index,
            })
        }
        AssemblyPieceSource::BoneAttachment(index) => {
            let &(parent, ordinal, attachment) = attachments.get(index)?;
            Some(PieceInputs {
                model: Some(attachment.model()),
                models: attachment.model_archive_sources(),
                definition: attachment.model_definition_origin(),
                material: MaterialInputs {
                    archive: attachment.material_archive_origins(),
                    owner: None,
                    attachment: Some((ordinal, attachment)),
                },
                ancestry: parent,
            })
        }
        _ => None,
    }
}

fn scene_inputs(
    projection: &Projection,
    assembly: &Assembly,
    scene: &AssemblyScene,
    visible: &[usize],
) -> Result<SceneInputs> {
    let mut sources = SceneInputs {
        placed: Vec::new(),
        suppressed_ranges: Vec::new(),
        drawn_geometry: Vec::new(),
    };
    let attachments: Vec<_> = assembly.indexed_attachments().collect();
    let mut placed_ancestry = HashSet::new();
    for piece in scene.placed_inputs() {
        let drawn = visible.binary_search(&piece.ordinal()).is_ok();
        let Some(input) = piece_inputs(assembly, &attachments, piece.source()) else {
            continue;
        };
        input.project(projection, assembly, &piece, drawn, &mut sources)?;
        let mut at = Some(input.ancestry);
        while let Some(index) = at.filter(|index| placed_ancestry.insert(*index)) {
            let Some(part) = assembly.parts().get(index) else {
                break;
            };
            port_placement(projection, part, &mut sources.placed)?;
            gear_origins::placement(projection, part, &mut sources.placed)?;
            node_origins::placement(projection, assembly, index, &mut sources.placed)?;
            bone_origins::placement(projection, assembly, index, &mut sources.placed)?;
            at = part.parent();
        }
    }
    suppressed_materials(
        projection,
        assembly,
        &attachments,
        scene,
        visible,
        &mut sources,
    )?;
    Ok(sources)
}

fn suppressed_materials(
    projection: &Projection,
    assembly: &Assembly,
    attachments: &[(usize, usize, &AttachedModel)],
    scene: &AssemblyScene,
    visible: &[usize],
    sources: &mut SceneInputs,
) -> Result<()> {
    for suppression in scene.material_suppressions() {
        let uses = match suppression.effect() {
            MaterialSuppressionEffect::WholePiece => &mut sources.placed,
            MaterialSuppressionEffect::DrawRange
                if visible.binary_search(&suppression.ordinal()).is_ok() =>
            {
                &mut sources.suppressed_ranges
            }
            _ => continue,
        };
        let field = match suppression.reason() {
            MaterialSuppressionReason::Hidden => "hiddenMaterial",
            MaterialSuppressionReason::Decal => "decalMaterial",
            _ => continue,
        };
        let Some(input) = piece_inputs(assembly, attachments, suppression.source()) else {
            continue;
        };
        if let Some(scope) = input.material.scope(projection, uses)? {
            if suppression.effect() == MaterialSuppressionEffect::WholePiece {
                input.material.whole_piece(projection, uses)?;
            }
            uses.push(material_origins::material_slot(
                &scope,
                suppression.slot(),
                field,
            )?);
        }
    }
    Ok(())
}

fn model_inputs(
    projection: &Projection,
    models: &[ModelArchiveSource],
    definition: Option<&EntryOrigin>,
    role: SourceUseRole,
    sources: &mut Vec<SourceUse>,
) -> Result<()> {
    for (origin, role) in models
        .iter()
        .filter(|source| !matches!(source.file(), ModelFile::LodModel(_)))
        .filter_map(|source| Some((source.origin()?, role)))
        .chain(definition.map(|source| (source, SourceUseRole::Selector)))
    {
        sources.push(projection.opaque_archive(origin, role)?);
    }
    Ok(())
}

fn port_placement(
    projection: &Projection,
    part: &Part,
    sources: &mut Vec<SourceUse>,
) -> Result<()> {
    if let Some(placement) = part.port_placement_source() {
        let position = placement.port();
        for field in placement.fields() {
            sources.push(projection.scope().at(
                SourceLocation::DataForge {
                    record_index: position.record_index(),
                    field: format!(
                        "Components[{}].Ports[{}].{}",
                        position.component_ordinal(),
                        position.port_ordinal(),
                        field.name(),
                    ),
                },
                SourceUseRole::Conversion,
            )?);
        }
    }
    Ok(())
}
