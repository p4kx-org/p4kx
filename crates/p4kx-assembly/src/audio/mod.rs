// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The audio of an Archive: the audio triggers Entities name, the ATL definitions that map them
//! to Wwise events and the soundbanks that hold those events.

mod atl;
mod atl_definitions;
mod atl_trigger_origins;
mod banks;
mod breaks;
mod cache;
mod entity;
mod error;
mod event_sources;
mod export;
mod export_preflight;
mod hirc_diagnostic_sites;
mod hirc_path_source;
mod hirc_path_uses;
mod hirc_route_source;
mod index;
mod index_read;
mod inventory;
mod language;
mod locate;
mod media;
mod music_clip_features;
mod playback_features;
mod read;
mod references;
mod resolve;
mod resolve_bounded;
mod resolve_options;
mod search;
mod sounds;
mod terminal_media_origin;
mod txtp;
mod users;
mod walk;
mod walk_banks;
mod wem_location_origin;
mod wem_selection;

pub use p4kx_extraction::sound::wwise_path::ATL_FOLDER;

pub use atl::{AtlTrigger, WwiseEvent};
pub use atl_definitions::AtlDefinitions;
pub use atl_trigger_origins::AtlTriggerOrigins;
pub use banks::IndexedSoundbank;
pub use breaks::{BreakReason, BrokenLink};
pub use cache::ArchiveAudio;
pub use entity::{EntityAudio, EntityTrigger, EventMatch, Unmatched};
pub use error::{AudioFileError, AudioIssue};
pub use event_sources::SoundbankEventSource;
pub(crate) use export::ExportedWemOrigin;
pub use export::{
    ExportFailure, ExportLoopOrigin, ExportLoopOrigins, ExportOptions, ExportReport, ExportState,
    ExportWatch, ExportedSound, SoundFormat,
};
pub use hirc_path_source::{HircPathOrigins, HircPathSource};
pub use index::AudioIndex;
pub use inventory::{
    AudioCandidate, AudioCandidateState, AudioSurvey, AudioSurveyKind, AudioSurveyProof,
};
pub use language::LanguageChoice;
pub use p4kx_audio::wwise_id;
pub use references::{
    PositionedTriggerReference, TriggerOccurrence, TriggerReference, TriggerScope,
    positioned_trigger_references_in, trigger_references, trigger_references_in,
};
pub use resolve_options::ResolveOptions;
pub use search::{MAX_SEARCH_PAGE, SearchPage, TriggerHit};
pub use sounds::{NodeKind, PathStep, SoundVariant, SourceKind, TriggerSounds, WemLocation};
pub use terminal_media_origin::TerminalMediaOrigins;
pub use txtp::{
    TxtpApproximation, TxtpFile, TxtpOrigins, TxtpReport, TxtpSkip, write_txtp,
    write_txtp_with_loop_origins, write_txtp_with_origins,
};
pub use users::TriggerUsers;
pub use wem_location_origin::WemLocationOrigins;
pub use wem_selection::WemSelection;

/// The largest soundbank Entry that is read, 32 MiB: six times the largest live soundbank.
pub const MAX_SOUNDBANK_LEN: u64 = 32 << 20;

/// The largest ATL definition Entry that is read, 8 MiB.
pub const MAX_ATL_LEN: u64 = 8 << 20;

/// The most nodes a text-XML ATL definition may hold.
pub const MAX_ATL_NODES: u32 = 1 << 20;

/// The most audio triggers one index holds; once it is full, further triggers are left out and
/// the ATL definition that brings the first of them is reported.
pub const MAX_AUDIO_TRIGGERS: usize = 1 << 18;

/// The language folder the game reads when its settings name none.
pub const DEFAULT_LANGUAGE: &str = "English(US)";

/// The most loose WEM Entries one index holds; further ones are left out and the first of them is
/// reported.
pub const MAX_LOOSE_WEMS: usize = 1 << 22;

/// The most language folders one index holds; WEMs in further ones are left out.
pub const MAX_LANGUAGES: usize = 1 << 10;
