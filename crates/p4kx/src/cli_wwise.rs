// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand};
use p4kx_assembly::audio::ResolveOptions;
use p4kx_extraction::CancelToken;

use crate::cli::{AudioOutput, Conflict};
use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx wwise`.
#[derive(Debug, Args)]
pub(crate) struct WwiseArgs {
    /// What to do with the Wwise audio.
    #[command(subcommand)]
    pub(crate) action: WwiseAction,
}

/// The actions of `p4kx wwise`.
#[derive(Debug, Subcommand)]
pub(crate) enum WwiseAction {
    /// Resolves an audio trigger to its sounds: follows each Wwise event through its actions and
    /// random, sequence, switch and blend containers to the sounds and lists every variant with
    /// its soundbank, WEM ID, source and container path; an embedded WEM is located in the
    /// soundbank that holds it. Where the chain breaks, the object and the reason are listed.
    /// A streamed or prefetched WEM is located as the loose WEM Entry named by its ID; a localised
    /// event, soundbank or WEM is resolved for --lang, with --fallback-lang where that has no
    /// localisation, and a missing localisation is listed as a broken link. Reads every ATL
    /// definition and soundbank of the Archive once for the audio index, then the soundbanks
    /// holding the trigger's events again.
    #[command(after_help = EXIT_CODES_HELP)]
    Resolve(WwiseResolveArgs),
    /// Searches the audio triggers and Wwise events of the ATL definitions by name: --search is a
    /// substring or a glob with * and ?, ignoring case, matched against trigger and event names;
    /// without it every trigger is listed. Each result lists the trigger, the Wwise events it
    /// posts with the soundbanks that hold them and the Entities that name the trigger. Results
    /// come in definition order, --limit at a time from --offset on, and the summary line says
    /// where the next page starts. Events that no ATL definition names are not found by name.
    /// Reads every ATL definition and soundbank of the Archive once and, to find the Entities,
    /// the Record of every Entity once.
    #[command(after_help = EXIT_CODES_HELP)]
    Events(WwiseEventsArgs),
    /// Writes the sounds of an audio trigger into --out as Ogg or WAV files named after the
    /// trigger: `<trigger>.<ext>` for a trigger with one sound, `<trigger>_<WEM ID>.<ext>` for
    /// one with several; a WEM several variants lead to is written once. Names are cut to ASCII
    /// letters, digits, `_` and `-`, and a name already taken, ignoring case, gets `_2`, `_3`, …
    /// appended. Resolves the trigger as `resolve` does, so --lang and --fallback-lang choose the
    /// language of localised sounds. A sound that cannot be decoded is reported and written
    /// unchanged as .wem, as are plug-in media and the prefetched start of a streamed WEM; a
    /// sound with no WEM, or none located, is listed as failed. Broken links are counted. With
    /// --txtp, vgmstream TXTP files for the trigger's Wwise events are written too.
    #[command(after_help = EXIT_CODES_HELP)]
    Extract(WwiseExtractArgs),
}

/// Arguments of `p4kx wwise extract`.
#[derive(Debug, Args)]
pub(crate) struct WwiseExtractArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The audio trigger as its ATL definition names it, e.g. `Play_QDrive_Spool`; ignores case.
    pub(crate) trigger: String,
    /// Target folder; created if missing, files already in it are handled by --conflict.
    #[arg(long, value_name = "FOLDER")]
    pub(crate) out: PathBuf,
    #[command(flatten)]
    pub(crate) export: SoundExportArgs,
    /// Also writes one `<event>.txtp` per Wwise event of the trigger next to the sounds: a text
    /// file that vgmstream plays as the event does, layering an Event's actions and blend
    /// containers, chaining sequence containers and music playlists and picking the first child of
    /// a random or switch container (change `>1` in its `group` line to pick another). The loop of
    /// a sound is written as `#I`. An event with a sound that has no whole file is listed instead.
    /// P4KX only writes the text; it does not run vgmstream.
    #[arg(long)]
    pub(crate) txtp: bool,
    #[command(flatten)]
    pub(crate) resolve: AudioResolveArgs,
}

/// How exported sounds are written.
#[derive(Debug, Args)]
pub(crate) struct SoundExportArgs {
    /// The format of the sound files: `ogg` rebuilds Wwise Vorbis and Opus without re-encoding
    /// and writes other codecs as WAV, `wav` writes a 16-bit PCM WAV, `raw` the WEM unchanged.
    #[arg(long, value_enum, default_value_t = AudioOutput::Ogg, value_name = "FORMAT")]
    pub(crate) format: AudioOutput,
    /// What to do with a file that already exists in the target.
    #[arg(long, value_enum, default_value_t = Conflict::Skip)]
    pub(crate) conflict: Conflict,
}

/// Arguments of `p4kx wwise events`.
#[derive(Debug, Args)]
pub(crate) struct WwiseEventsArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Text in a trigger or event name, or a glob with `*` and `?` such as `Play_*_Spool`;
    /// ignores case, and `/` and `\` match each other.
    #[arg(long, value_name = "TEXT")]
    pub(crate) search: Option<String>,
    /// The most results listed.
    #[arg(
        long,
        default_value_t = 100,
        value_name = "COUNT",
        value_parser = clap::value_parser!(u32).range(1..=10_000)
    )]
    pub(crate) limit: u32,
    /// The number of results to skip, as the summary line of the previous page gives it.
    #[arg(long, default_value_t = 0, value_name = "COUNT")]
    pub(crate) offset: u32,
    /// The most Entities listed per trigger, in definition order; the count is always given. Only
    /// Entities whose own Record names the trigger are counted, not those that reach it through
    /// a Loadout or a Reference.
    #[arg(
        long,
        default_value_t = 5,
        value_name = "COUNT",
        value_parser = clap::value_parser!(u32).range(0..=1_000),
        conflicts_with = "no_entities"
    )]
    pub(crate) entities: u32,
    /// Does not look for the Entities, so the DataForge is not read.
    #[arg(long)]
    pub(crate) no_entities: bool,
}

/// Arguments of `p4kx wwise resolve`.
#[derive(Debug, Args)]
pub(crate) struct WwiseResolveArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The audio trigger as its ATL definition names it, e.g. `Play_QDrive_Spool`; ignores case.
    pub(crate) trigger: String,
    #[command(flatten)]
    pub(crate) resolve: AudioResolveArgs,
}

/// Language selection and traversal controls for audio trigger resolution.
#[derive(Debug, Args)]
pub(crate) struct AudioResolveArgs {
    /// The language folder below Data/Sounds/wwise to resolve localised sounds in, as the
    /// Archive spells it, e.g. English(US); ignores case. Without it, the fallback language.
    #[arg(long, value_name = "LANGUAGE")]
    pub(crate) lang: Option<String>,
    /// The language folder used where --lang has no localisation of a sound; defaults to
    /// English(US), or the first language folder when the Archive has none of that name.
    #[arg(long, value_name = "LANGUAGE")]
    pub(crate) fallback_lang: Option<String>,
    /// Limits attempted HIRC objects and music track sources per trigger. Without it, full
    /// traversal. A cut is reported by inspection and prevents extraction.
    #[arg(long, value_name = "COUNT")]
    max_visits: Option<usize>,
}

impl AudioResolveArgs {
    /// Returns the caller's traversal options for one trigger.
    pub(crate) fn options<'a>(&self, cancel: &'a CancelToken) -> ResolveOptions<'a> {
        let options = ResolveOptions::default().with_cancel(cancel);
        self.max_visits
            .map_or(options, |limit| options.with_max_visits(limit))
    }
}
