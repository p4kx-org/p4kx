// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io;
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_assembly::{Assembly, Part};
use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, EntryOrigin, FieldOrigin, FileReceipt,
    InputScope, MaterialArchiveOrigins, ModelArchiveSource, OriginArtifactBinding,
    OriginOutputAddress, SourceCoordinateOmission, SourceInput, SourceLocation, SourceUse,
    SourceUseRole, write_origin_companion,
};
use p4kx_render::{AssemblyPieceSource, AssemblyScene, Picture};

use super::origins::CameraInput;
use super::{HologramInputs, Projection};
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
    let mut sources = scene_inputs(original.projection, assembly, scene, visible)?;
    let scene = uses.origin(&sources, false);
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

fn scene_inputs(
    projection: &Projection,
    assembly: &Assembly,
    scene: &AssemblyScene,
    visible: &[usize],
) -> Result<Vec<SourceUse>> {
    let mut sources = Vec::new();
    let attachments: Vec<_> = assembly.attached_models().collect();
    let mut placed_ancestry = HashSet::new();
    for piece in scene.placed_inputs() {
        let role = if visible.binary_search(&piece.ordinal()).is_ok() {
            SourceUseRole::Payload
        } else {
            SourceUseRole::Conversion
        };
        let (models, definition, materials, material_owner, index) = match piece.source() {
            AssemblyPieceSource::Part(index) => {
                let Some(part) = assembly.parts().get(index) else {
                    continue;
                };
                authored_selector(projection, part.geometry_selector(), &mut sources)?;
                (
                    part.model_archive_sources(),
                    part.model_definition_origin(),
                    part.material_archive_origins(),
                    Some(part),
                    index,
                )
            }
            AssemblyPieceSource::BoneAttachment(index) => {
                let Some(&(parent, attachment)) = attachments.get(index) else {
                    continue;
                };
                (
                    attachment.model_archive_sources(),
                    attachment.model_definition_origin(),
                    attachment.material_archive_origins(),
                    None,
                    parent,
                )
            }
            _ => continue,
        };
        model_inputs(projection, models, definition, role, &mut sources)?;
        selected_materials(
            projection,
            materials,
            material_owner,
            piece.material_slots(),
            &mut sources,
        )?;
        let mut at = Some(index);
        while let Some(index) = at.filter(|index| placed_ancestry.insert(*index)) {
            let Some(part) = assembly.parts().get(index) else {
                break;
            };
            port_placement(projection, part, &mut sources)?;
            at = part.parent();
        }
    }
    Ok(sources)
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
        let scope = projection
            .uses()
            .observe(SourceInput::Archive(origin), InputScope::Payload)?;
        sources.push(scope.at_opaque(role, SourceCoordinateOmission::Unavailable)?);
    }
    Ok(())
}

fn selected_materials(
    projection: &Projection,
    materials: &MaterialArchiveOrigins,
    material_owner: Option<&Part>,
    slots: impl Iterator<Item = usize>,
    sources: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(origin) = materials.library() else {
        return Ok(());
    };
    let mut slots = slots.peekable();
    if slots.peek().is_none() {
        return Ok(());
    }
    authored_selector(
        projection,
        material_owner.and_then(Part::material_selector),
        sources,
    )?;
    let scope = projection
        .uses()
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    for slot in slots {
        sources.push(scope.at(
            SourceLocation::Subresource {
                namespace: "materialLibrary".into(),
                indices: vec![u64::try_from(slot)?],
                field: "selectedMaterial".into(),
            },
            SourceUseRole::Selector,
        )?);
    }
    Ok(())
}

fn authored_selector(
    projection: &Projection,
    selector: Option<SourceLocation>,
    sources: &mut Vec<SourceUse>,
) -> Result<()> {
    if let Some(selector) = selector {
        sources.push(projection.scope().at(selector, SourceUseRole::Selector)?);
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
