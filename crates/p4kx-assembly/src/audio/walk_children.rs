// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::Archive;
use p4kx_audio::{HircObject, HircValue};

use crate::audio::HircPathSource;
use crate::audio::hirc_path_source::{HircInboundSource, HircRouteSource};
use crate::audio::walk_banks::{Loaded, select_holders};

use super::{BreakReason, MAX_CONTAINER_CHILDREN, Via, Walk};

pub(super) enum FollowedEdge<'a> {
    Child {
        owner: &'a HircObject,
        ordinal: usize,
    },
    Action(&'a HircObject),
}

impl FollowedEdge<'_> {
    pub(super) fn source(
        self,
        archive: &Archive,
        bank: &Loaded<'_>,
        parent: Option<&HircPathSource>,
        bank_id: Option<u32>,
    ) -> Option<HircInboundSource> {
        let route = match self {
            Self::Child { owner, ordinal } => {
                if !matches!(owner.value, HircValue::Event(_)) {
                    return parent?
                        .selection_child(&owner.value, ordinal)
                        .map(HircInboundSource::Selection);
                }
                HircRouteSource::event_action(archive, bank, owner, ordinal)
            }
            Self::Action(owner) => HircRouteSource::action(archive, bank, owner, bank_id),
        }?;
        Some(HircInboundSource::Route(Arc::new(route)))
    }
}

impl Walk<'_, '_> {
    pub(super) fn children(&mut self, ids: &[u32], owner: &HircObject) {
        for (ordinal, &id) in ids.iter().take(MAX_CONTAINER_CHILDREN).enumerate() {
            self.reach(
                id,
                None,
                Via::Local,
                Some(FollowedEdge::Child { owner, ordinal }),
            );
        }
        if ids.len() > MAX_CONTAINER_CHILDREN {
            self.broke(BreakReason::TooManyChildren {
                count: ids.len(),
                limit: MAX_CONTAINER_CHILDREN,
            });
        }
    }

    pub(super) fn stingers(&mut self, trigger: u32, action: &HircObject) {
        let stingers = self.index.stingers(trigger);
        if stingers.is_empty() {
            self.broke(BreakReason::Missing { id: trigger });
            return;
        }
        let mut banks: Vec<u32> = stingers.iter().map(|stinger| stinger.bank).collect();
        banks.sort_unstable();
        banks.dedup();
        let chosen = select_holders(self.index, &banks, self.choice);
        if chosen.is_empty() {
            self.broke(self.choice.no_localisation());
            return;
        }
        let wanted: Vec<_> = stingers
            .iter()
            .filter(|stinger| chosen.contains(&stinger.bank))
            .collect();
        for stinger in wanted.iter().take(MAX_CONTAINER_CHILDREN) {
            self.reach(
                stinger.segment_id,
                None,
                Via::At(stinger.bank),
                Some(FollowedEdge::Action(action)),
            );
        }
        if wanted.len() > MAX_CONTAINER_CHILDREN {
            self.broke(BreakReason::TooManyChildren {
                count: wanted.len(),
                limit: MAX_CONTAINER_CHILDREN,
            });
        }
    }
}
