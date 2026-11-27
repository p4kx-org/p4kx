// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use p4kx_extraction::{AudioError, EmbeddedWem, EntryOrigin};

use super::SoundInfo;
use super::soundbank_provenance::{BankSoundProvenance, BankWemInspection};
use super::wem_header_provenance::WemHeaderEvidenceError;

/// One WEM embedded in a soundbank, as the list of the Audio tab shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankSound {
    id: u32,
    size: u64,
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sound(SoundInfo),
    Prefetch,
    Plugin,
    Midi,
    Unreadable,
}

impl BankSound {
    pub(super) fn of(wem: &EmbeddedWem<'_>) -> (Self, BankWemRow) {
        let (kind, inspection) = if wem.is_prefetch() {
            (Kind::Prefetch, BankWemInspection::Prefetch)
        } else {
            match wem.parse() {
                Ok(parsed) => (
                    Kind::Sound(SoundInfo::of(&parsed)),
                    BankWemInspection::parsed(parsed.metadata()),
                ),
                Err(AudioError::PluginMedia) => (Kind::Plugin, BankWemInspection::Plugin),
                Err(AudioError::MidiMedia) => (
                    Kind::Midi,
                    BankWemInspection::Midi {
                        riff: wem.declared_len().is_some(),
                    },
                ),
                Err(_) => (Kind::Unreadable, BankWemInspection::Unreadable),
            }
        };
        (
            Self {
                id: wem.id(),
                size: wem.bytes().len() as u64,
                kind,
            },
            BankWemRow {
                inspection,
                proof: OnceLock::new(),
            },
        )
    }

    /// Returns the WEM ID.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the size of the WEM in the soundbank, in bytes.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Returns whether the soundbank holds only the prefetched start of a streamed WEM, which a
    /// loose WEM Entry of the same ID holds whole.
    #[must_use]
    pub const fn is_prefetch(&self) -> bool {
        matches!(self.kind, Kind::Prefetch)
    }

    /// Returns whether the media is Wwise plug-in data, such as a convolution reverb impulse,
    /// which holds no sound to play.
    #[must_use]
    pub const fn is_plugin(&self) -> bool {
        matches!(self.kind, Kind::Plugin)
    }

    /// Returns whether the media is a MIDI file, which holds no sound to decode.
    #[must_use]
    pub const fn is_midi(&self) -> bool {
        matches!(self.kind, Kind::Midi)
    }

    /// Returns whether the WEM can be decoded: it is neither a prefetched start, plug-in media
    /// nor MIDI.
    #[must_use]
    pub const fn is_playable(&self) -> bool {
        !matches!(self.kind, Kind::Prefetch | Kind::Plugin | Kind::Midi)
    }

    /// Returns what the header says about the sound, or `None` for a prefetched start, plug-in
    /// media and media that is no valid WEM.
    #[must_use]
    pub const fn info(&self) -> Option<&SoundInfo> {
        match &self.kind {
            Kind::Sound(info) => Some(info),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct BankWemRow {
    inspection: BankWemInspection,
    proof: OnceLock<Result<Box<BankSoundProvenance>, WemHeaderEvidenceError>>,
}

impl BankWemRow {
    pub(super) fn provenance(
        &self,
        wem: &EmbeddedWem<'_>,
        original: Option<&EntryOrigin>,
    ) -> Result<&BankSoundProvenance, &WemHeaderEvidenceError> {
        self.proof
            .get_or_init(|| {
                BankSoundProvenance::capture(wem, &self.inspection, original).map(Box::new)
            })
            .as_deref()
    }
}
