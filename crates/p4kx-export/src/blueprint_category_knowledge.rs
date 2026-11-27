// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};

use p4kx_extraction::{Guid, SourceUse, SourceUseRole, SourceValue};

use crate::blueprint_choice_inputs::BlueprintChoice;
use crate::catalog_origins::CatalogOrigins;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, Kind};

pub(super) struct CategoryKnowledge {
    missing: BTreeMap<Guid, Vec<u32>>,
    unclassified: BTreeMap<Guid, Vec<SourceUse>>,
}

impl CategoryKnowledge {
    pub(super) fn read(
        missing: &HashSet<Guid>,
        wanted: impl Iterator<Item = Guid>,
        sources: &[SourceObservation],
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        let wanted: HashSet<_> = wanted.collect();
        let mut result = Self {
            missing: missing.iter().map(|guid| (*guid, Vec::new())).collect(),
            unclassified: BTreeMap::new(),
        };
        for source in sources {
            if source.kind == Some(Kind::BlueprintCategories)
                && (source.disposition.failed()
                    || matches!(
                        source.disposition,
                        SourceDisposition::SkippedBeforeRead | SourceDisposition::SkippedAfterRead
                    ))
                && let Some(physical) = result.missing.get_mut(&source.guid)
            {
                physical.push(source.record_index);
            } else if source.kind.is_none()
                && source.disposition.failed()
                && matches!(
                    source.role,
                    ExportSourceRole::SelectionCandidate | ExportSourceRole::Dependency
                )
            {
                let guid = source.key()?.guid();
                if wanted.contains(&guid)
                    && let Some(inputs) = source.subject().unclassified_selection_inputs(
                        Kind::BlueprintCategories.records(),
                        &origins.uses,
                    )?
                {
                    result.unclassified.entry(guid).or_default().extend(inputs);
                }
            }
        }
        Ok(result)
    }

    pub(super) fn finish(
        &self,
        category: &mut BlueprintChoice<CatalogGuid>,
        origins: &CatalogOrigins,
    ) -> Result<(), CatalogError> {
        let SourceValue::Present(guid) = &category.state else {
            return Ok(());
        };
        if let Some(inputs) = self.unclassified.get(&guid.0) {
            category.used.extend(inputs.iter().cloned());
            category.complete = false;
        } else if let Some(physical) = self.missing.get(&guid.0) {
            for index in physical {
                let context = origins.record(*index);
                category.used.push(
                    context.observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?,
                );
                category.used.push(
                    context
                        .observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?,
                );
            }
            category.complete &= !physical.is_empty();
            category.state = SourceValue::Absent;
        }
        Ok(())
    }
}
