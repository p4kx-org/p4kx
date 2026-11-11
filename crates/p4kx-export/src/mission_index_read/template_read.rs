// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError};

#[derive(Debug)]
pub(super) struct TemplateReadSource {
    inventory: SourceUses,
    scope: SourceScope,
}

impl TemplateReadSource {
    pub(super) fn new(origins: &CatalogOrigins) -> Self {
        Self {
            inventory: origins.uses.clone(),
            scope: origins.dataforge.clone(),
        }
    }

    pub(super) fn check(&self, origins: &CatalogOrigins) -> Result<(), CatalogError> {
        if !self.inventory.is_same_inventory(&origins.uses)
            || self.scope.source_ref() != origins.dataforge.source_ref()
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(super) fn observe(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self.scope.at(location, role)?)
    }
}

/// An admitted Template Record whose canonical tree read failed.
pub(crate) struct FailedTemplate {
    source: TemplateReadSource,
    record_index: u32,
}

impl FailedTemplate {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
        failure: &CatalogReadFailure<'_>,
    ) -> Result<Self, CatalogError> {
        admission.subject().check_input(input)?;
        input.record_context(admission.record())?;
        input.record_context(failure.record())?;
        let record_index = admission.record().index();
        if failure.record().index() != record_index || failure.canonical_error().is_none() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            source: TemplateReadSource::new(input.origins()),
            record_index,
        })
    }

    pub(crate) fn retain_attempted(
        &self,
        origins: &CatalogOrigins,
        decisions: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        self.source.check(origins)?;
        decisions
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        let attempted = self.source.observe(
            SourceLocation::Subresource {
                namespace: "dataforgeRecordBody".to_owned(),
                indices: vec![u64::from(self.record_index)],
                field: "body".to_owned(),
            },
            SourceUseRole::Attempted,
        )?;
        decisions.push(attempted);
        Ok(())
    }
}
