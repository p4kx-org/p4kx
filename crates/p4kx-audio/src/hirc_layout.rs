// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! How far the reader decodes a `HIRC` object, and its place in the hierarchy.

use crate::{BankError, HircKind, HircObject, HircValue, SoundSource};

/// How far this reader decodes a `HIRC` object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HircLayout {
    /// The fields of the type were read; containers and music objects must also use the whole
    /// body. State objects report the remaining body range.
    Decoded,
    /// The reader has no decoder for the type, so the body is kept raw.
    Undecoded,
    /// The reader decodes the type, but not in this bank version.
    Unsupported,
    /// The body does not hold the fields of its type.
    Malformed,
}

/// Returns whether the reader decodes objects of `kind` in some bank version.
pub(crate) const fn has_decoder(kind: HircKind) -> bool {
    matches!(
        kind,
        HircKind::State
            | HircKind::Sound
            | HircKind::Action
            | HircKind::Event
            | HircKind::RandomSequenceContainer
            | HircKind::SwitchContainer
            | HircKind::LayerContainer
            | HircKind::MusicSegment
            | HircKind::MusicTrack
            | HircKind::MusicSwitch
            | HircKind::MusicPlaylist
    )
}

impl HircKind {
    /// Returns a stable lowercase name with hyphens, e.g. `music-playlist`; `other` for a type
    /// byte this reader does not know.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Sound => "sound",
            Self::Action => "action",
            Self::Event => "event",
            Self::RandomSequenceContainer => "random-sequence-container",
            Self::SwitchContainer => "switch-container",
            Self::ActorMixer => "actor-mixer",
            Self::AudioBus => "audio-bus",
            Self::LayerContainer => "layer-container",
            Self::MusicSegment => "music-segment",
            Self::MusicTrack => "music-track",
            Self::MusicSwitch => "music-switch",
            Self::MusicPlaylist => "music-playlist",
            Self::Attenuation => "attenuation",
            Self::DialogueEvent => "dialogue-event",
            Self::FxShareSet => "fx-share-set",
            Self::FxCustom => "fx-custom",
            Self::AuxiliaryBus => "auxiliary-bus",
            Self::Lfo => "lfo",
            Self::Envelope => "envelope",
            Self::AudioDevice => "audio-device",
            Self::TimeModulator => "time-modulator",
            Self::Other(_) => "other",
        }
    }

    /// Returns the kinds this reader names, in type byte order.
    #[must_use = "the kinds are only listed by iterating"]
    pub fn known() -> impl Iterator<Item = Self> {
        KNOWN.into_iter()
    }

    /// Returns the kind whose [`slug`](Self::slug) is `slug`; `None` for `other` and for text
    /// that names no kind.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::known().find(|kind| kind.slug() == slug)
    }
}

/// Every kind but [`HircKind::Other`], in type byte order.
const KNOWN: [HircKind; 22] = [
    HircKind::State,
    HircKind::Sound,
    HircKind::Action,
    HircKind::Event,
    HircKind::RandomSequenceContainer,
    HircKind::SwitchContainer,
    HircKind::ActorMixer,
    HircKind::AudioBus,
    HircKind::LayerContainer,
    HircKind::MusicSegment,
    HircKind::MusicTrack,
    HircKind::MusicSwitch,
    HircKind::MusicPlaylist,
    HircKind::Attenuation,
    HircKind::DialogueEvent,
    HircKind::FxShareSet,
    HircKind::FxCustom,
    HircKind::AuxiliaryBus,
    HircKind::Lfo,
    HircKind::Envelope,
    HircKind::AudioDevice,
    HircKind::TimeModulator,
];

impl HircObject {
    /// Returns how far the reader decodes the object.
    #[must_use]
    pub const fn layout(&self) -> HircLayout {
        match &self.value {
            HircValue::Raw if has_decoder(self.kind) => HircLayout::Unsupported,
            HircValue::Raw => HircLayout::Undecoded,
            HircValue::Malformed(BankError::UnsupportedObjectLayout { .. }) => {
                HircLayout::Unsupported
            }
            HircValue::Malformed(_) => HircLayout::Malformed,
            _ => HircLayout::Decoded,
        }
    }

    /// Returns the ID of the parent node, 0 for a node at the top; `None` for an object that has
    /// no parent field or is not decoded.
    #[must_use]
    pub const fn parent_id(&self) -> Option<u32> {
        match &self.value {
            HircValue::Sound(source) => source.parent_id,
            HircValue::Container(container) => Some(container.parent_id),
            HircValue::MusicSegment(segment) => Some(segment.node.parent_id),
            HircValue::MusicTrack(track) => Some(track.parent_id),
            HircValue::MusicSwitch(switch) => Some(switch.node.parent_id),
            HircValue::MusicPlaylist(playlist) => Some(playlist.node.parent_id),
            _ => None,
        }
    }

    /// Returns the IDs the object lists, in bank order: the actions of an Event, the children of
    /// a container, the tracks of a segment, the segments of a playlist or the sources of a music
    /// track (media IDs, not object IDs).
    pub fn children(&self) -> impl Iterator<Item = u32> + '_ {
        let (ids, sources): (&[u32], &[SoundSource]) = match &self.value {
            HircValue::Event(actions) => (actions, &[]),
            HircValue::Container(container) => (&container.children, &[]),
            HircValue::MusicSegment(segment) => (&segment.node.children, &[]),
            HircValue::MusicSwitch(switch) => (&switch.node.children, &[]),
            HircValue::MusicPlaylist(playlist) => (&playlist.node.children, &[]),
            HircValue::MusicTrack(track) => (&[], &track.sources),
            _ => (&[], &[]),
        };
        ids.iter()
            .copied()
            .chain(sources.iter().map(|source| source.source_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_kind_is_found_by_its_slug() {
        for (byte, kind) in (1..).zip(HircKind::known()) {
            assert_eq!(HircKind::from_byte(byte), kind, "type byte {byte}");
        }
        assert_eq!(
            HircKind::from_byte(23),
            HircKind::Other(23),
            "no kind follows the table"
        );
        for kind in HircKind::known() {
            assert_eq!(HircKind::from_slug(kind.slug()), Some(kind), "{kind}");
        }
    }

    #[test]
    fn other_and_unknown_text_name_no_kind() {
        for text in ["other", "", "Sound", "music_track", "music-track "] {
            assert_eq!(HircKind::from_slug(text), None, "{text:?}");
        }
    }
}
