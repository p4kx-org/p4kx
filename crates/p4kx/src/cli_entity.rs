// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use p4kx_assembly::{ChangeError, LoadoutChanges, LoadoutPicks};

use crate::cli_model::lod_range;
use crate::cli_wwise::{AudioLanguageArgs, SoundExportArgs};
use crate::exit::EXIT_CODES_HELP;

/// The material modes of `p4kx entity glb --materials`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum EntityMaterialsArg {
    /// Plain colours without textures.
    Colours,
    /// The plain colours with the textures of the materials as embedded PNG images.
    Textures,
}

/// The options that change the Loadout of `p4kx entity loadout` and `p4kx entity glb`.
#[derive(Debug, Args, Default)]
pub(crate) struct LoadoutChangeArgs {
    /// Leaves out a port with everything attached below it. The port is its path of port names
    /// from the Entity joined by `/`, as `entity loadout` lists them, e.g.
    /// `hardpoint_weapon_nose/hardpoint_class_2`; ignores case. May be given several times. A
    /// port the Loadout does not have is invalid input.
    #[arg(long, value_name = "PORT")]
    pub(crate) exclude: Vec<String>,
    /// Attaches another Entity to a port instead of what it holds, as `<port>=<entity>`: the
    /// port as for --exclude and the Entity a class name, GUID or record path. The Entity is
    /// resolved with its own default Loadout, and the Loadout of the Entity it replaces is
    /// dropped. An Entity that does not fit the port's item types and sizes is attached all the
    /// same, with a warning on stderr and in `incompatible` in the JSON. May be given several
    /// times.
    #[arg(long, value_name = "PORT=ENTITY")]
    pub(crate) replace: Vec<String>,
    /// Takes another alternative of a group of a generated default Loadout (a creature's or an
    /// NPC's, which the game picks at random) than the first, as `<group>=<index>`, e.g.
    /// `Body=2`; the group is its name in the assortment, ignoring case. A bare `<index>` takes
    /// it in every group that has it. Indices count from 0 (the first alternative). An index
    /// beyond a named group, and a named group the Loadout does not have, is a Loadout issue and
    /// the group keeps its first alternative. Several picks are separated by commas or given as
    /// several options.
    #[arg(long, value_name = "GROUP=INDEX", value_delimiter = ',')]
    pub(crate) loadout_pick: Vec<String>,
}

impl LoadoutChangeArgs {
    /// Returns the changes the options ask for.
    pub(crate) fn changes(&self) -> Result<LoadoutChanges, ChangeError> {
        let changes = self
            .exclude
            .iter()
            .fold(LoadoutChanges::new(), LoadoutChanges::exclude);
        let changes = self
            .replace
            .iter()
            .try_fold(changes, |changes, text| changes.replace_text(text))?;
        let picks = self
            .loadout_pick
            .iter()
            .try_fold(LoadoutPicks::new(), |picks, text| picks.add_text(text))?;
        Ok(changes.with_picks(picks))
    }
}

/// Arguments of `p4kx entity`.
#[derive(Debug, Args)]
pub(crate) struct EntityArgs {
    /// What to show of the Entity.
    #[command(subcommand)]
    pub(crate) action: EntityAction,
}

/// The actions of `p4kx entity`.
#[derive(Debug, Subcommand)]
pub(crate) enum EntityAction {
    /// Prints the default Loadout tree of an Entity: every port, the Entity attached to it and
    /// its own ports, recursively; ports that cannot be resolved are listed with the reason.
    /// With --exclude and --replace it prints the tree of an own configuration, marking the
    /// ports changed.
    #[command(after_help = EXIT_CODES_HELP)]
    Loadout(EntityLoadoutArgs),
    /// Lists the audio triggers an Entity names, each with the Wwise events its ATL definition
    /// posts and the soundbanks that hold them; triggers that reach no soundbank are listed with
    /// the reason. With --sounds each trigger also lists the sounds it plays, with the source
    /// (embedded, streamed, localised) and the location of every WEM. Reads every ATL definition
    /// and soundbank of the Archive once, and with --sounds the soundbanks of the triggers
    /// again.
    #[command(after_help = EXIT_CODES_HELP)]
    Audio(EntityAudioArgs),
    /// Writes an Entity as one glTF 2.0 binary file (GLB): its geometry and the geometry of every
    /// Entity of its default Loadout, each placed at its port, as a node named after the port and
    /// the Entity below the node of the Entity it is attached to. Y up, in metres. Parts whose
    /// Entity or geometry cannot be resolved are skipped and listed; when the geometry of the
    /// Entity itself is missing (exit code 2) or cannot be read (exit code 3), nothing is written.
    /// With --exclude and --replace the Loadout is an own configuration: excluded branches and
    /// the Entity a replacement removes are not written. Reads every model once, in parallel. A
    /// docked vehicle (a Merlin in a docking collar, say) is placed as one part, without the
    /// parts of its own Loadout, unless --expand-docked-vehicles is given; a change to a port
    /// below such a docked vehicle is not applied, is no error, and is warned about on stderr
    /// and listed as `ignored` in the JSON.
    #[command(after_help = EXIT_CODES_HELP)]
    Glb(EntityGlbArgs),
    /// Writes an Entity as a Blender package, a folder that does not depend on Blender's file
    /// format: `scene.json` with the version, the source Channel and Build and the node
    /// hierarchy (names, transforms, the mesh each node draws), one GLB file per distinct mesh
    /// (a model drawn with one material library and palette, however many nodes use it) and
    /// the textures of the materials as PNG files, each written once. The Loadout, parts, levels
    /// of detail and Loadout changes are those of `entity glb`, docked vehicles are never
    /// expanded, and a change below one is not applied and warned about. As for `entity glb`, a
    /// missing or unreadable geometry of the Entity itself fails the run (exit code 2 or 3). The
    /// folder must not exist or must hold an earlier package; it is written beside the target
    /// first and moved into place when complete, so a failed or cancelled run leaves nothing
    /// behind. Y up, in metres. See docs/blender-package.md.
    #[command(after_help = EXIT_CODES_HELP)]
    Package(EntityPackageArgs),
}

/// Arguments of `p4kx entity package`.
#[derive(Debug, Args)]
pub(crate) struct EntityPackageArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The Entity: a class name such as `AEGS_Avenger_Titan`, a GUID, or a record path such as
    /// `libs/foundry/records/entities/spaceships/aegs_avenger_titan`; ignores case.
    pub(crate) entity: String,
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
    /// Leaves the vertex colours out of the parts.
    #[arg(long)]
    pub(crate) no_vertex_colours: bool,
    /// Includes the Interior as for `entity glb --interior`: the socpak containers the Records of
    /// the Entity and of its attached Entities name, each placed at its bone of the Entity that
    /// names it, as nodes below an `Interior` node of that Entity, with their models and props
    /// as nodes that draw meshes and their lights as nodes of the kind `light`. A docked vehicle
    /// brings its containers only when docked vehicles are expanded. Containers and parts that
    /// cannot be resolved are skipped and listed. Without it only the hull is written.
    #[arg(long)]
    pub(crate) interior: bool,
    #[command(flatten)]
    pub(crate) changes: LoadoutChangeArgs,
}

/// Arguments of `p4kx entity glb`.
#[derive(Debug, Args)]
pub(crate) struct EntityGlbArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The Entity: a class name such as `AEGS_Avenger_Titan`, a GUID, or a record path such as
    /// `libs/foundry/records/entities/spaceships/aegs_avenger_titan`; ignores case.
    pub(crate) entity: String,
    /// The GLB file to write; an existing file is replaced.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: PathBuf,
    /// The level of detail of every model: 0 is the most detailed. A model that lacks the level
    /// is written at the nearest lower level it has.
    #[arg(long, value_name = "N", default_value_t = 0, value_parser = lod_range())]
    pub(crate) lod: u32,
    /// How to write the materials. colours: one glTF material per distinct material, with base
    /// colour, opacity, roughness, metalness and emissive colour and no textures. textures: the
    /// same with the diffuse, normal and specular maps of every part as PNG images, each image
    /// once however many parts use it; a texture that cannot be read leaves the plain colour and
    /// is listed. A material library the Entity's Record names replaces the one its model names.
    /// The tint palette of each part (the one its Record names, else the default of its
    /// material library) gives the colours of the materials that take one, in both modes; a
    /// palette that cannot be resolved is listed.
    #[arg(long, value_enum, value_name = "MODE", default_value_t = EntityMaterialsArg::Colours)]
    pub(crate) materials: EntityMaterialsArg,
    /// The mip level of the textures of every part with --materials textures: 0 is the largest,
    /// each level halves the width and height; a texture with fewer mips gives its smallest.
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub(crate) texture_mip: u32,
    /// Leaves the vertex colours out; viewers multiply the material's base colour by them.
    #[arg(long)]
    pub(crate) no_vertex_colours: bool,
    /// Includes the Interior: the socpak containers the Records of the Entity and of its attached
    /// Entities name (`VehicleComponentParams.objectContainers` and
    /// `SObjectContainerComponentParams`), each placed at its bone of the Entity that names it
    /// with the Record's offset, as nodes below an `Interior` node of that Entity, with the
    /// containers nested in them. A docked vehicle brings its containers only with
    /// `--expand-docked-vehicles`. Containers and parts that cannot be resolved are skipped and
    /// listed. Without it only the hull is written.
    #[arg(long)]
    pub(crate) interior: bool,
    /// Expands docked vehicles, the attached Entities whose item type starts with
    /// `NOITEM_Vehicle`, ignoring case: their own Loadout parts are placed below them. Without it
    /// a docked vehicle is placed as one part.
    #[arg(long)]
    pub(crate) expand_docked_vehicles: bool,
    #[command(flatten)]
    pub(crate) changes: LoadoutChangeArgs,
}

/// Arguments of `p4kx entity loadout`.
#[derive(Debug, Args)]
pub(crate) struct EntityLoadoutArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The Entity: a class name such as `AEGS_Avenger_Titan`, a GUID, or a record path such as
    /// `libs/foundry/records/entities/spaceships/aegs_avenger_titan`; ignores case.
    pub(crate) entity: String,
    /// Stops at docked vehicles, the attached Entities whose item type starts with
    /// `NOITEM_Vehicle`, ignoring case: they stay in their port, marked, but their own ports are
    /// not listed. A change (--exclude, --replace) to a port below one is not applied, is no
    /// error, and is warned about on stderr and listed as `ignored` in the JSON.
    #[arg(long)]
    pub(crate) stop_at_docked_vehicles: bool,
    #[command(flatten)]
    pub(crate) changes: LoadoutChangeArgs,
}

/// Arguments of `p4kx entity audio`.
#[derive(Debug, Args)]
pub(crate) struct EntityAudioArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The Entity: a class name such as `ORIG_890Jump`, a GUID, or a record path such as
    /// `libs/foundry/records/entities/spaceships/orig_890jump`; ignores case.
    pub(crate) entity: String,
    /// Also lists the sounds of every trigger: each variant with its source and WEM location.
    #[arg(long)]
    pub(crate) sounds: bool,
    /// Writes the sounds of every trigger into this folder as Ogg or WAV files named
    /// `<trigger>[_<WEM ID>].<ext>`, as `p4kx wwise extract` does; implies --sounds.
    #[arg(long, value_name = "FOLDER")]
    pub(crate) extract: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) export: SoundExportArgs,
    #[command(flatten)]
    pub(crate) language: AudioLanguageArgs,
}
