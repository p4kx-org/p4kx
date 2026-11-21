// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{CatalogRecordCoverage, CatalogRecordSelection, QualityDefinitionSnapshot};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::crafting::ledger::QualityDependencies;
use crate::crafting_quantization::QUANTIZATIONS;
use crate::quality_catalog::contract::QualityQuantizationResources;

/// Selected inverse membership rows and the recorded coverage of their original dependencies.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct QuantizationResourceSnapshot {
    /// Canonical rows, original evidence and selected quantizer coverage.
    pub projection: QualityDefinitionSnapshot<QualityQuantizationResources>,
    /// Survey coverage of additional quantizers and `ResourceType` relationships.
    pub dependency_coverage: CatalogRecordCoverage,
}

/// Supplies the canonical resource membership of selected physical quantizers.
/// Publish owned changes after outer success and review of both coverage reports.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_quantization_resources<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(QuantizationResourceSnapshot) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_quality_quantization_resources(selection, cancel, inspect)
}

/// Supplies the same membership rows from a decoded Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence or cancellation errors.
pub fn with_quantization_resources_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(QuantizationResourceSnapshot) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_quality_quantization_resources(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical inverse membership for this session's selected quantizers.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_quality_quantization_resources<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(QuantizationResourceSnapshot) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_quality_quantization_resources(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies inverse membership while retaining this view's DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_quality_quantization_resources<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(QuantizationResourceSnapshot) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let origins = CatalogOrigins::for_archive_records(archive, entry)?;
            let input = OriginalCatalogInput::new(forge, &origins);
            let result = QualityDependencies::read(forge, &input, selection, cancel)?;
            let table = CatalogTable::new(forge, &[QUANTIZATIONS.type_name], result.rows);
            let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
            checkpoint(cancel)?;
            Ok(inspect(QuantizationResourceSnapshot {
                projection: QualityDefinitionSnapshot {
                    build: archive.build().cloned(),
                    availability: table.availability,
                    rows: table.rows,
                    source_inventory,
                    coverage: result.coverage,
                },
                dependency_coverage: result.dependency_coverage,
            }))
        })
    }
}
