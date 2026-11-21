// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::cli_armour::BodyArg;
use crate::cli_chf::ChfMaterialsArg;
use crate::cli_model::lod_range;
use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx character`.
#[derive(Debug, Args)]
pub(crate) struct CharacterArgs {
    /// What to do with the Character.
    #[command(subcommand)]
    pub(crate) action: CharacterAction,
}

/// The actions of `p4kx character`.
#[derive(Debug, Subcommand)]
pub(crate) enum CharacterAction {
    /// Writes a Character as a Blender package: a folder with `scene.json` and one skinned GLB
    /// that holds the skeleton of the body, a skinned mesh per part, the morph targets of the
    /// head's expressions and the animations, and that the Blender add-on imports as an armature
    /// with shape keys and actions. The Character is what `chf glb` and `armour glb` write: the
    /// head of a CHF file with its eyes, teeth and hair (--chf), pieces of armour and clothing
    /// (--piece, --loadout), on the default body of the CHF's or the chosen variant (--body).
    /// Without a CHF, a piece or a Loadout, the bare body is written. The materials are in the
    /// GLB file, there are no texture files. Pieces that cannot be fitted and animations that fit
    /// nothing are listed in the manifest and the report.
    #[command(after_help = EXIT_CODES_HELP)]
    Package(Box<CharacterPackageArgs>),
    /// Writes original body, item-port and player default loadout declarations as JSON.
    #[command(after_help = EXIT_CODES_HELP)]
    Source(CharacterSourceArgs),
}

#[derive(Debug, Args)]
pub(crate) struct CharacterSourceArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Body variant whose default CHF is selected. Repeat with --bundle.
    #[arg(long, value_enum, value_name = "VARIANT", required = true)]
    pub(crate) body: Vec<BodyArg>,
    /// Writes a source bundle to a new directory.
    #[arg(long)]
    pub(crate) bundle: bool,
    /// JSON file, or a new directory with --bundle.
    #[arg(long, value_name = "PATH")]
    pub(crate) out: PathBuf,
}

/// Arguments of `p4kx character package`.
#[derive(Debug, Args)]
pub(crate) struct CharacterPackageArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The folder to write the package to; it must not exist, or be empty, or hold a package an
    /// earlier run wrote, which is replaced.
    #[arg(long, value_name = "FOLDER")]
    pub(crate) out: PathBuf,
    /// A CHF character file: its face DNA shapes the head, its head port gives the eyes and hair,
    /// and its head rig decides the body, so --body is not allowed with it. A character without
    /// face DNA or for an unknown head rig is invalid input.
    #[arg(long, value_name = "FILE", conflicts_with = "body")]
    pub(crate) chf: Option<PathBuf>,
    /// The body variant to wear the pieces on when there is no CHF, see `p4kx armour bodies`.
    #[arg(long, value_enum, value_name = "VARIANT")]
    pub(crate) body: Option<BodyArg>,
    /// A piece to wear: an Entity by class name, GUID or record path, as for `armour glb --piece`.
    /// May be given several times.
    #[arg(long = "piece", value_name = "ENTITY")]
    pub(crate) pieces: Vec<String>,
    /// A Loadout to take the pieces from, as for `armour glb --loadout`: a file on disk, an
    /// Entity whose default Loadout is used, or an XML Loadout of the Archive. A piece takes the
    /// place of the Loadout's pieces of the same item type.
    #[arg(long, value_name = "LOADOUT")]
    pub(crate) loadout: Option<String>,
    /// The name of the Character, the name of its root object in Blender; the CHF's file name
    /// without extension, else the code of the body variant.
    #[arg(long, value_name = "NAME")]
    pub(crate) name: Option<String>,
    /// The level of detail of the head and the body: 0 is the most detailed. A model that lacks
    /// the level is written at the nearest lower level it has.
    #[arg(long, value_name = "N", default_value_t = 0, value_parser = lod_range())]
    pub(crate) lod: u32,
    /// How to write the materials into the GLB file. textures: the diffuse, normal and specular
    /// maps as embedded PNG images; colours: plain colours.
    #[arg(long, value_enum, value_name = "MODE", default_value_t = ChfMaterialsArg::Textures)]
    pub(crate) materials: ChfMaterialsArg,
    /// The mip level of the textures with --materials textures: 0 is the largest, each level
    /// halves the width and height. The default 1 keeps the file at a few tens of megabytes.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub(crate) texture_mip: u32,
    /// Leaves the vertex colours out of the meshes.
    #[arg(long)]
    pub(crate) no_vertex_colours: bool,
    /// Writes a source vertex map for the rigged GLB.
    #[arg(long)]
    pub(crate) vertex_maps: bool,
    /// Writes only the expression with this name as a morph target, as for `chf glb
    /// --expression`; repeat the option for more.
    #[arg(
        long = "expression",
        value_name = "NAME",
        conflicts_with = "no_expressions"
    )]
    pub(crate) expressions: Vec<String>,
    /// Leaves the expressions out: the head has its neutral face only.
    #[arg(long)]
    pub(crate) no_expressions: bool,
    /// Includes an animation as for `chf glb --anim`; repeat the option for more, or use `all`.
    /// Animations with keys for bones become actions of the armature, facial animations the
    /// poses of the facial bones and the values of the shape keys, which needs the expressions
    /// and LOD 0.
    #[arg(long = "anim", value_name = "NAME")]
    pub(crate) animations: Vec<String>,
    /// A .caf or .dba Entry to look for animations in, in addition to the folders of the
    /// character; repeatable, used with --anim.
    #[arg(long = "anim-from", value_name = "ENTRY")]
    pub(crate) animation_sources: Vec<String>,
    /// Looks for animations only below this folder of the Archive, a path below `Data` such as
    /// `Animations/pu/male/eckhart`, instead of the folders of the body; repeatable, used with
    /// --anim. Without it, a --loadout whose character has a folder of its own animations below
    /// `Animations/pu/<body>/` looks there first and then in the folders of the body.
    #[arg(long = "anim-folder", value_name = "FOLDER")]
    pub(crate) animation_folders: Vec<String>,
}
