// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, SourceUse};

use crate::catalog_read::original::{EntryCapture, EntryReceipt};
use crate::loot_soc::TagSite;
use crate::{CatalogError, ExportAccountingError};

struct TagSelection {
    member: usize,
    site: TagSite,
}

/// First accepted `TagId` attributes from verified SOC members in stored Entry order.
#[derive(Default)]
pub(super) struct TagEvidence {
    members: Vec<EntryReceipt>,
    first: HashMap<Guid, TagSelection>,
}

impl TagEvidence {
    pub(super) fn retain_member(
        &mut self,
        receipt: &EntryReceipt,
        candidate: HashMap<Guid, TagSite>,
    ) -> Result<(), CatalogError> {
        let member = self.members.len();
        let mut selected = false;
        for (guid, site) in candidate {
            if self.first.contains_key(&guid) {
                continue;
            }
            self.first
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            self.first.insert(guid, TagSelection { member, site });
            selected = true;
        }
        if selected {
            self.members
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            self.members.push(receipt.clone());
        }
        Ok(())
    }

    pub(super) fn bind(
        self,
        capture: &EntryCapture<'_>,
    ) -> Result<HashMap<Guid, SourceUse>, CatalogError> {
        let mut origins = HashMap::new();
        origins
            .try_reserve(self.first.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for (guid, selected) in self.first {
            let receipt = self
                .members
                .get(selected.member)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            origins.insert(guid, receipt.soc_tag_id(capture, selected.site)?);
        }
        Ok(origins)
    }
}
