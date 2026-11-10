// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{HircKind, HircValue, MusicStinger, Soundbank};

use super::AudioFileError;
use super::language::bank_language;

/// A soundbank Entry of the audio index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedSoundbank {
    path: Box<str>,
    id: u32,
    language_id: u32,
    event_count: u32,
    pub(crate) entry: usize,
}

impl IndexedSoundbank {
    /// Returns the path of the soundbank Entry.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the file name of the soundbank Entry, e.g. `SSAM_ORIG_890J.bnk`.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.path.rsplit(['\\', '/']).next().unwrap_or_default()
    }

    /// Returns the language folder the soundbank lies in, e.g. `English(US)`, or `None` for a
    /// soundbank that is not localised.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        bank_language(&self.path)
    }

    /// Returns the soundbank ID its `BKHD` section states.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the language ID its `BKHD` section states.
    #[must_use]
    pub const fn language_id(&self) -> u32 {
        self.language_id
    }

    /// Returns how many Event objects its `HIRC` section holds.
    #[must_use]
    pub const fn event_count(&self) -> u32 {
        self.event_count
    }
}

/// A WEM a soundbank embeds: its ID, its offset in the `DATA` section and its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EmbeddedMedia {
    pub(crate) id: u32,
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

/// What the audio index keeps of one soundbank.
#[derive(Debug)]
pub(crate) struct BankContents {
    pub(crate) soundbank: IndexedSoundbank,
    pub(crate) events: Vec<u32>,
    pub(crate) media: Vec<EmbeddedMedia>,
    pub(crate) stingers: Vec<MusicStinger>,
}

/// Parses the soundbank at the Archive position `entry` and returns it with the IDs of its Event
/// objects and the WEMs it embeds.
pub(crate) fn read(path: &str, entry: usize, bytes: &[u8]) -> Result<BankContents, AudioFileError> {
    let bank = Soundbank::parse(bytes)?;
    let events: Vec<u32> = bank
        .objects()
        .iter()
        .filter(|object| object.kind == HircKind::Event)
        .map(|object| object.id)
        .collect();
    let media = bank
        .wems()
        .map(|wem| EmbeddedMedia {
            id: wem.id(),
            offset: wem.offset(),
            len: u64::try_from(wem.bytes().len()).unwrap_or(u64::MAX),
        })
        .collect();
    let stingers = bank
        .objects()
        .iter()
        .flat_map(|object| match &object.value {
            HircValue::MusicSegment(segment) => &segment.node.stingers[..],
            HircValue::MusicSwitch(switch) => &switch.node.stingers[..],
            HircValue::MusicPlaylist(playlist) => &playlist.node.stingers[..],
            _ => &[],
        })
        .filter(|stinger| stinger.segment_id != 0)
        .copied()
        .collect();
    let header = bank.header();
    Ok(BankContents {
        soundbank: IndexedSoundbank {
            path: path.into(),
            id: header.id,
            language_id: header.language_id,
            event_count: u32::try_from(events.len()).unwrap_or(u32::MAX),
            entry,
        },
        events,
        media,
        stingers,
    })
}
