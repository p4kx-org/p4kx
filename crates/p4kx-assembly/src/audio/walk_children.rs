// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::Archive;
use p4kx_audio::{BankMusicStinger, HircObject, HircValue};

use crate::audio::HircPathSource;
use crate::audio::banks::{StingerDeclaration, music_node};
use crate::audio::hirc_diagnostic_sites::DiagnosticSite;
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

#[derive(Clone, Copy)]
pub(super) struct StingerSchedule<'bank> {
    owner: &'bank HircObject,
    row: &'bank BankMusicStinger,
    ordinal: usize,
}

impl StingerSchedule<'_> {
    pub(super) fn diagnostic(self) -> Option<DiagnosticSite> {
        DiagnosticSite::stinger(self.owner, self.row)
    }

    pub(super) fn source(
        self,
        archive: &Archive,
        bank: &Loaded<'_>,
    ) -> Option<Arc<HircRouteSource>> {
        HircRouteSource::stinger(archive, bank, self.owner, self.row, self.ordinal).map(Arc::new)
    }
}

impl FollowedEdge {
    pub(super) fn stinger_scheduling<'bank>(
        self,
        bank: &'bank Loaded<'_>,
        segment: u32,
    ) -> Option<StingerSchedule<'bank>> {
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
        (row.trigger_id.value == trigger && row.segment_id.value == segment).then_some(
            StingerSchedule {
                owner,
                row,
                ordinal: declaration.row,
            },
        )
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
        let action = Arc::new(route);
        Some(match self {
            Self::Stinger { .. } => HircInboundSource::Stinger {
                action,
                schedule: None,
            },
            Self::Child(_) | Self::Action => HircInboundSource::Route(action),
        })
    }
}
