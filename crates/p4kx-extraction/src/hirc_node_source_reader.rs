// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    BankHircActorMixer, BankHircContainerNode, BankHircSemantic, BankHircSoundNode,
    BankNodeParentFields, BankNodePropertyKind, BankNodePropertyRows, BankSharedNode,
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
            Self::Sound(node) => node
                .sound()
                .fields
                .node
                .as_ref()
                .and_then(|node| node.base.as_ref())
                .map(|base| &base.fields),
            Self::Actor(node) => Some(&node.fields().node),
            Self::Container(node) => Some(&node.fields().node),
            Self::Music(node) => Some(node.parent()),
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
