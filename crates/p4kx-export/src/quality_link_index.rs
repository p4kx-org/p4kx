// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, Guid};

use crate::CatalogError;

use super::resource_links::ResourceLinks;
use super::{check_cancel, evidence::Evidence};

#[derive(Default)]
pub(super) struct Member {
    pub(super) decision: Evidence,
    pub(super) values: Evidence,
}

pub(super) type Members = BTreeMap<Guid, Member>;

/// Original resource inventory and exact matching members indexed by authored target.
pub(super) struct Links {
    by_target: BTreeMap<Guid, Members>,
    pub(super) inventory: Evidence,
}

impl Links {
    pub(super) fn read(
        resources: Vec<ResourceLinks>,
        mut inventory: Evidence,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut by_target = BTreeMap::<Guid, Members>::new();
        for resource in resources {
            check_cancel(cancel)?;
            inventory.append(&resource.inventory)?;
            for (target, evidence) in resource.targets {
                check_cancel(cancel)?;
                let member = by_target
                    .entry(target)
                    .or_default()
                    .entry(resource.resource)
                    .or_default();
                member.decision.append(&resource.admission)?;
                member.decision.append(&evidence)?;
                member.values.keep(resource.guid_source.clone())?;
            }
        }
        Ok(Self {
            by_target,
            inventory,
        })
    }

    pub(super) fn take(&mut self, guid: Guid) -> Members {
        self.by_target.remove(&guid).unwrap_or_default()
    }

    pub(super) fn unresolved(&self) -> usize {
        self.by_target.values().map(BTreeMap::len).sum()
    }
}
