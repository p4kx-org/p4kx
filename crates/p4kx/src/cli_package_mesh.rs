// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use clap::{Args, ValueEnum};
use p4kx_blender::PackageTextureContainer;
use p4kx_extraction::GlbOptions;

use crate::cli_model::lod_range;

/// Mesh and texture arguments shared by the entity and interior package commands.
#[derive(Debug, Args)]
pub(crate) struct PackageMeshArgs {
    /// The level of detail of every model: 0 is the most detailed. A model that lacks the level
    /// is written at the nearest lower level it has.
    #[arg(long, value_name = "N", default_value_t = 0, value_parser = lod_range())]
    pub(crate) lod: u32,
    /// The mip level of the textures: 0 is the largest, each level halves the width and height;
    /// a texture with fewer mips gives its smallest.
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub(crate) texture_mip: u32,
    /// Writes supported DDS mips unchanged, with PNG fallback for other textures.
    #[arg(long, value_enum, value_name = "CONTAINER", default_value_t)]
    texture_container: PackageTextureContainerArg,
    /// Leaves the vertex colours out of the parts.
    #[arg(long)]
    no_vertex_colours: bool,
    /// Writes source vertex maps for the package's meshes.
    #[arg(long)]
    pub(crate) vertex_maps: bool,
}

impl PackageMeshArgs {
    pub(crate) fn glb_options(&self) -> GlbOptions {
        let options = GlbOptions::default()
            .with_lod(self.lod)
            .with_texture_mip(self.texture_mip);
        if self.no_vertex_colours {
            options.without_vertex_colours()
        } else {
            options
        }
    }

    pub(crate) const fn texture_container(&self) -> PackageTextureContainer {
        self.texture_container.container()
    }
}

/// An external texture container supported by the Blender package importer.
#[derive(Debug, Default, Clone, Copy, ValueEnum)]
enum PackageTextureContainerArg {
    /// Converts textures to PNG.
    #[default]
    Png,
    /// Uses verified DDS blocks for compatible material textures.
    Dds,
}

impl PackageTextureContainerArg {
    const fn container(self) -> PackageTextureContainer {
        match self {
            Self::Png => PackageTextureContainer::Png,
            Self::Dds => PackageTextureContainer::BlenderDds,
        }
    }
}
