// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads the Wwise sounds of Star Citizen Archives from bytes: parses WEMs (the RIFF or RIFX
//! structure and the Wwise `fmt` variants), detects the codec, writes PCM, ADPCM, Wwise Vorbis
//! and Opus sounds as WAV, rebuilds Wwise Vorbis sounds as Ogg Vorbis and repackages Opus sounds
//! as Ogg Opus into a writer the caller supplies; parses BNK soundbanks with their sections, their
//! `HIRC` objects and the WEMs they embed.

mod adpcm;
mod bank_error;
mod bank_object_fields;
mod bank_parse;
mod bank_reader;
mod bank_source;
mod bank_source_didx;
mod bank_source_event;
mod bank_source_hirc;
mod bank_stid;
mod bank_structure;
mod codec;
mod error;
mod hirc;
mod hirc_action;
mod hirc_event;
mod hirc_layout;
mod hirc_music;
mod hirc_music_parse;
mod hirc_node;
mod hirc_plugin;
mod hirc_source;
mod ima;
mod loops;
mod media_class;
mod ogg;
mod opus;
mod ptadpcm;
mod read_at;
mod riff;
mod soundbank;
mod vorbis;
mod wav;
mod wem;
mod wem_cue;
mod wem_info_source;
mod wem_list;
mod wem_metadata;
mod wem_metadata_read;
mod wem_source;
mod wem_text;

pub use bank_error::{BankError, BankWarning, SectionId};
pub use bank_object_fields::BankHircField;
pub use bank_source::{
    BankObservedHeader, BankReadError, BankSectionCursor, BankSource, BankSourceSection,
};
pub use bank_source_didx::{BankDidx, BankDidxEntry, BankMediaInfo};
pub use bank_source_event::{BankEvent, BankEventAction, BankEventRead, BankHircEvent};
pub use bank_source_hirc::{
    BankActionRead, BankHirc, BankHircAction, BankHircObject, BankHircSemantic,
};
pub use bank_stid::{BankStid, BankStidName};
pub use codec::Codec;
pub use error::AudioError;
pub use hirc::{HircKind, HircObject, HircValue, MediaStorage, SoundSource};
pub use hirc_action::{
    ActionTarget, BankActionFields, BankPlayBankDecision, BankPlayBankFields, PlayBank,
};
pub use hirc_event::BankEventCountEncoding;
pub use hirc_layout::HircLayout;
pub use hirc_music::{
    MusicClip, MusicGroupKind, MusicMarker, MusicMeter, MusicNode, MusicPlayMode, MusicPlaylist,
    MusicPlaylistItem, MusicSegment, MusicStinger, MusicSwitch, MusicTrack, MusicTrackKind,
    MusicTrackSwitch,
};
pub use hirc_node::{Container, ContainerKind};
pub use loops::Loop;
pub use media_class::{BankDeclaredLengthSource, BankMediaClassSource, MediaClass};
pub use ogg::OggWriter;
pub use opus::OpusError;
pub use riff::{ByteOrder, WemSourceChunk};
pub use soundbank::{BankHeader, EmbeddedWem, Section, Soundbank};
pub use vorbis::{Codebooks, OggReport, VorbisError};
pub use wav::WavReport;
pub use wem::{Wem, WemChunk};
pub use wem_cue::{WemCue, WemCuePoint};
pub use wem_info_source::{
    WemCodecSource, WemInfoSource, WemLoopEnd, WemLoopSource, WemSampleCountSource,
};
pub use wem_list::{WemCueTextRegion, WemList, WemListItem, WemListPayload};
pub use wem_metadata::WemMetadata;
pub use wem_source::{WemChunkCursor, WemChunkEntry, WemReadError, WemSource};
pub use wem_text::{WemCueText, WemTextUtf8};
