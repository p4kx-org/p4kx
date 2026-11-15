// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankBusDuckRows, BankHircActorMixer, BankHircBus, BankHircContainerNode, BankHircField,
    BankHircSemantic, BankHircSoundNode, BankInlineStateRows, BankNodeAuxFields, BankNodeFxKind,
    BankNodeFxList, BankNodeFxSlots, BankNodeParentFields, BankNodePlaybackFields,
    BankNodePositioningFields, BankNodePropertyKind, BankNodePropertyRows, BankNodeUserAuxBuses,
    BankPositioningRows, BankPositioningTableKind, BankReadError, BankRtpcRead, BankRtpcRows,
    BankSharedNode, BankSoundStateRead,
};

pub(super) enum NodeReader<'source, 'input, R> {
    Sound(&'source mut BankHircSoundNode<'input, R>),
    Actor(&'source mut BankHircActorMixer<'input, R>),
    Container(&'source mut BankHircContainerNode<'input, R>),
    Music(BankSharedNode<'source, R>),
    Bus(&'source mut BankHircBus<'input, R>),
    UnsupportedBus,
}

impl<'source, 'input, R> NodeReader<'source, 'input, R> {
    pub(super) fn new(
        observed: &'source mut BankHircSemantic<'input, R>,
        version: u32,
    ) -> Option<Self> {
        Some(match observed {
            BankHircSemantic::SoundWithNode(node) => Self::Sound(node),
            BankHircSemantic::ActorMixer(node) => Self::Actor(node),
            BankHircSemantic::ContainerWithNode(node) => Self::Container(node),
            BankHircSemantic::MusicSegment(node) => Self::Music(node.shared_node()),
            BankHircSemantic::MusicTrack(node) => Self::Music(node.shared_node()),
            BankHircSemantic::MusicSwitch(node) => Self::Music(node.shared_node()),
            BankHircSemantic::MusicPlaylist(node) => Self::Music(node.shared_node()),
            BankHircSemantic::Bus(bus) => Self::Bus(bus),
            BankHircSemantic::Raw(object) if is_bus(object.kind(version)) => Self::UnsupportedBus,
            _ => return None,
        })
    }

    pub(super) fn override_bus_id(&self) -> Option<&BankHircField<u32>> {
        if let Self::Bus(bus) = self {
            return bus.fields().override_bus_id.as_ref();
        }
        self.parent()
            .and_then(BankNodeParentFields::override_bus_id)
    }

    pub(super) fn fx(&self, kind: BankNodeFxKind) -> Option<BankNodeFxList<'_>> {
        if let Self::Bus(bus) = self {
            return Some(bus.fields().fx(kind));
        }
        self.parent().map(|parent| parent.fx(kind))
    }

    pub(super) fn positioning(&self) -> Option<&BankNodePositioningFields> {
        if let Self::Bus(bus) = self {
            return Some(&bus.fields().positioning);
        }
        self.playback().map(|fields| &fields.positioning)
    }

    pub(super) fn ducks(&mut self) -> Option<BankBusDuckRows<'_, R>> {
        match self {
            Self::Bus(bus) => bus.ducks(),
            _ => None,
        }
    }

    pub(super) fn parent(&self) -> Option<&BankNodeParentFields> {
        match self {
            Self::Sound(node) => sound_base(node).map(|base| &base.fields),
            Self::Actor(node) => Some(&node.fields().node),
            Self::Container(node) => Some(&node.fields().node),
            Self::Music(node) => Some(node.parent()),
            Self::Bus(_) | Self::UnsupportedBus => None,
        }
    }

    pub(super) fn aux(&self) -> Option<&BankNodeAuxFields> {
        match self {
            Self::Sound(node) => sound_base(node).map(|base| &base.aux),
            Self::Actor(node) => Some(&node.fields().aux),
            Self::Container(node) => Some(&node.fields().parameters.aux),
            Self::Music(node) => Some(&node.parameters().aux),
            Self::Bus(bus) => Some(&bus.fields().aux),
            Self::UnsupportedBus => None,
        }
    }

    pub(super) fn playback(&self) -> Option<&BankNodePlaybackFields> {
        match self {
            Self::Sound(node) => sound_base(node).map(|base| &base.playback),
            Self::Actor(node) => Some(&node.fields().playback),
            Self::Container(node) => Some(&node.fields().parameters.playback),
            Self::Music(node) => Some(&node.parameters().playback),
            Self::Bus(_) | Self::UnsupportedBus => None,
        }
    }

    pub(super) fn fx_slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        match self {
            Self::Sound(node) => node.fx_slots(kind),
            Self::Actor(node) => node.fx_slots(kind),
            Self::Container(node) => node.fx_slots(kind),
            Self::Music(node) => node.fx_slots(kind),
            Self::Bus(bus) => bus.fx_slots(kind),
            Self::UnsupportedBus => None,
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
            Self::Bus(bus) => bus.user_aux_buses(),
            Self::UnsupportedBus => Ok(None),
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
            Self::Bus(bus) => bus.positioning_rows(kind),
            Self::UnsupportedBus => None,
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
            Self::Bus(bus) => bus.states(),
            Self::UnsupportedBus => None,
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
            Self::Bus(bus) => bus.rtpc(),
            Self::UnsupportedBus => None,
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
            Self::Bus(bus) if kind == BankNodePropertyKind::Properties => bus.properties(),
            Self::Bus(_) | Self::UnsupportedBus => None,
        }
    }
}

pub(super) const fn is_bus(kind: p4kx_audio::HircKind) -> bool {
    matches!(
        kind,
        p4kx_audio::HircKind::AudioBus | p4kx_audio::HircKind::AuxiliaryBus
    )
}

fn sound_base<'node, R>(
    node: &'node BankHircSoundNode<'_, R>,
) -> Option<&'node p4kx_audio::BankSoundNodeBase> {
    node.sound().fields.node.as_ref()?.base.as_ref()
}
