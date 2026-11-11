// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::BankHircState;
use crate::BankSoundStateRead;
use crate::bank_source_actor_mixer::{self, BankHircActorMixer};
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

use super::sound_states;
use super::{
    BankHirc, BankHircAction, BankHircActionProperties, BankHircObject, BankHircSoundNode,
};

/// A physical HIRC occurrence and any selected bounded interpretation.
#[non_exhaustive]
pub enum BankHircSemantic<'input, R> {
    /// Physical object whose semantic fields are outside this iterator's coverage.
    Raw(BankHircObject),
    /// Owner-bound Action interpretation.
    Action(BankHircAction),
    /// Owner-bound Play action with checked property rows.
    ActionWithProperties(Box<BankHircActionProperties<'input, R>>),
    /// Owner-bound Event interpretation.
    Event(BankHircEvent<'input, R>),
    /// Owner-bound Sound interpretation.
    Sound(Box<BankHircSound>),
    /// Owner-bound Sound interpretation and inline State observations.
    SoundWithStates {
        /// Original Sound interpretation.
        sound: Box<BankHircSound>,
        /// Disposition of the checked inline State region.
        states: BankSoundStateRead<'input, R>,
    },
    /// Owner-bound Sound and sequential source cursors for its node chunks.
    SoundWithNode(BankHircSoundNode<'input, R>),
    /// Owner-bound standalone State interpretation.
    State(BankHircState),
    /// Owner-bound container interpretation and ordered children.
    Container(Box<BankHircContainer<'input, R>>),
    /// Owner-bound `ActorMixer` node regions and ordered children.
    ActorMixer(Box<BankHircActorMixer<'input, R>>),
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
            Self::ActionWithProperties(action) => &action.action().object,
            Self::Event(event) => &event.object,
            Self::Sound(sound) | Self::SoundWithStates { sound, .. } => &sound.object,
            Self::SoundWithNode(node) => &node.sound().object,
            Self::State(state) => &state.object,
            Self::Container(container) => &container.object,
            Self::ActorMixer(actor) => actor.object(),
            Self::MusicPlaylist(playlist) => &playlist.object,
            Self::MusicSegment(segment) => &segment.object,
            Self::MusicSwitch(switch) => &switch.object,
            Self::MusicTrack(track) => &track.object,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeMode {
    Raw,
    States,
    StatesAndRtpc,
    SourceFields,
}

impl<R: Read + Seek> BankHirc<'_, R> {
    /// Reads the next physical occurrence with its supported semantic fields.
    ///
    /// The observed bank header must belong to the same input.
    pub fn next_semantic(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        self.next_semantic_in(header, NodeMode::Raw)
    }

    /// Reads semantic fields including standalone and inline State observations.
    ///
    /// The observed bank header must belong to the same input.
    pub fn next_semantic_with_states(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        self.next_semantic_in(header, NodeMode::States)
    }

    /// Reads semantic fields with standalone State and checked Sound node regions.
    ///
    /// The observed bank header must belong to the same input.
    pub fn next_semantic_with_node_fields(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        self.next_semantic_in(header, NodeMode::StatesAndRtpc)
    }

    /// Reads semantic fields with Sound node regions and Play action property rows.
    ///
    /// The observed bank header must belong to the same input.
    pub fn next_semantic_with_source_fields(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        self.next_semantic_in(header, NodeMode::SourceFields)
    }

    fn next_semantic_in(
        &mut self,
        header: &BankObservedHeader,
        mode: NodeMode,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(cause) => return Some(Err(cause)),
        };
        Some(self.semantic(object, version, mode))
    }

    fn semantic(
        &mut self,
        object: BankHircObject,
        version: u32,
        mode: NodeMode,
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
        self.primitive(object, kind, version, mode)
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
        mode: NodeMode,
    ) -> Result<BankHircSemantic<'_, R>, BankReadError> {
        let result = match kind {
            HircKind::Event => {
                return bank_source_event::observe(self.input, &mut self.stopped, object, version)
                    .map(BankHircSemantic::Event);
            }
            HircKind::Action if mode == NodeMode::SourceFields => {
                return self.action_with_properties(object, version);
            }
            HircKind::Action => self.action(object, version).map(BankHircSemantic::Action),
            HircKind::ActorMixer => {
                return bank_source_actor_mixer::observe(
                    self.input,
                    &mut self.stopped,
                    object,
                    version,
                )
                .map(|actor| BankHircSemantic::ActorMixer(Box::new(actor)));
            }
            HircKind::State if mode != NodeMode::Raw => {
                self.state(object, version).map(BankHircSemantic::State)
            }
            HircKind::Sound if hirc_source::has_sound_layout(version) => {
                return self.sound_semantic(object, version, mode);
            }
            _ => Ok(BankHircSemantic::Raw(object)),
        };
        if result.is_err() {
            self.stopped = true;
        }
        result
    }

    fn action_with_properties(
        &mut self,
        object: BankHircObject,
        version: u32,
    ) -> Result<BankHircSemantic<'_, R>, BankReadError> {
        let mut properties = None;
        let action = match self.action_observed(object, version, &mut properties) {
            Ok(action) => action,
            Err(cause) => {
                self.stopped = true;
                return Err(cause);
            }
        };
        Ok(match properties {
            Some(fields) => BankHircSemantic::ActionWithProperties(Box::new(
                BankHircActionProperties::new(self.input, action, fields),
            )),
            None => BankHircSemantic::Action(action),
        })
    }

    fn sound_semantic(
        &mut self,
        object: BankHircObject,
        version: u32,
        mode: NodeMode,
    ) -> Result<BankHircSemantic<'_, R>, BankReadError> {
        let sound = match bank_source_sound::observe(self.input, object, version) {
            Ok(sound) => Box::new(sound),
            Err(cause) => {
                self.stopped = true;
                return Err(cause);
            }
        };
        match (mode, version) {
            (NodeMode::States, hirc_source::NODE_BASE_VERSION) => {
                let states = sound_states::read(self.input, &sound);
                Ok(BankHircSemantic::SoundWithStates { sound, states })
            }
            (NodeMode::StatesAndRtpc | NodeMode::SourceFields, hirc_source::NODE_BASE_VERSION) => {
                Ok(BankHircSemantic::SoundWithNode(BankHircSoundNode::new(
                    self.input, sound,
                )))
            }
            _ => Ok(BankHircSemantic::Sound(sound)),
        }
    }
}
