// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The audio of an Archive: the audio triggers Entities name, the ATL definitions that map them
//! to Wwise events and the soundbanks that hold those events.

mod atl;
mod banks;
mod breaks;
mod cache;
mod entity;
mod error;
mod export;
mod index;
mod index_read;
mod language;
mod locate;
mod media;
mod read;
mod references;
mod resolve;
mod search;
mod sounds;
mod txtp;
mod users;
mod walk;
mod walk_banks;
mod wem_selection;

pub use p4kx_extraction::sound::wwise_path::ATL_FOLDER;

pub use atl::{AtlTrigger, WwiseEvent};
pub use banks::IndexedSoundbank;
pub use breaks::{BreakReason, BrokenLink};
pub use cache::ArchiveAudio;
pub use entity::{EntityAudio, EntityTrigger, EventMatch, Unmatched};
pub use error::{AudioFileError, AudioIssue};
pub use export::{
    ExportFailure, ExportOptions, ExportReport, ExportState, ExportWatch, ExportedSound,
    SoundFormat,
};
pub use index::AudioIndex;
pub use language::LanguageChoice;
pub use p4kx_audio::wwise_id;
pub use references::{TriggerReference, TriggerScope, trigger_references, trigger_references_in};
pub use search::{MAX_SEARCH_PAGE, SearchPage, TriggerHit};
pub use sounds::{NodeKind, PathStep, SoundVariant, SourceKind, TriggerSounds, WemLocation};
pub use txtp::{TxtpApproximation, TxtpFile, TxtpReport, TxtpSkip, write_txtp};
pub use users::TriggerUsers;
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

/// The most objects a container path holds, the Event object included.
pub const MAX_PATH_DEPTH: usize = 32;

/// The most `PlayEvent` and `Trigger` actions that follow one another on one path.
pub const MAX_ACTION_CHAIN: usize = 8;

/// The most children of one container that are followed.
pub const MAX_CONTAINER_CHILDREN: usize = 1024;

/// The most objects followed from one Wwise event in one soundbank.
pub const MAX_EVENT_OBJECTS: usize = 1 << 14;

/// The most objects followed for one audio trigger over all its events and soundbanks; the share
/// of each event in each soundbank shrinks once the product exceeds it.
pub const MAX_TRIGGER_OBJECTS: usize = 1 << 21;
