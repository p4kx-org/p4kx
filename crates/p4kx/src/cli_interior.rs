// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};

use crate::cli_model::lod_range;
use crate::exit::EXIT_CODES_HELP;

/// The material modes of `p4kx interior glb --materials`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum InteriorMaterialsArg {
    /// Plain colours without textures.
    Colours,
}

/// Runs the action of `p4kx interior`.
pub(crate) fn dispatch(action: InteriorAction, as_json: bool) -> Result<()> {
    match action {
        InteriorAction::Glb(args) => crate::interior_glb::run(&args, as_json),
        InteriorAction::Package(args) => crate::interior_package::run(&args, as_json),
        InteriorAction::Source(args) => crate::interior_source::run(&args),
    }
}

/// Arguments of `p4kx interior`.
#[derive(Debug, Args)]
pub(crate) struct InteriorArgs {
    /// What to do with the Interior.
    #[command(subcommand)]
    pub(crate) action: InteriorAction,
}

/// The actions of `p4kx interior`.
#[derive(Debug, Subcommand)]
pub(crate) enum InteriorAction {
    /// Writes the Interior of a socpak container as one glTF 2.0 binary file (GLB): a node for
    /// the container, one below it for each `.soc` file it streams with a node per placed model,
    /// prop (an entity with geometry of its own) and light at its transform, and one for each
    /// container nested in it at its position and rotation, recursively. Point, spot and
    /// directional lights are written with the glTF lights extension; ambient and planar lights are
    /// skipped and listed. Y up, in metres. Containers nested in themselves are not entered again;
    /// containers, files and models that cannot be resolved are skipped and listed. Models are
    /// looked for inside the container first, then in the Archive. Reads every model once, in
    /// parallel.
    #[command(after_help = EXIT_CODES_HELP)]
    Glb(InteriorGlbArgs),
    /// Writes the Interior of a socpak container as a Blender package: a folder with `scene.json`,
    /// one GLB per distinct model and one PNG per distinct texture, the same nodes, props and
    /// lights as `entity package --interior` writes for the container, without an Entity around
    /// them. The container is the root node; containers, files and models that cannot be resolved
    /// are skipped and listed in the manifest. Models are looked for inside the container first,
    /// then in the Archive. Textures that exist only inside a container are listed as failed. The
    /// folder must not exist, or be empty, or hold a package an earlier run wrote, which is
    /// replaced.
    #[command(after_help = EXIT_CODES_HELP)]
    Package(InteriorPackageArgs),
    /// Writes the authored room, door, link and loadout sources as versioned JSON.
    #[command(after_help = EXIT_CODES_HELP)]
    Source(InteriorSourceArgs),
}

/// Arguments of `p4kx interior glb`.
#[derive(Debug, Args)]
pub(crate) struct InteriorGlbArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Path of the container Entry in the Archive, with `\` or `/` as separator, ignoring case;
    /// each further path names a container inside the one before, as for `p4kx socpak list`.
    #[arg(required = true, value_name = "CONTAINER")]
    pub(crate) containers: Vec<String>,
    /// The GLB file to write; an existing file is replaced.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: PathBuf,
    /// The level of detail of every model: 0 is the most detailed. A model that lacks the level
    /// is written at the nearest lower level it has.
    #[arg(long, value_name = "N", default_value_t = 0, value_parser = lod_range())]
    pub(crate) lod: u32,
    /// How to write the materials. colours: one glTF material per distinct material, with base
    /// colour, opacity, roughness, metalness and emissive colour and no textures.
    #[arg(long, value_enum, value_name = "MODE", default_value_t = InteriorMaterialsArg::Colours)]
    pub(crate) materials: InteriorMaterialsArg,
    /// Leaves the vertex colours out; viewers multiply the material's base colour by them.
    #[arg(long)]
    pub(crate) no_vertex_colours: bool,
}

/// Arguments of `p4kx interior package`.
#[derive(Debug, Args)]
pub(crate) struct InteriorPackageArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Path of the container Entry in the Archive, with `\` or `/` as separator, ignoring case;
    /// each further path names a container inside the one before, as for `p4kx socpak list`.
    #[arg(required = true, value_name = "CONTAINER")]
    pub(crate) containers: Vec<String>,
    /// The folder to write the package to; it must not exist, or be empty, or hold a package an
    /// earlier run wrote, which is replaced.
    #[arg(long, value_name = "FOLDER")]
    pub(crate) out: PathBuf,
    /// The level of detail of every model: 0 is the most detailed. A model that lacks the level
    /// is written at the nearest lower level it has.
    #[arg(long, value_name = "N", default_value_t = 0, value_parser = lod_range())]
    pub(crate) lod: u32,
    /// The mip level of the textures: 0 is the largest, each level halves the width and height;
    /// a texture with fewer mips gives its smallest.
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub(crate) texture_mip: u32,
    /// Leaves the vertex colours out of the models.
    #[arg(long)]
    pub(crate) no_vertex_colours: bool,
    /// Includes the versioned authored SOC room and door document.
    #[arg(long)]
    pub(crate) authored_soc: bool,
}

/// Arguments of `p4kx interior source`.
#[derive(Debug, Args)]
pub(crate) struct InteriorSourceArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Container Entry paths from the outermost to the selected container.
    #[arg(required = true, value_name = "CONTAINER")]
    pub(crate) containers: Vec<String>,
    /// The JSON file to replace after the original sources have been captured.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: PathBuf,
}
