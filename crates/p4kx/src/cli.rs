// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use p4kx_settings::KeyText;

pub(crate) use crate::cli_actionmap::ActionMapArgs;
pub(crate) use crate::cli_armour::ArmourArgs;
pub(crate) use crate::cli_audio::{AudioAction, AudioArgs, AudioSource};
pub(crate) use crate::cli_bnk::{BnkAction, BnkArgs, BnkFormat, BnkSource};
pub(crate) use crate::cli_bnk_objects::BnkObjectsArgs;
pub(crate) use crate::cli_catalog::{CatalogArgs, CatalogCommand};
pub(crate) use crate::cli_channels::{ChannelsAction, ChannelsArgs};
use crate::cli_character::CharacterArgs;
pub(crate) use crate::cli_chf::{ChfAction, ChfArgs, ChfDiffArgs, ChfDiffFormat, ChfGlbArgs};
use crate::cli_chunks::ChunksArgs;
pub(crate) use crate::cli_conflict::Conflict;
pub(crate) use crate::cli_convert::ConvertArgs;
pub(crate) use crate::cli_coverage::CoverageArgs;
pub(crate) use crate::cli_cryxml::CryXmlArgs;
pub(crate) use crate::cli_dcb::{
    DcbArgs, DcbCommand, DcbExportArgs, DcbInfoArgs, DcbQueryArgs, DcbRefsArgs, DcbTagsArgs,
    QueryFormat, RecordFormat, RefsFormat, TagsFormat,
};
pub(crate) use crate::cli_dcb_fields::DcbFieldsArgs;
pub(crate) use crate::cli_entity::{EntityAction, EntityArgs, EntityAudioArgs, EntityLoadoutArgs};
pub(crate) use crate::cli_extract::ExtractArgs;
pub(crate) use crate::cli_grep::{GrepArgs, GrepFormat};
pub(crate) use crate::cli_key::{KeyAction, KeyArgs};
use crate::cli_loc::LocArgs;
use crate::cli_media::MediaArgs;
use crate::cli_model::ModelArgs;
pub(crate) use crate::cli_output::{AudioOutput, TextureOutput};
pub(crate) use crate::cli_ships::{ShipsArgs, ShipsFormat};
pub(crate) use crate::cli_socpak::{SocpakAction, SocpakArgs, SocpakSource};
pub(crate) use crate::cli_ui::{UiAction, UiArgs, UiScreensArgs};
pub(crate) use crate::cli_verify::VerifyArgs;
pub(crate) use crate::cli_wwise::{
    AudioLanguageArgs, WwiseAction, WwiseArgs, WwiseEventsArgs, WwiseResolveArgs,
};
use crate::exit::EXIT_CODES_HELP;
use crate::notice::{LEGAL_NOTICE, VERSION};

/// Opens, lists and extracts Star Citizen Data.p4k Archives.
#[derive(Debug, Parser)]
#[command(
    name = "p4kx",
    version = VERSION,
    before_help = LEGAL_NOTICE,
    after_help = EXIT_CODES_HELP
)]
pub(crate) struct Cli {
    /// Prints the result as one line of JSON instead of text.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// The Archive key as 32 hexadecimal digits, needed to read encrypted Entries. Without it the
    /// `P4KX_ARCHIVE_KEY` environment variable is used, then the key stored with `p4kx key set`.
    /// Prefer the variable or the stored key: an option shows up in process lists and shell
    /// history.
    #[arg(long = "key", global = true, value_name = "HEX")]
    pub(crate) archive_key: Option<KeyText>,
    /// Reads and writes the stored Archive key in this file instead of the one in your profile;
    /// the `P4KX_KEY_FILE` environment variable does the same.
    #[arg(long, global = true, value_name = "FILE", env = "P4KX_KEY_FILE")]
    pub(crate) key_file: Option<PathBuf>,
    /// Rebuilds an Archive whose end of central directory is missing or whose central directory
    /// is damaged by scanning its last 4 GiB for directory records. Entries whose records are
    /// lost are missing; a warning says so. An intact Archive opens as usual. The `key` commands
    /// ignore it. Never the default.
    #[arg(long, global = true)]
    pub(crate) recover: bool,
    /// What to do.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The subcommands of `p4kx`.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Lists every Entry of an Archive, a split texture as one Entry.
    #[command(after_help = EXIT_CODES_HELP)]
    List(ListArgs),
    /// Lists the Entries of an Archive whose path matches a Filter.
    #[command(after_help = EXIT_CODES_HELP)]
    Search(SearchArgs),
    /// Extracts the given Entries of an Archive into a new target folder.
    #[command(after_help = EXIT_CODES_HELP)]
    Extract(ExtractArgs),
    /// Converts the files of a folder extracted earlier, CryXml, split textures, sounds,
    /// soundbanks and a DataForge file, into a new folder as `extract --convert` writes them.
    #[command(after_help = EXIT_CODES_HELP)]
    Convert(ConvertArgs),
    /// Checks the data of the Entries of an Archive against the CRC-32C of its directory, in
    /// parallel, and reports every mismatch; nothing is written. Exits with 1 when an Entry
    /// mismatches or cannot be read.
    #[command(after_help = EXIT_CODES_HELP)]
    Verify(VerifyArgs),
    /// Prints the Channel and Build of an Archive and a summary of its Entries.
    #[command(after_help = EXIT_CODES_HELP)]
    Info(InfoArgs),
    /// Lists the installed Channels, or adds or removes a manually added folder or Archive.
    #[command(after_help = EXIT_CODES_HELP)]
    Channels(ChannelsArgs),
    /// Reads the DataForge of an Archive.
    #[command(after_help = EXIT_CODES_HELP)]
    Dcb(DcbArgs),
    /// Finds a text in the contents of an Archive: in the string, localisation key and enum
    /// values of Records, in the localisation texts of each language and in the lines of text and
    /// CryXml Entries.
    #[command(after_help = EXIT_CODES_HELP)]
    Grep(GrepArgs),
    /// Writes the Basetool catalog of an Archive as JSON, or prints its JSON Schema.
    #[command(
        after_help = EXIT_CODES_HELP,
        args_conflicts_with_subcommands = true,
        subcommand_negates_reqs = true
    )]
    Catalog(CatalogArgs),
    /// Exports versioned media metadata and retained sources, or prints their schemas.
    #[command(after_help = EXIT_CODES_HELP)]
    Media(MediaArgs),
    /// Checks the game data a companion app reads against the Archive: whether each listed Record
    /// type, `EntityClassDefinition` component and vehicle folder exists, how many it has and
    /// whether it reads, exports and converts; exits with 1 when something listed fails.
    #[command(after_help = EXIT_CODES_HELP)]
    Coverage(CoverageArgs),
    /// Writes the base data of every ship as JSON or NDJSON: hull mass and hit points from the
    /// vehicle implementation XML, cargo capacity from the cargo grids of the default Loadout,
    /// size and the Loadout as class names per port path, each value with the file and field it
    /// was read from. Docked vehicles are not counted or expanded.
    #[command(after_help = EXIT_CODES_HELP)]
    Ships(ShipsArgs),
    /// Shows the Loadout tree or the audio triggers of an Entity: a ship, vehicle, weapon or
    /// item.
    #[command(after_help = EXIT_CODES_HELP)]
    Entity(EntityArgs),
    /// Exports the Interior of a socpak container as GLB.
    #[command(after_help = EXIT_CODES_HELP)]
    Interior(crate::cli_interior::InteriorArgs),
    /// Lists the prefab libraries of an Archive and their prefabs, or prints a prefab with its
    /// nested prefabs resolved.
    #[command(after_help = EXIT_CODES_HELP)]
    Prefab(crate::cli_prefab::PrefabArgs),
    /// Lists the body variants, or writes pieces of armour and clothing worn on a body as one
    /// skinned GLB.
    #[command(after_help = EXIT_CODES_HELP)]
    Armour(ArmourArgs),
    /// Writes a Character, its head, armour and animations on a body, as a Blender package.
    #[command(after_help = EXIT_CODES_HELP)]
    Character(CharacterArgs),
    /// Compares the Entries and Records of an old and a new Archive, or prints the line diff of
    /// one Entry; exits with 1 when they differ.
    #[command(after_help = EXIT_CODES_HELP)]
    Compare(CompareArgs),
    /// Lists the languages of an Archive, looks up the text of a localisation key or exports all
    /// keys with their texts.
    #[command(after_help = EXIT_CODES_HELP)]
    Loc(LocArgs),
    /// Exports the action maps of the default profile: every action with its localised label and
    /// its default key, mouse, gamepad and joystick bindings.
    #[command(name = "actionmap", after_help = EXIT_CODES_HELP)]
    ActionMap(ActionMapArgs),
    /// Converts a CryXml file on disk to UTF-8 XML; text XML is written unchanged.
    #[command(name = "cryxml", after_help = EXIT_CODES_HELP)]
    CryXml(CryXmlArgs),
    /// Shows, merges or exports as PNG a DDS texture split into a header and mip parts.
    #[command(after_help = EXIT_CODES_HELP)]
    Texture(TextureArgs),
    /// Shows a Wwise WEM sound or converts it to WAV.
    #[command(after_help = EXIT_CODES_HELP)]
    Audio(AudioArgs),
    /// Lists or extracts the WEMs a Wwise BNK soundbank embeds, or prints its object hierarchy.
    #[command(after_help = EXIT_CODES_HELP)]
    Bnk(BnkArgs),
    /// Lists or extracts the bitmaps, shapes, fonts and sprites of a GFx or SWF UI movie.
    #[command(after_help = EXIT_CODES_HELP)]
    Ui(UiArgs),
    /// Lists or extracts the Entries of a socpak container, also of one nested in another.
    #[command(after_help = EXIT_CODES_HELP)]
    Socpak(SocpakArgs),
    /// Converts a CHF character file to JSON and back, byte-exactly.
    #[command(after_help = EXIT_CODES_HELP)]
    Chf(ChfArgs),
    /// Lists the chunks of a CryEngine chunk file, #ivo or CrCh, or writes the bytes of one.
    #[command(after_help = EXIT_CODES_HELP)]
    Chunks(ChunksArgs),
    /// Resolves Wwise audio triggers through the soundbank hierarchy to their sounds.
    #[command(after_help = EXIT_CODES_HELP)]
    Wwise(WwiseArgs),
    /// Shows a static #ivo model with its nodes, meshes and materials, or writes it as GLB.
    #[command(after_help = EXIT_CODES_HELP)]
    Model(ModelArgs),
    /// Stores, shows or removes the Archive key of your profile.
    #[command(after_help = EXIT_CODES_HELP)]
    Key(KeyArgs),
}

/// Arguments of `p4kx texture`.
#[derive(Debug, Args)]
pub(crate) struct TextureArgs {
    /// What to do with the texture.
    #[command(subcommand)]
    pub(crate) action: TextureAction,
}

/// The actions of `p4kx texture`.
#[derive(Debug, Subcommand)]
pub(crate) enum TextureAction {
    /// Prints the format, size, depth, mip count, array size, cubemap faces, parts and alpha tail
    /// of a texture.
    #[command(after_help = EXIT_CODES_HELP)]
    Info(TextureSource),
    /// Merges the header and the mip parts of a texture into one standard DDS file; a cubemap
    /// stays a cubemap. The alpha tail is not included; name `<ENTRY>.a` to merge it on its own.
    #[command(after_help = EXIT_CODES_HELP)]
    Merge {
        /// The texture.
        #[command(flatten)]
        source: TextureSource,
        /// The DDS file to write; an existing file is replaced.
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
    },
    /// Decodes one mip of a texture, with its alpha tail in the alpha channel, and writes it as an
    /// 8-bit RGBA PNG; a cubemap becomes six face files `<STEM>_posx` … `<STEM>_negz` next to FILE,
    /// an array one file per element `<STEM>_layerN` and a volume one per slice `<STEM>_sliceN`.
    /// A high-range BC6H or floating-point texture is written as OpenEXR with 32-bit float RGB instead.
    #[command(after_help = EXIT_CODES_HELP)]
    Png {
        /// The texture.
        #[command(flatten)]
        source: TextureSource,
        /// The PNG file to write; an existing file is replaced. A high-range texture goes to FILE
        /// with the extension .exr instead, unless --tone-map is given.
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
        /// The mip level to export, 0 being the largest.
        #[arg(long, value_name = "LEVEL", default_value_t = 0)]
        mip: u32,
        /// Writes a cubemap as one horizontal cross into FILE instead of six face files.
        #[arg(long)]
        cross: bool,
        /// Writes a high-range texture as an 8-bit PNG to FILE too, tone-mapped: Reinhard per
        /// channel, then sRGB. Other textures are 8-bit already; for them it only prints a note.
        #[arg(long)]
        tone_map: bool,
    },
}

/// Where a texture's header and parts are.
#[derive(Debug, Args)]
pub(crate) struct TextureSource {
    /// A Data.p4k Archive when ENTRY is given; otherwise the texture's header file, with its parts
    /// `<FILE>.1` … `<FILE>.N` and any alpha tail `<FILE>.a`, `<FILE>.1a` … next to it.
    #[arg(value_name = "ARCHIVE_OR_FILE")]
    pub(crate) path: PathBuf,
    /// Path of the texture's header Entry in the Archive, with `\` or `/` as separator, ignoring
    /// case; its parts are the Entries `<ENTRY>.1` … `<ENTRY>.N` and its alpha tail `<ENTRY>.a`
    /// with `<ENTRY>.1a` … `<ENTRY>.Na`.
    pub(crate) entry: Option<String>,
}

/// Arguments of `p4kx list`.
#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Lists every part of a split texture as an Entry of its own; by default the parts
    /// `<name>.dds.1` … `.N` and the alpha tail `<name>.dds.a` with its parts are folded into the
    /// row of the header `<name>.dds`, with the sizes of the whole texture.
    #[arg(long)]
    pub(crate) parts: bool,
}

/// Arguments of `p4kx search`.
#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The Filter: with `*` (any run of characters) or `?` (one character) a glob over the full
    /// path, otherwise text anywhere in the path; ignores case, `\` and `/` match alike.
    pub(crate) filter: String,
    /// Leaves out every Entry this Filter matches, with the rules of the Filter; repeatable. The
    /// parts of a texture whose header it matches are left out too. With --exclude, a search that
    /// finds nothing is invalid input.
    #[arg(long, value_name = "FILTER")]
    pub(crate) exclude: Vec<String>,
}

/// Arguments of `p4kx compare`.
#[derive(Debug, Args)]
pub(crate) struct CompareArgs {
    /// Path of the old Data.p4k Archive, the side the changes start from.
    pub(crate) old: PathBuf,
    /// Path of the new Data.p4k Archive, the side the changes lead to.
    pub(crate) new: PathBuf,
    /// Compares the Entries from both central directories, without decompressing: added, removed,
    /// or changed in size or CRC-32C. Every level is compared when none is given.
    #[arg(long)]
    pub(crate) entries: bool,
    /// Compares the Records of both DataForges, joined by GUID: added, removed, moved to another
    /// record path, or changed in name, type or a field value. Every level is compared when none
    /// is given.
    #[arg(long)]
    pub(crate) records: bool,
    /// Compares only the Entries whose path and the Records whose record path on either side the
    /// Filter matches, with the rules of `p4kx search`.
    #[arg(long, value_name = "FILTER")]
    pub(crate) filter: Option<String>,
    /// Writes the Comparison as a JSON report to FILE; an existing file is replaced.
    #[arg(long, value_name = "FILE")]
    pub(crate) report: Option<PathBuf>,
    /// Prints the line diff of the Entry at this path, with `\` or `/` as separator, ignoring
    /// case, instead of comparing every Entry. CryXml is converted to XML first; an Entry on one
    /// side only is diffed against an empty side, and a binary Entry is only reported as changed
    /// or not. With --json, the hunks are printed as JSON.
    #[arg(long, value_name = "ENTRY", conflicts_with_all = ["entries", "filter", "report"])]
    pub(crate) diff: Option<String>,
    /// Compares the schema of both DataForges instead of their contents: the structs, properties
    /// (data type, conversion type, target), enums and enum options added, removed or changed,
    /// each with the number of Records it concerns on both sides. With --json, the report is
    /// printed as JSON.
    #[arg(
        long,
        conflicts_with_all = ["entries", "records", "filter", "report", "diff"]
    )]
    pub(crate) schema: bool,
}

/// Arguments of `p4kx info`.
#[derive(Debug, Args)]
pub(crate) struct InfoArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
}
