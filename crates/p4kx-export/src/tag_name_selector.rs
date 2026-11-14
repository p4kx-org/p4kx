// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, SourceEvidenceError, SourceUse, SourceUseRole};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::OriginalReferenceObservation;
use crate::{CatalogError, ExportAccountingError};

use super::{CapturedTags, PathReceipts};

/// A genuine Record reference's pending comparison in its paired Tag tree.
#[derive(Debug)]
pub(crate) struct TagNameSelector {
    paths: Arc<PathReceipts>,
    guid: Guid,
    absence: SourceUse,
    negative_complete: bool,
}

impl<'source> CapturedTags<'_, 'source> {
    pub(crate) fn name_selector(
        &self,
        observed: &OriginalReferenceObservation<'_, '_, '_, 'source>,
    ) -> Result<TagNameSelector, CatalogError> {
        observed.caller().check_input(self.input)?;
        let reference = observed.reference();
        if reference.location().is_some() {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        }
        let [absence] = self.absence.as_slice() else {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        };
        Ok(TagNameSelector {
            paths: Arc::clone(&self.paths),
            guid: reference.guid(),
            absence: absence.clone(),
            negative_complete: self.negative_complete,
        })
    }
}

impl TagNameSelector {
    pub(crate) fn matches(
        &self,
        name: &str,
        origins: &CatalogOrigins,
        into: &mut Vec<SourceUse>,
    ) -> Result<bool, CatalogError> {
        if !self.paths.uses.is_same_inventory(&origins.uses) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        let (equal, selected, role) = self.decision(name)?;
        let input = origins
            .dataforge
            .at(selected.description().location().clone(), role)?;
        into.try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        into.push(input);
        Ok(equal)
    }

    fn decision(&self, name: &str) -> Result<(bool, &SourceUse, SourceUseRole), CatalogError> {
        let Some(tag) = self.paths.tree.get(self.guid) else {
            let role = if self.negative_complete {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            };
            return Ok((false, &self.absence, role));
        };
        let node = self
            .paths
            .get(tag.position())
            .ok_or(ExportAccountingError::ConflictingTagTrace)?;
        if node.guid != tag.guid() {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        }
        let selected = node.name_input();
        let role = if selected.role() == SourceUseRole::Attempted {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        Ok((tag.name() == name, selected, role))
    }
}
