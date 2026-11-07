// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::process::ExitCode;

use p4kx_assembly::{AssemblyError, ChangeError, EntityLookupError, HeadError, LoadoutError};
use p4kx_blender::PackageError;
use p4kx_comparison::LineDiffError;
use p4kx_export::{ActionMapError, CatalogError};
use p4kx_extraction::cryxml::CryXmlError;
use p4kx_extraction::{
    ArchiveError, DataForgeError, DataForgeLoadError, FieldStatisticsError, FilterError,
    FolderError, LocalizationLoadError, UnknownEntries,
};
use p4kx_screens::ScreenError;

use crate::armour_loadout::UnreadableLoadout;
use crate::channels::InvalidPath;
use crate::chf::{InvalidChf, InvalidChfJson, UnreadableChfInput};
use crate::cryxml::{JsonNeedsOut, UnreadableInput};
use crate::dcb_export::{
    ArchiveOptionRefused, CombinedNeedsXml, FlatConflict, NoRecordMatch, TextsNeedArchive,
};
use crate::dcb_query::{InvalidQuery, JsonNeedsJsonFormat, OutputIsArchive, UnreadableSchema};
use crate::dcb_refs::{JsonWithTable, UnknownRecord};
use crate::dcb_tags::UnknownTag;
use crate::entity_glb::GlbOutputIsArchive;
use crate::entity_root::RootGeometryError;
use crate::filter_pair::ExcludedAll;
use crate::grep::EmptySearchText;
use crate::interior_glb::InteriorOutputIsArchive;
use crate::key::InvalidKey;
use crate::loc::{LanguageNotInArchive, NoLanguages};
use crate::no_match::NoMatch;
use crate::wwise::{UnknownAudioLanguage, UnknownTrigger};
use crate::{model, texture};
use p4kx_render::OffscreenError;

/// Every requested Entry was extracted or already existed; a Comparison found no difference.
pub(crate) const SUCCESS: u8 = 0;
/// Some Entries or Records failed or were cancelled, the data of an Entry does not match its
/// checksum, the target folder could not be created, a texture is invalid or cannot be merged,
/// decoded or written, a sound or soundbank is invalid or cannot be converted or written, a tag or
/// definition of a UI movie cannot be read or written, a model cannot be exported or written or has
/// a layout that is not implemented, or `wwise events` cannot look up the Entities.
pub(crate) const PARTIAL_FAILURE: u8 = 1;
/// A Comparison found differences, or `chf diff` found two CHF files that differ.
pub(crate) const DIFFERENCES: u8 = 1;
/// The command line is invalid, names an Entry the Archive does not contain (for a line diff:
/// neither Archive contains), a file or geometry companion that does not exist, a mip level the
/// texture, a chunk the chunk file or a LOD the model does not have, has a Filter that matches no
/// Entry or Record, has Filter text that is too long or has too many wildcards, an empty
/// exclusion Filter, or exclusion Filters that leave out every Entry the Filter matches, names an
/// output that is one of its inputs or lies inside a Star Citizen installation, names a
/// package output folder that exists and holds something else than an earlier package, names an
/// armour `--loadout` that is neither an Entity nor an XML Loadout that can be read, has an
/// invalid Query or a Query or field-statistics output that does not fit --json, names a Loadout
/// change (`--exclude`, `--replace`) that is malformed, names a port the Loadout lacks or
/// contradicts another, names a Record dcb refs does not find, or gives CHF JSON that is invalid
/// or cannot be written as CHF, asks loc for a language the Archive holds no global.ini for, or
/// looks up a localisation key that has no text, asks actionmap export for an Archive without a
/// default profile, names an Entity whose own geometry the Archive lacks, or
/// names a prefab library or prefab the Archive lacks.
pub(crate) const INVALID_INPUT: u8 = 2;
/// The Archive, a container in it, an Entry's data, the input file or a model companion cannot be
/// opened or read, is no valid chunk file, model or CHF file, or is too large to read into memory;
/// also the geometry of an exported Entity that cannot be read.
pub(crate) const ARCHIVE_ERROR: u8 = 3;
/// The Archive changed on disk after it was opened, so the Extraction stopped.
pub(crate) const ARCHIVE_CHANGED: u8 = 4;
/// The Archive holds no DataForge, or its DataForge has an unknown version or is corrupt.
pub(crate) const DATAFORGE_ERROR: u8 = 5;
/// A Comparison ran, but its report or its output cannot be written.
pub(crate) const COMPARISON_FAILED: u8 = 6;
/// An Entry needed the Archive key, and none was given or the one given does not fit.
pub(crate) const KEY_ERROR: u8 = 7;
/// A second Ctrl+C ended an Extraction without waiting for it to stop.
pub(crate) const INTERRUPTED: u8 = 130;

/// The exit-code section of the help text.
pub(crate) const EXIT_CODES_HELP: &str = "Exit codes:
  0  success: every requested Entry was extracted or already existed; verify: every Entry matched
     its checksum; compare: no differences, also for the --diff Entry
  1  partial failure: some Entries or Records failed or were cancelled, the data of an Entry
     does not match its checksum (verify, extract --verify), the target or the settings cannot
     be written, a texture is invalid or cannot be merged, decoded or written, e.g. because its
     format is not supported, a sound or soundbank is invalid or cannot be
     converted or written, e.g. a corrupt WEM or BNK or a codec without a decoder, a tag or
     definition of a UI movie cannot be read or written, e.g. a truncated bitmap tag or a
     missing external texture, a screen that ui render --screen draws without some widgets
     (types without a renderer, approximated layout) or whose composition hit a limit, or
     the renderer skipped an element of it, or a model cannot be exported or written, e.g. one too
     large for a GLB file, or a CrCh model with a layout that is not implemented (streamed
     mesh, big-endian chunk, another chunk version, physics primitive other than a polyhedron),
     or a head that chf glb or character package could not shape by its DNA (the GLB is written
     without it) or whose expressions it could not make, or a name given to --expression that
     the rig lacks or that moves no vertex (the GLB is written without them); wwise events: the
     Entities cannot be looked up (the triggers are still listed); compare: the Archives or the
     --diff Entry differ; chf diff: the CHF files differ; media export: diagnosed incompleteness,
     unavailable providers, exceeded limits, cancellation or generation failures
  2  invalid input: bad arguments, an Entry path the Archive does not contain (for --diff:
     neither Archive contains), a Filter that matches no Entry or Record or whose --exclude
     Filters leave nothing, Filter text that is too long or has too many wildcards, a Channel
     path that does not exist or was never added, a texture, sound, soundbank, chunk file, UI
     movie file, model or geometry companion that does not exist, the geometry of the Entity
     that entity glb or entity package exports when it does not exist (nothing is written), a
     mip level the texture, a chunk the chunk file or a LOD the model does not have, an output
     that is one of its inputs or lies inside a Star Citizen installation (the tool never writes
     there), a package folder that exists and holds something else than an earlier package,
     --json without --out for cryxml or chf to-json, a catalog --out ending in .gz, an invalid
     Query, --json with another --format and without --out for dcb query, --json with --format
     table for dcb refs or grep, --json with another format than json for dcb fields, an empty
     text for grep, a GUID or record path for dcb refs that no Record has, an Entity that no
     Record or several Records match, an armour --loadout that is neither an Entity nor an XML
     Loadout that can be read, a Loadout change (--exclude, --replace) that is malformed, names
     a port the Loadout lacks or contradicts another, an audio trigger no ATL definition has, an
     audio language the Archive holds no folder for, or a CHF JSON file that is invalid, has an
     unknown version or holds values a CHF file cannot store, a CHF file for chf glb or
     character package that holds no face DNA
     or whose DNA belongs to an unknown head rig, a language the Archive holds no global.ini
     for, or a localisation key that has no text in the language or its fallback, an Archive
     without defaultProfile.xml for actionmap export, --csv-raw with another format for
     actionmap export, a canvas for ui render --screen that no Record, several Records or a
     Record of another type name, or whose side is below 1 or above 16384 pixels, or a prefab
     library or prefab the Archive lacks, or for convert an input that is no folder or an output
     folder that overlaps the input folder, or an unsafe media export destination
  3  the Archive or the input file cannot be read: missing, not an Archive, not CryXml, not a
     chunk or CHF file or UI movie, corrupt, a corrupt model or companion (for entity glb and
     entity package the geometry of the Entity itself, its .chrm or .skinm companion or the
     character definition that names no model, which cannot be read; nothing is written), with
     a CHF CRC-32C mismatch or unknown CHF version, a sound, soundbank, chunk file or UI movie
     too large to read into memory, or with unreadable Entry data, also a socpak container that
     cannot be opened, a --diff Entry that cannot be read or holds corrupt CryXml, a global.ini of the
     catalog that is corrupt or cannot be read, a defaultProfile.xml that is no XML, is too large
     or cannot be read, a prefab library that cannot be read, the skeleton of a body variant
     (armour glb) that is missing or unreadable, or the input folder of convert that cannot be
     read or holds too many files; a missing texture, sound, soundbank, chunk file, UI movie
     file, model or geometry companion is 2
  4  the Archive changed on disk while it was open, e.g. updated by the RSI Launcher; the
     Extraction stopped, and Entries written before may be stale
  5  the DataForge cannot be read: the Archive holds none, its version is not supported or it
     is corrupt, or its Records have more field paths than dcb fields counts, or the Record of
     the Entity whose Loadout or audio is shown cannot be read
  6  compare: the report or the output cannot be written
  7  the Archive key is missing or wrong: an Entry is encrypted and no key was given with --key,
     P4KX_ARCHIVE_KEY or `p4kx key set`, or the key does not decrypt the Archive
  130  interrupted: a second Ctrl+C ended an Extraction at once; temporary .part files may remain";

/// Returns the exit code for an error that ended a command.
pub(crate) fn for_error(error: &anyhow::Error) -> ExitCode {
    ExitCode::from(code_for(error))
}

/// Returns the exit code for an error that ended `p4kx compare`, whose code 1 means differences.
pub(crate) fn for_compare_error(error: &anyhow::Error) -> ExitCode {
    match code_for(error) {
        PARTIAL_FAILURE => ExitCode::from(COMPARISON_FAILED),
        code => ExitCode::from(code),
    }
}

/// Returns whether `error` is Filter text that is invalid or a Filter pair that selects nothing.
fn is_filter_refusal(error: &anyhow::Error) -> bool {
    error.downcast_ref::<NoMatch>().is_some()
        || error.downcast_ref::<ExcludedAll>().is_some()
        || error.downcast_ref::<FilterError>().is_some()
}

fn specialized_code(error: &anyhow::Error) -> Option<u8> {
    if crate::key::is_key_error(error) {
        return Some(KEY_ERROR);
    }
    if let Some(code) = crate::media::error_code(error) {
        return Some(code);
    }
    error
        .downcast_ref::<LineDiffError>()
        .map(|error| match error {
            LineDiffError::UnknownEntry { .. } => INVALID_INPUT,
            _ => ARCHIVE_ERROR,
        })
}

fn code_for(error: &anyhow::Error) -> u8 {
    if let Some(code) = specialized_code(error) {
        return code;
    }
    let dataforge = error.downcast_ref::<DataForgeLoadError>();
    if error.downcast_ref::<ArchiveError>().is_some()
        || texture::is_archive_error(error)
        || model::is_archive_error(error)
        || error.downcast_ref::<CryXmlError>().is_some()
        || error.downcast_ref::<UnreadableInput>().is_some()
        || error.downcast_ref::<UnreadableChfInput>().is_some()
        || error.downcast_ref::<InvalidChf>().is_some()
        || is_text_or_profile_error(error)
        || is_unreadable_assembly_source(error)
        || matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::Texts { .. })
        )
        || matches!(
            dataforge,
            Some(DataForgeLoadError::Archive(_) | DataForgeLoadError::Read { .. })
        )
    {
        ARCHIVE_ERROR
    } else if dataforge.is_some()
        || error.downcast_ref::<DataForgeError>().is_some()
        || error.downcast_ref::<FieldStatisticsError>().is_some()
        || is_loadout_or_screen_forge_error(error)
        || is_unreadable_change(error)
    {
        DATAFORGE_ERROR
    } else if error.downcast_ref::<UnknownEntries>().is_some()
        || error.downcast_ref::<InvalidPath>().is_some()
        || is_filter_refusal(error)
        || error.downcast_ref::<NoRecordMatch>().is_some()
        || is_export_conflict(error)
        || texture::is_invalid_input(error)
        || model::is_invalid_input(error)
        || is_output_or_query_error(error)
        || is_missing_root_geometry(error)
        || error.downcast_ref::<EntityLookupError>().is_some()
        || is_screen_error(error, |error| !matches!(error, ScreenError::DataForge(_)))
        || is_unknown_audio_input(error)
        || error.downcast_ref::<InvalidChfJson>().is_some()
        || crate::catalog_input::is_invalid_input(error)
        || error.downcast_ref::<LanguageNotInArchive>().is_some()
        || error.downcast_ref::<NoLanguages>().is_some()
        || is_action_map_error(error, true)
        || error.downcast_ref::<InvalidKey>().is_some()
        || crate::guard::is_refusal(error)
    {
        INVALID_INPUT
    } else {
        PARTIAL_FAILURE
    }
}

fn is_text_or_profile_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<LocalizationLoadError>().is_some()
        || is_action_map_error(error, false)
        || is_unreadable_folder(error)
}

fn is_action_map_error(error: &anyhow::Error, missing: bool) -> bool {
    error
        .downcast_ref::<ActionMapError>()
        .is_some_and(|error| matches!(error, ActionMapError::NotFound) == missing)
}

fn is_screen_error(error: &anyhow::Error, which: impl Fn(&ScreenError) -> bool) -> bool {
    error.downcast_ref::<ScreenError>().is_some_and(which)
}

fn is_loadout_or_screen_forge_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<UnreadableSchema>().is_some()
        || error.downcast_ref::<LoadoutError>().is_some()
        || is_screen_error(error, |error| matches!(error, ScreenError::DataForge(_)))
}

/// Returns whether `error` is an audio trigger or language the Archive does not define.
fn is_unknown_audio_input(error: &anyhow::Error) -> bool {
    error.downcast_ref::<UnknownTrigger>().is_some()
        || error.downcast_ref::<UnknownAudioLanguage>().is_some()
}

/// Returns whether `error` is an input folder of `p4kx convert` that cannot be read or is too
/// large.
fn is_unreadable_folder(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<FolderError>(),
        Some(
            FolderError::Read { .. }
                | FolderError::TooManyEntries { .. }
                | FolderError::TooDeep { .. }
        )
    )
}

/// Returns whether `error` is a folder pair of `p4kx convert` that must not be used: the input
/// is no folder, or the output overlaps it or lies in a Star Citizen installation.
fn is_refused_folder(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<FolderError>(),
        Some(
            FolderError::NotAFolder(_) | FolderError::Overlap { .. } | FolderError::Installation(_)
        )
    )
}

/// Returns whether `error` is a source of an Assembly that cannot be read: the skeleton of a body
/// variant, a prefab library or the geometry of the exported Entity itself.
fn is_unreadable_assembly_source(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<AssemblyError>(),
        Some(AssemblyError::Body { .. })
    ) || crate::prefab::is_archive_error(error)
        || is_unreadable_root_geometry(error)
}

/// Returns whether `error` is a geometry of the exported Entity itself that does not exist.
fn is_missing_root_geometry(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<RootGeometryError>(),
        Some(RootGeometryError::Missing { .. })
    )
}

/// Returns whether `error` is a geometry of the exported Entity itself that cannot be read.
fn is_unreadable_root_geometry(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<RootGeometryError>(),
        Some(RootGeometryError::Unreadable { .. })
    )
}

fn is_export_conflict(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CombinedNeedsXml>().is_some()
        || error.downcast_ref::<FlatConflict>().is_some()
        || error.downcast_ref::<TextsNeedArchive>().is_some()
        || error.downcast_ref::<ArchiveOptionRefused>().is_some()
}

fn is_unreadable_change(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ChangeError>(),
        Some(ChangeError::Loadout(_) | ChangeError::Unreadable { .. })
    )
}

fn is_output_or_query_error(error: &anyhow::Error) -> bool {
    crate::prefab::is_invalid_input(error)
        || is_refused_folder(error)
        || error.downcast_ref::<ChangeError>().is_some()
        || error.downcast_ref::<JsonNeedsOut>().is_some()
        || error.downcast_ref::<InvalidQuery>().is_some()
        || error.downcast_ref::<JsonNeedsJsonFormat>().is_some()
        || error.downcast_ref::<JsonWithTable>().is_some()
        || error.downcast_ref::<EmptySearchText>().is_some()
        || error.downcast_ref::<UnknownRecord>().is_some()
        || error.downcast_ref::<UnknownTag>().is_some()
        || error.downcast_ref::<OutputIsArchive>().is_some()
        || error.downcast_ref::<GlbOutputIsArchive>().is_some()
        || matches!(
            error.downcast_ref::<OffscreenError>(),
            Some(OffscreenError::Empty | OffscreenError::TooLarge { .. })
        )
        || matches!(
            error.downcast_ref::<PackageError>(),
            Some(PackageError::NotAPackage { .. } | PackageError::NoFolderName { .. })
        )
        || error.downcast_ref::<HeadError>().is_some()
        || error.downcast_ref::<UnreadableLoadout>().is_some()
        || error.downcast_ref::<InteriorOutputIsArchive>().is_some()
}
