// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::ops::Range;

use crate::bank_reader::SectionReader;
use crate::bank_structure::{HircEnvelope, OBJECT_HEADER_LEN, OBJECT_ID_LEN};
use crate::hirc_action::{self, ActionTarget};
use crate::hirc_container;
use crate::hirc_event;
use crate::hirc_layout::has_decoder;
use crate::hirc_music::{MusicPlaylist, MusicSegment, MusicSwitch, MusicTrack};
use crate::hirc_music_parse;
use crate::hirc_music_track;
use crate::hirc_node::Container;
use crate::hirc_source;
use crate::hirc_state::{self, StateObject};
use crate::{BankError, SectionId};

const LAST_SOUND_WITHOUT_CACHE_ID_VERSION: u32 = 150;
pub(crate) const CONTAINER_VERSION: u32 = 154;
const LAST_KNOWN_VERSION: u32 = 154;
const LAST_OLD_TYPE_NUMBERING_VERSION: u32 = 126;
const PLUGIN_TYPE_MASK: u32 = 0x0F;
const LANGUAGE_SPECIFIC_BIT: u8 = 0x01;
const SOURCE_PLUGIN_TYPE: u32 = 2;

/// The type of a `HIRC` object, from its type byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HircKind {
    /// A state, type 1.
    State,
    /// A sound playing one media source, type 2.
    Sound,
    /// An action such as play or stop, type 3.
    Action,
    /// An event running a list of actions, type 4.
    Event,
    /// A random or sequence container, type 5.
    RandomSequenceContainer,
    /// A switch container, type 6.
    SwitchContainer,
    /// An actor-mixer, type 7.
    ActorMixer,
    /// An audio bus, type 8.
    AudioBus,
    /// A blend or layer container, type 9.
    LayerContainer,
    /// A music segment, type 10.
    MusicSegment,
    /// A music track, type 11.
    MusicTrack,
    /// A music switch container, type 12.
    MusicSwitch,
    /// A music playlist container, type 13.
    MusicPlaylist,
    /// An attenuation, type 14.
    Attenuation,
    /// A dialogue event, type 15.
    DialogueEvent,
    /// An effect share set, type 16.
    FxShareSet,
    /// A custom effect, type 17.
    FxCustom,
    /// An auxiliary bus, type 18.
    AuxiliaryBus,
    /// A low-frequency oscillator, type 19.
    Lfo,
    /// An envelope, type 20.
    Envelope,
    /// An audio device, type 21.
    AudioDevice,
    /// A time modulator, type 22.
    TimeModulator,
    /// A type byte this reader does not know.
    Other(u8),
}

impl fmt::Display for HircKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RandomSequenceContainer => f.write_str("random or sequence container"),
            Self::SwitchContainer => f.write_str("switch container"),
            Self::LayerContainer => f.write_str("blend container"),
            Self::Other(byte) => write!(f, "{byte:#04x}"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl HircKind {
    /// Returns the kind of the type byte `kind` in a bank of `version`.
    ///
    /// Up to version 126 the types from `0x10` on are numbered differently: `0x10` and `0x11` are
    /// a feedback bus and a feedback node, which this reader reports as [`HircKind::Other`], and
    /// `0x12` to `0x17` are the effect share set, custom effect, auxiliary bus, LFO, envelope and
    /// audio device that later versions number `0x10` to `0x15`.
    #[must_use]
    pub const fn of(version: u32, kind: u8) -> Self {
        if version > LAST_OLD_TYPE_NUMBERING_VERSION {
            return Self::from_byte(kind);
        }
        match kind {
            0x10 | 0x11 => Self::Other(kind),
            0x12..=0x17 => Self::from_byte(kind - 2),
            other => Self::from_byte(other),
        }
    }

    /// Returns the kind of the type byte `kind` as banks of version 128 and later number it.
    #[must_use]
    pub const fn from_byte(kind: u8) -> Self {
        match kind {
            1 => Self::State,
            2 => Self::Sound,
            3 => Self::Action,
            4 => Self::Event,
            5 => Self::RandomSequenceContainer,
            6 => Self::SwitchContainer,
            7 => Self::ActorMixer,
            8 => Self::AudioBus,
            9 => Self::LayerContainer,
            10 => Self::MusicSegment,
            11 => Self::MusicTrack,
            12 => Self::MusicSwitch,
            13 => Self::MusicPlaylist,
            14 => Self::Attenuation,
            15 => Self::DialogueEvent,
            16 => Self::FxShareSet,
            17 => Self::FxCustom,
            18 => Self::AuxiliaryBus,
            19 => Self::Lfo,
            20 => Self::Envelope,
            21 => Self::AudioDevice,
            22 => Self::TimeModulator,
            other => Self::Other(other),
        }
    }
}

/// Where the media of a sound is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaStorage {
    /// Embedded in a soundbank, storage 0.
    Embedded,
    /// Its start embedded in a soundbank, the rest streamed from a loose WEM, storage 1.
    PrefetchStreamed,
    /// Streamed from a loose WEM, storage 2.
    Streamed,
    /// A storage value this reader does not know.
    Other(u8),
}

impl MediaStorage {
    pub(crate) const fn from_byte(storage: u8) -> Self {
        match storage {
            0 => Self::Embedded,
            1 => Self::PrefetchStreamed,
            2 => Self::Streamed,
            other => Self::Other(other),
        }
    }
}

/// The media source of a Sound object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundSource {
    /// The Wwise plug-in ID of the codec, e.g. `0x0004_0001` for Vorbis.
    pub plugin_id: u32,
    /// Where the media is stored.
    pub storage: MediaStorage,
    /// The WEM ID of the media.
    pub source_id: u32,
    /// The size of the media in memory as the bank states it.
    pub in_memory_size: u32,
    /// The source bits after the size: bit 0 marks the media as language-specific.
    pub source_bits: u8,
    /// The ID of the parent node in the hierarchy, from the node base parameters of a Sound
    /// object in bank version 154; `None` for a music track source and in other versions.
    pub parent_id: Option<u32>,
}

impl SoundSource {
    /// Returns whether the source is a plug-in that generates the sound, e.g. silence or a tone,
    /// so that no WEM holds it.
    #[must_use]
    pub const fn is_source_plugin(&self) -> bool {
        self.plugin_id & PLUGIN_TYPE_MASK == SOURCE_PLUGIN_TYPE
    }

    /// Returns whether the media is language-specific: its WEM lies in the folder of a language,
    /// loose or in a localised soundbank.
    #[must_use]
    pub const fn is_language_specific(&self) -> bool {
        self.source_bits & LANGUAGE_SPECIFIC_BIT != 0
    }
}

/// What this reader decodes from the body of a `HIRC` object.
#[derive(Debug, Clone, PartialEq)]
pub enum HircValue {
    /// A standalone State's original property bundle.
    State(Box<StateObject>),
    /// A Sound object's media source, from bank version 150 on.
    Sound(SoundSource),
    /// An Action object's type and target.
    Action(ActionTarget),
    /// An Event object's actions, in order.
    Event(Vec<u32>),
    /// A random, sequence, switch or blend container's children, in bank version 154.
    Container(Container),
    /// A music segment, in bank version 154.
    MusicSegment(Box<MusicSegment>),
    /// A music track, in bank version 154.
    MusicTrack(Box<MusicTrack>),
    /// A music switch container, in bank version 154.
    MusicSwitch(Box<MusicSwitch>),
    /// A music playlist container, in bank version 154.
    MusicPlaylist(Box<MusicPlaylist>),
    /// Any other object, or a Sound or container of another bank version; its body is kept raw.
    Raw,
    /// A Sound, Action, Event, container or music object whose body does not hold its fields; its
    /// body is kept raw.
    Malformed(BankError),
}

/// One object of the `HIRC` section, with its ID and its decoded value.
#[derive(Debug, Clone, PartialEq)]
pub struct HircObject {
    /// The type of the object.
    pub kind: HircKind,
    /// The object ID.
    pub id: u32,
    /// What this reader decodes from the body.
    pub value: HircValue,
    pub(crate) body: Range<usize>,
}

impl HircObject {
    /// Returns why the body could not be decoded, for a [`HircValue::Malformed`] object.
    #[must_use]
    pub const fn error(&self) -> Option<&BankError> {
        match &self.value {
            HircValue::Malformed(error) => Some(error),
            _ => None,
        }
    }
}

/// The objects of a `HIRC` section and the number of bytes after the last of them.
pub(crate) struct Hirc {
    pub(crate) objects: Vec<HircObject>,
    pub(crate) trailing: usize,
}

/// Parses the body of the `HIRC` section, found at `base` in a bank of `version`.
pub(crate) fn parse(version: u32, body: &[u8], base: usize) -> Result<Hirc, BankError> {
    let mut reader = SectionReader::new(SectionId::HIRC, body);
    let count = reader.u32()?;
    let capacity = usize::try_from(count)
        .unwrap_or(usize::MAX)
        .min(reader.remaining() / (OBJECT_HEADER_LEN + OBJECT_ID_LEN));
    let mut objects = Vec::with_capacity(capacity);
    for _ in 0..count {
        let header = HircEnvelope::read(&mut reader)?;
        let kind = HircKind::of(version, header.type_code);
        let len = usize::try_from(header.body_len).unwrap_or(usize::MAX);
        let range = reader.skip(len)?;
        let object = body.get(range.clone()).unwrap_or_default();
        let mut fields = SectionReader::new(SectionId::HIRC, object);
        let id = fields.u32()?;
        let bank_offset = base + range.start;
        let value = value(kind, version, &mut fields, bank_offset as u64)
            .unwrap_or_else(HircValue::Malformed);
        objects.push(HircObject {
            kind,
            id,
            value,
            body: bank_offset..base + range.end,
        });
    }
    Ok(Hirc {
        objects,
        trailing: reader.remaining(),
    })
}

/// Returns whether a Sound object or a music track playlist item of a bank of `version` carries
/// a `cacheID` before its in-memory media size.
pub(crate) const fn has_cache_id(version: u32) -> bool {
    version > LAST_SOUND_WITHOUT_CACHE_ID_VERSION
}

/// Returns whether an object of `kind` is one this reader decodes but whose layout in a bank of
/// `version` is not known, because the version is newer than 154.
pub(crate) const fn has_unknown_layout(kind: HircKind, version: u32) -> bool {
    version > LAST_KNOWN_VERSION && has_decoder(kind)
}

/// Decodes the version-qualified fields after an object's ID.
fn value(
    kind: HircKind,
    version: u32,
    fields: &mut SectionReader<'_>,
    bank_offset: u64,
) -> Result<HircValue, BankError> {
    if has_unknown_layout(kind, version) {
        return Err(BankError::UnsupportedObjectLayout { kind, version });
    }
    Ok(match kind {
        HircKind::State if hirc_state::supports_standalone(version) => {
            HircValue::State(Box::new(hirc_state::read_object(fields, bank_offset)?))
        }
        HircKind::Sound if hirc_source::has_sound_layout(version) => {
            HircValue::Sound(hirc_source::sound(version, fields)?)
        }
        HircKind::Action => HircValue::Action(hirc_action::action(version, fields)?),
        HircKind::Event => HircValue::Event(hirc_event::read(version, fields)?),
        HircKind::RandomSequenceContainer
        | HircKind::SwitchContainer
        | HircKind::LayerContainer
            if version == CONTAINER_VERSION =>
        {
            HircValue::Container(hirc_container::container(kind, fields)?)
        }
        HircKind::MusicSegment if version == CONTAINER_VERSION => {
            HircValue::MusicSegment(Box::new(hirc_music_parse::segment(fields)?))
        }
        HircKind::MusicTrack if version == CONTAINER_VERSION => {
            HircValue::MusicTrack(Box::new(hirc_music_track::track(version, fields)?))
        }
        HircKind::MusicSwitch if version == CONTAINER_VERSION => {
            HircValue::MusicSwitch(Box::new(hirc_music_parse::switch(fields)?))
        }
        HircKind::MusicPlaylist if version == CONTAINER_VERSION => {
            HircValue::MusicPlaylist(Box::new(hirc_music_parse::playlist(fields)?))
        }
        _ => HircValue::Raw,
    })
}
