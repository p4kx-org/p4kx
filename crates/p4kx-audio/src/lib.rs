// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads the Wwise sounds of Star Citizen Archives from bytes: parses WEMs (the RIFF or RIFX
//! structure and the Wwise `fmt` variants), detects the codec, writes PCM, ADPCM, Wwise Vorbis
//! and Opus sounds as WAV, rebuilds Wwise Vorbis sounds as Ogg Vorbis and repackages Opus sounds
//! as Ogg Opus into a writer the caller supplies; parses BNK soundbanks with their sections, their
//! `HIRC` objects and the WEMs they embed.

mod adpcm;
mod bank_error;
mod bank_inline_state_rows;
mod bank_node_property_rows;
mod bank_object_fields;
mod bank_parse;
mod bank_reader;
mod bank_rtpc_rows;
mod bank_source;
mod bank_source_actor_mixer;
mod bank_source_container;
mod bank_source_didx;
mod bank_source_event;
mod bank_source_hirc;
mod bank_source_music_playlist;
mod bank_source_music_segment;
mod bank_source_music_switch;
mod bank_source_music_track;
mod bank_source_sound;
mod bank_stid;
mod bank_structure;
mod codec;
mod error;
mod hirc;
mod hirc_action;
mod hirc_actor_mixer;
mod hirc_container;
mod hirc_event;
mod hirc_layout;
mod hirc_music;
mod hirc_music_node;
mod hirc_music_parse;
mod hirc_music_playlist;
mod hirc_music_playlist_depth;
mod hirc_music_segment;
mod hirc_music_switch;
mod hirc_music_switch_traversal;
mod hirc_music_track;
mod hirc_music_transition;
mod hirc_node;
mod hirc_node_property;
mod hirc_node_state;
mod hirc_plugin;
mod hirc_rtpc;
mod hirc_source;
mod hirc_state;
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
mod wwise_id;

pub use bank_error::{BankError, BankWarning, SectionId};
pub use bank_inline_state_rows::{BankInlineStateRow, BankInlineStateRows};
pub use bank_node_property_rows::{
    BankNodePropertyRow, BankNodePropertyRows, BankNodePropertyValue,
};
pub use bank_object_fields::BankHircField;
pub use bank_rtpc_rows::{BankRtpcRow, BankRtpcRows};
pub use bank_source::{
    BankObservedHeader, BankReadError, BankSectionCursor, BankSource, BankSourceSection,
};
pub use bank_source_actor_mixer::BankHircActorMixer;
pub use bank_source_container::{
    BankContainerChild, BankContainerChildren, BankContainerRead, BankHircContainer,
};
pub use bank_source_didx::{BankDidx, BankDidxEntry, BankMediaInfo};
pub use bank_source_event::{BankEvent, BankEventAction, BankEventRead, BankHircEvent};
pub use bank_source_hirc::BankHircSoundNode;
pub use bank_source_hirc::{
    BankActionRead, BankHirc, BankHircAction, BankHircObject, BankHircRtpc, BankHircSemantic,
    BankHircSoundStates, BankHircState, BankRtpcRead, BankSoundStateRead, BankStateRead,
};
pub use bank_source_music_playlist::{
    BankHircMusicPlaylist, BankMusicPlaylistDepth, BankMusicPlaylistFields, BankMusicPlaylistRead,
    BankMusicPlaylistRow, BankMusicPlaylistRows,
};
pub use bank_source_music_segment::{
    BankHircMusicSegment, BankMusicMarker, BankMusicSegmentRead, BankMusicSegmentRow,
    BankMusicSegmentRows,
};
pub use bank_source_music_switch::{
    BankHircMusicSwitch, BankMusicSwitchRead, BankMusicSwitchRow, BankMusicSwitchRows,
};
pub use bank_source_music_track::{BankHircMusicTrack, BankMusicTrackRead, BankMusicTrackRows};
pub use bank_source_sound::{BankHircSound, BankSoundRead};
pub use bank_stid::{BankStid, BankStidName};
pub use codec::Codec;
pub use error::AudioError;
pub use hirc::{HircKind, HircObject, HircValue, MediaStorage, SoundSource};
pub use hirc_action::{
    ActionTarget, BankActionFields, BankPlayBankDecision, BankPlayBankFields, PlayBank,
};
pub use hirc_actor_mixer::{ActorMixer, BankActorMixerFields};
pub use hirc_container::BankContainerFields;
pub use hirc_event::BankEventCountEncoding;
pub use hirc_layout::HircLayout;
pub use hirc_music::{
    MusicClip, MusicGroupKind, MusicMarker, MusicMeter, MusicNode, MusicPlayMode, MusicPlaylist,
    MusicPlaylistItem, MusicSegment, MusicStinger, MusicSwitch, MusicTrack, MusicTrackKind,
    MusicTrackSwitch,
};
pub use hirc_music_node::{BankMusicNodeFields, BankMusicStinger};
pub use hirc_music_playlist::BankMusicPlaylistItem;
pub use hirc_music_segment::BankMusicSegmentFields;
pub use hirc_music_switch::{BankMusicSwitchFields, BankMusicSwitchGroup, BankMusicSwitchNode};
pub use hirc_music_switch_traversal::{BankMusicSwitchBranch, BankMusicSwitchTarget};
pub use hirc_music_track::{
    BankMusicTrackClip, BankMusicTrackFields, BankMusicTrackRow, BankMusicTrackSource,
    BankMusicTrackSwitchFields,
};
pub use hirc_music_transition::BankMusicTransitionFields;
pub use hirc_node::{BankNodeParentFields, Container, ContainerKind};
pub use hirc_node_property::{
    BankNodePropertyExtent, BankNodePropertyExtents, BankNodePropertyKind,
};
pub use hirc_node_state::{BankStateBinding, BankStateGroup};
pub use hirc_rtpc::{BankRtpcCurve, BankRtpcExtent, BankRtpcPoint};
pub use hirc_source::{
    BankMediaSourceFields, BankSoundContinuation, BankSoundFields, BankSoundNodeBase,
    BankSoundNodeFields,
};
pub use hirc_state::{StateObject, StateProperties, StateProperty};
pub use loops::Loop;
pub use media_class::{BankDeclaredLengthSource, BankMediaClassSource, MediaClass};
pub use ogg::OggWriter;
pub use opus::{OpusError, WemOpusOgg, WemOpusRanges, WemOpusWav, WemOpusWriteError};
pub use riff::{ByteOrder, WemSourceChunk};
pub use soundbank::{BankHeader, EmbeddedWem, Section, Soundbank};
pub use vorbis::{
    Codebooks, OggReport, VorbisError, WemVorbisOgg, WemVorbisRanges, WemVorbisWav,
    WemVorbisWriteError,
};
pub use wav::WavReport;
pub use wem::{Wem, WemChunk};
pub use wem_cue::{WemCue, WemCuePoint};
pub use wem_info_source::{
    WemCodecSource, WemInfoSource, WemLoopEnd, WemLoopSource, WemSampleCountSource,
};
pub use wem_list::{WemCueTextRegion, WemList, WemListItem, WemListPayload};
pub use wem_metadata::WemMetadata;
pub use wem_source::{
    WemChunkCursor, WemChunkEntry, WemInspection, WemPcmWav, WemPcmWriteError, WemReadError,
    WemSource,
};
pub use wem_text::{WemCueText, WemTextUtf8};
pub use wwise_id::wwise_id;
