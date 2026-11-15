// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::Archive;
use p4kx_audio::{BankMusicStinger, HircObject, HircValue};

use crate::audio::HircPathSource;
use crate::audio::banks::{StingerDeclaration, music_node};
use crate::audio::hirc_path_source::{HircInboundSource, HircRouteSource};
use crate::audio::walk_banks::Loaded;

#[derive(Clone, Copy)]
pub(super) enum FollowedEdge {
    Child(usize),
    Action,
    Stinger {
        trigger: u32,
        declaration: Option<StingerDeclaration>,
    },
}

impl FollowedEdge {
    pub(super) fn stinger_scheduling<'bank>(
        self,
        bank: &'bank Loaded<'_>,
        segment: u32,
    ) -> Option<(&'bank HircObject, &'bank BankMusicStinger)> {
        let Self::Stinger {
            trigger,
            declaration: Some(declaration),
        } = self
        else {
            return None;
        };
        let owner = bank.bank.objects().get(declaration.object)?;
        let node = music_node(&owner.value)?;
        let row = node.stinger_scheduling_evidence(declaration.row)?;
        (row.trigger_id.value == trigger && row.segment_id.value == segment).then_some((owner, row))
    }

    pub(super) fn source(
        self,
        archive: &Archive,
        bank: &Loaded<'_>,
        owner: &HircObject,
        parent: Option<&HircPathSource>,
        bank_id: Option<u32>,
    ) -> Option<HircInboundSource> {
        let route = match self {
            Self::Child(ordinal) => {
                if !matches!(owner.value, HircValue::Event(_)) {
                    return parent?
                        .child(&owner.value, ordinal)
                        .map(HircInboundSource::Child);
                }
                HircRouteSource::event_action(archive, bank, owner, ordinal)
            }
            Self::Action | Self::Stinger { .. } => {
                HircRouteSource::action(archive, bank, owner, bank_id)
            }
        }?;
        Some(HircInboundSource::Route(Arc::new(route)))
    }
}
