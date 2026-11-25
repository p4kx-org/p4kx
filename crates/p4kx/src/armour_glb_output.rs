// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{ArchiveModels, Armour, CharacterAnimations};
use p4kx_extraction::model::MeshOrigin;
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, FrozenGlbMeshOrigins, GlbMeshOrigins,
    GlbOptions, GlbSummary, GltfError, MaterialMode, TextureLoader,
};

use crate::cli_armour::ArmourGlbArgs;
use crate::cli_chf::ChfMaterialsArg;
use crate::file_receipt::FileReceipt;

pub(super) fn write(
    armour: &Armour,
    animated: &CharacterAnimations<'_>,
    args: &ArmourGlbArgs,
    models: &ArchiveModels<'_>,
    (protection, cancel): (&ArchiveWriteProtection, &CancelToken),
) -> Result<GlbSummary> {
    let mode = match args.materials {
        ChfMaterialsArg::Colours => MaterialMode::Colours,
        ChfMaterialsArg::Textures => MaterialMode::Textures,
    };
    let mut options = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        options = options.without_vertex_colours();
    }
    publish(
        &args.out,
        protection,
        cancel,
        &Export {
            armour,
            animated,
            options: &options,
            textures: &models.textures(),
        },
    )
}

struct Export<'a> {
    armour: &'a Armour,
    animated: &'a CharacterAnimations<'a>,
    options: &'a GlbOptions,
    textures: &'a dyn TextureLoader,
}

fn publish(
    out: &Path,
    protection: &ArchiveWriteProtection,
    cancel: &CancelToken,
    export: &Export<'_>,
) -> Result<GlbSummary> {
    let has_mount = export
        .armour
        .parts()
        .iter()
        .any(|part| part.mount_source().is_some());
    let receipt = if has_mount {
        FileReceipt::new_with_cancel(out, Some(protection), Some(protection), cancel.clone())
    } else {
        FileReceipt::for_staged_replacement(out, Some(protection), cancel.clone())
    }?;
    let (summary, _) = receipt
        .publish(
            |writer| export.write(writer),
            |writer, binding, (_, origins)| origins.write_companion(writer, binding),
        )
        .map_err(|error| match error.downcast::<GltfError>() {
            Ok(source) => ConversionError::Glb {
                file: export.armour.variant().code().to_owned(),
                source,
            },
            Err(source) => ConversionError::Write {
                path: out.to_path_buf(),
                source,
            },
        })?;
    Ok(summary)
}

struct EmittedMesh {
    accessor: usize,
    source: Option<MeshOrigin>,
    part: usize,
}

impl Export<'_> {
    fn write(
        &self,
        mut writer: &mut dyn io::Write,
    ) -> io::Result<(GlbSummary, FrozenGlbMeshOrigins)> {
        let mut meshes = Vec::new();
        let (summary, emission) = self
            .armour
            .write_glb_animated_observed_with_vertex_origins(
                self.animated,
                self.options,
                self.textures,
                &mut writer,
                &mut |mesh, _| {
                    meshes.push(EmittedMesh {
                        accessor: mesh.position_accessor,
                        source: mesh.origin,
                        part: mesh.part,
                    });
                    Ok(())
                },
            )
            .map_err(|error| match error {
                GltfError::Write(source) => source,
                source => io::Error::other(source),
            })?;
        let mut origins = GlbMeshOrigins::with_expanded_use_refs();
        // Worn fields register first so their original reference encoding is stable.
        let mounts = self
            .armour
            .worn_mount_fields(&emission, origins.source_uses())
            .map_err(io::Error::other)?;
        origins.include_json_fields(mounts);
        for mesh in meshes {
            let part = self
                .armour
                .parts()
                .get(mesh.part)
                .ok_or_else(|| io::Error::other("rigged mesh lost its producing part"))?;
            origins.observe_mesh(mesh.accessor, mesh.source, part.model_archive_sources())?;
        }
        Ok((summary, origins.finish()?))
    }
}
