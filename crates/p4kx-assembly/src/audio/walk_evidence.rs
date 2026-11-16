// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::HircObject;

use crate::audio::hirc_path_source::{
    HircArrival, HircInboundSource, HircPathSource, PathSourceMode,
};
use crate::audio::walk_banks::Loaded;

use super::Walk;
use super::children::{FollowedEdge, StingerSchedule};

impl Walk<'_, '_> {
    pub(super) fn path_source(
        &self,
        owner: &Loaded<'_>,
        object: &HircObject,
        edge: Option<FollowedEdge>,
        bank_id: Option<u32>,
        stinger: Option<StingerSchedule<'_>>,
    ) -> Option<HircPathSource> {
        if matches!(self.path_sources, PathSourceMode::Omit) {
            return None;
        }
        let archive = self.banks.archive();
        let arrival = edge.map_or(HircArrival::Start, |edge| {
            let mut inbound = self.inbound_source(edge, bank_id);
            if let Some(HircInboundSource::Stinger { schedule, .. }) = &mut inbound {
                *schedule = stinger.and_then(|row| row.source(archive, owner));
            }
            HircArrival::Link(inbound)
        });
        Some(HircPathSource::of(
            archive,
            owner,
            object,
            arrival,
            stinger.and_then(StingerSchedule::diagnostic),
        ))
    }

    pub(super) fn inbound_source(
        &self,
        edge: FollowedEdge,
        bank_id: Option<u32>,
    ) -> Option<HircInboundSource> {
        if matches!(self.path_sources, PathSourceMode::Omit) {
            return None;
        }
        let parent = self.path.last()?;
        edge.source(
            self.banks.archive(),
            &parent.owner,
            parent.object()?,
            parent.source.as_ref(),
            bank_id,
        )
    }
}
