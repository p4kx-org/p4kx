// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_source_container::{self, BankHircContainer};
use crate::bank_source_event;
use crate::bank_source_music_playlist::{self, BankHircMusicPlaylist};
use crate::bank_source_music_segment::{self, BankHircMusicSegment};
use crate::bank_source_music_switch::{self, BankHircMusicSwitch};
use crate::bank_source_music_track::{self, BankHircMusicTrack};
use crate::bank_source_sound::{self, BankHircSound};
use crate::hirc::CONTAINER_VERSION;
use crate::hirc_container::Layout as ContainerLayout;
use crate::hirc_source;
use crate::{BankHircEvent, BankObservedHeader, BankReadError, HircKind};

use super::{BankHirc, BankHircAction, BankHircObject};

/// A physical HIRC occurrence and any selected bounded interpretation.
#[non_exhaustive]
pub enum BankHircSemantic<'input, R> {
    /// Physical object whose semantic fields are outside this iterator's coverage.
    Raw(BankHircObject),
    /// Owner-bound Action interpretation.
    Action(BankHircAction),
    /// Owner-bound Event interpretation.
    Event(BankHircEvent<'input, R>),
    /// Owner-bound Sound interpretation.
    Sound(Box<BankHircSound>),
    /// Owner-bound container interpretation and ordered children.
    Container(Box<BankHircContainer<'input, R>>),
    /// Owner-bound music playlist interpretation and ordered values.
    MusicPlaylist(Box<BankHircMusicPlaylist<'input, R>>),
    /// Owner-bound music segment interpretation and ordered values.
    MusicSegment(Box<BankHircMusicSegment<'input, R>>),
    /// Owner-bound music switch interpretation and original decision-tree rows.
    MusicSwitch(Box<BankHircMusicSwitch<'input, R>>),
    /// Owner-bound music track interpretation and ordered source rows.
    MusicTrack(Box<BankHircMusicTrack<'input, R>>),
}

impl<R> BankHircSemantic<'_, R> {
    /// Returns the original physical object envelope.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        match self {
            Self::Raw(object) => object,
            Self::Action(action) => &action.object,
            Self::Event(event) => &event.object,
            Self::Sound(sound) => &sound.object,
            Self::Container(container) => &container.object,
            Self::MusicPlaylist(playlist) => &playlist.object,
            Self::MusicSegment(segment) => &segment.object,
            Self::MusicSwitch(switch) => &switch.object,
            Self::MusicTrack(track) => &track.object,
        }
    }
}

impl<R: Read + Seek> BankHirc<'_, R> {
    /// Reads the next physical occurrence with its supported semantic fields.
    ///
    /// The observed bank header must belong to the same input.
    pub fn next_semantic(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(cause) => return Some(Err(cause)),
        };
        Some(self.semantic(object, version))
    }

    fn semantic(
        &mut self,
        object: BankHircObject,
        version: u32,
    ) -> Result<BankHircSemantic<'_, R>, BankReadError> {
        let kind = object.kind(version);
        if version >= CONTAINER_VERSION {
            match kind {
                HircKind::MusicPlaylist => {
                    return bank_source_music_playlist::observe(
                        self.input,
                        &mut self.stopped,
                        object,
                        version,
                    )
                    .map(|playlist| BankHircSemantic::MusicPlaylist(Box::new(playlist)));
                }
                HircKind::MusicSegment => {
                    return bank_source_music_segment::observe(
                        self.input,
                        &mut self.stopped,
                        object,
                        version,
                    )
                    .map(|segment| BankHircSemantic::MusicSegment(Box::new(segment)));
                }
                HircKind::MusicSwitch => return self.music_switch(object, version),
                HircKind::MusicTrack => {
                    return bank_source_music_track::observe(
                        self.input,
                        &mut self.stopped,
                        object,
                        version,
                    )
                    .map(|track| BankHircSemantic::MusicTrack(Box::new(track)));
                }
                _ => {}
            }
            if let Some(layout) = ContainerLayout::of(kind) {
                return bank_source_container::observe(
                    self.input,
                    &mut self.stopped,
                    object,
                    version,
                    kind,
                    layout,
                )
                .map(|container| BankHircSemantic::Container(Box::new(container)));
            }
        }
        self.primitive(object, kind, version)
    }

    fn music_switch(
        &mut self,
        object: BankHircObject,
        version: u32,
    ) -> Result<BankHircSemantic<'_, R>, BankReadError> {
        bank_source_music_switch::observe(self.input, &mut self.stopped, object, version)
            .map(|switch| BankHircSemantic::MusicSwitch(Box::new(switch)))
    }

    fn primitive(
        &mut self,
        object: BankHircObject,
        kind: HircKind,
        version: u32,
    ) -> Result<BankHircSemantic<'_, R>, BankReadError> {
        let result = match kind {
            HircKind::Event => {
                return bank_source_event::observe(self.input, &mut self.stopped, object, version)
                    .map(BankHircSemantic::Event);
            }
            HircKind::Action => self.action(object, version).map(BankHircSemantic::Action),
            HircKind::Sound if hirc_source::has_sound_layout(version) => {
                bank_source_sound::observe(self.input, object, version)
                    .map(|sound| BankHircSemantic::Sound(Box::new(sound)))
            }
            _ => Ok(BankHircSemantic::Raw(object)),
        };
        if result.is_err() {
            self.stopped = true;
        }
        result
    }
}
