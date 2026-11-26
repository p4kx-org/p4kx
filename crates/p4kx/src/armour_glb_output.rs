// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{ArchiveModels, Armour, CharacterAnimations};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, FrozenGlbMeshOrigins, GlbOptions,
    GlbSummary, GltfError, MaterialMode, TextureLoader,
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

impl Export<'_> {
    fn write(
        &self,
        mut writer: &mut dyn io::Write,
    ) -> io::Result<(GlbSummary, FrozenGlbMeshOrigins)> {
        self.armour
            .write_glb_with_origins(self.animated, self.options, self.textures, &mut writer)
            .map_err(|error| match error {
                GltfError::Write(source) => source,
                source => io::Error::other(source),
            })
    }
}
