// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircActorMixer, BankHircContainerNode, BankHircSemantic, BankHircSoundNode,
    BankInlineStateRows, BankNodeAuxFields, BankNodeFxKind, BankNodeFxSlots, BankNodeParentFields,
    BankNodePlaybackFields, BankNodePropertyKind, BankNodePropertyRows, BankNodeUserAuxBuses,
    BankPositioningRows, BankPositioningTableKind, BankReadError, BankRtpcRead, BankRtpcRows,
    BankSharedNode, BankSoundStateRead,
};

pub(super) enum NodeReader<'source, 'input, R> {
    Sound(&'source mut BankHircSoundNode<'input, R>),
    Actor(&'source mut BankHircActorMixer<'input, R>),
    Container(&'source mut BankHircContainerNode<'input, R>),
    Music(BankSharedNode<'source, R>),
}

impl<'source, 'input, R> NodeReader<'source, 'input, R> {
    pub(super) fn new(observed: &'source mut BankHircSemantic<'input, R>) -> Option<Self> {
        Some(match observed {
            BankHircSemantic::SoundWithNode(node) => Self::Sound(node),
            BankHircSemantic::ActorMixer(node) => Self::Actor(node),
            BankHircSemantic::ContainerWithNode(node) => Self::Container(node),
            BankHircSemantic::MusicSegment(node) => Self::Music(node.shared_node()),
            BankHircSemantic::MusicTrack(node) => Self::Music(node.shared_node()),
            BankHircSemantic::MusicSwitch(node) => Self::Music(node.shared_node()),
            BankHircSemantic::MusicPlaylist(node) => Self::Music(node.shared_node()),
            _ => return None,
        })
    }

    pub(super) fn parent(&self) -> Option<&BankNodeParentFields> {
        match self {
            Self::Sound(node) => sound_base(node).map(|base| &base.fields),
            Self::Actor(node) => Some(&node.fields().node),
            Self::Container(node) => Some(&node.fields().node),
            Self::Music(node) => Some(node.parent()),
        }
    }

    pub(super) fn aux(&self) -> Option<&BankNodeAuxFields> {
        match self {
            Self::Sound(node) => sound_base(node).map(|base| &base.aux),
            Self::Actor(node) => Some(&node.fields().aux),
            Self::Container(node) => Some(&node.fields().parameters.aux),
            Self::Music(node) => Some(&node.parameters().aux),
        }
    }

    pub(super) fn playback(&self) -> Option<&BankNodePlaybackFields> {
        match self {
            Self::Sound(node) => sound_base(node).map(|base| &base.playback),
            Self::Actor(node) => Some(&node.fields().playback),
            Self::Container(node) => Some(&node.fields().parameters.playback),
            Self::Music(node) => Some(&node.parameters().playback),
        }
    }

    pub(super) fn fx_slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        match self {
            Self::Sound(node) => node.fx_slots(kind),
            Self::Actor(node) => node.fx_slots(kind),
            Self::Container(node) => node.fx_slots(kind),
            Self::Music(node) => node.fx_slots(kind),
        }
    }

    pub(super) fn user_aux_buses(&mut self) -> Result<Option<BankNodeUserAuxBuses>, BankReadError>
    where
        R: Read + Seek,
    {
        match self {
            Self::Sound(node) => node.user_aux_buses(),
            Self::Actor(node) => node.user_aux_buses(),
            Self::Container(node) => node.user_aux_buses(),
            Self::Music(node) => node.user_aux_buses(),
        }
    }

    pub(super) fn positioning_rows(
        &mut self,
        kind: BankPositioningTableKind,
    ) -> Option<BankPositioningRows<'_, R>> {
        match self {
            Self::Sound(node) => node.positioning_rows(kind),
            Self::Actor(node) => node.positioning_rows(kind),
            Self::Container(node) => node.positioning_rows(kind),
            Self::Music(node) => node.positioning_rows(kind),
        }
    }

    pub(super) fn states(&mut self) -> Option<BankInlineStateRows<'_, R>> {
        match self {
            Self::Sound(node) => match node.states() {
                BankSoundStateRead::Readable(rows) => Some(rows),
                _ => None,
            },
            Self::Actor(node) => node.states(),
            Self::Container(node) => node.states(),
            Self::Music(node) => node.states(),
        }
    }

    pub(super) fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        match self {
            Self::Sound(node) => match node.rtpc() {
                BankRtpcRead::Readable(rows) | BankRtpcRead::Malformed { rows, .. } => Some(rows),
                _ => None,
            },
            Self::Actor(node) => node.rtpc(),
            Self::Container(node) => node.rtpc(),
            Self::Music(node) => node.rtpc(),
        }
    }

    pub(super) fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        match self {
            Self::Sound(node) => node.properties(kind),
            Self::Actor(node) => node.properties(kind),
            Self::Container(node) => node.properties(kind),
            Self::Music(node) => node.properties(kind),
        }
    }
}

fn sound_base<'node, R>(
    node: &'node BankHircSoundNode<'_, R>,
) -> Option<&'node p4kx_audio::BankSoundNodeBase> {
    node.sound().fields.node.as_ref()?.base.as_ref()
}
