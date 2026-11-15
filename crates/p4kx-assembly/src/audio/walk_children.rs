// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::Archive;
use p4kx_audio::{HircObject, HircValue};

use crate::audio::HircPathSource;
use crate::audio::hirc_path_source::{HircInboundSource, HircRouteSource};
use crate::audio::walk_banks::Loaded;

#[derive(Clone, Copy)]
pub(super) enum FollowedEdge {
    Child(usize),
    Action,
}

impl FollowedEdge {
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
            Self::Action => HircRouteSource::action(archive, bank, owner, bank_id),
        }?;
        Some(HircInboundSource::Route(Arc::new(route)))
    }
}
