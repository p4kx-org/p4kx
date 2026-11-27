// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use p4kx_extraction::Guid;

use crate::CatalogError;

use super::evidence::Evidence;

#[derive(Default)]
pub(crate) struct Fallbacks {
    resources: BTreeMap<Guid, Vec<Member>>,
}

struct Member {
    quantizer: Arc<Evidence>,
    relationship: Evidence,
}

impl Fallbacks {
    pub(super) fn retain(
        &mut self,
        guid: Guid,
        quantizer: &Arc<Evidence>,
        relationship: Evidence,
    ) -> Result<(), CatalogError> {
        let members = self.resources.entry(guid).or_default();
        members
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        members.push(Member {
            quantizer: Arc::clone(quantizer),
            relationship,
        });
        Ok(())
    }

    pub(crate) fn targets(&self) -> impl Iterator<Item = Guid> + '_ {
        self.resources.keys().copied()
    }

    pub(crate) fn evidence(&self, resource: Guid) -> Option<impl Iterator<Item = &Evidence>> {
        self.resources.get(&resource).map(|members| {
            members
                .iter()
                .flat_map(|member| [member.quantizer.as_ref(), &member.relationship])
        })
    }
}
