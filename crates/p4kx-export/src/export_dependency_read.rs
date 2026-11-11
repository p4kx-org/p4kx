// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForgeError, RecordTree, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

/// A dependency's actual canonical read and its production observations.
pub(crate) struct DependencyRead<'f> {
    pub(crate) tree: Result<RecordTree<'f>, DataForgeError>,
    pub(crate) ledger: ExportLedger,
}

/// Reads one dependency and inspects owned field values before optional projection helpers.
pub(crate) fn read<'f>(
    input: Option<&OriginalCatalogInput<'_, '_>>,
    trees: &mut TreeBuilder<'f>,
    admission: &RecordObservation<'_, 'f>,
    kind: Kind,
    cancel: Option<&CancelToken>,
) -> Result<DependencyRead<'f>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let tree = trees.build(admission.record());
    let mut ledger = ExportLedger::default();
    let observation = match &tree {
        Ok(tree) => {
            ledger.merge(crate::export_field_walk::inspect_observed(
                input,
                tree,
                admission,
                Some(kind),
                ExportSourceRole::Dependency,
                cancel,
            )?);
            admission.source(
                Some(kind),
                ExportSourceRole::Dependency,
                SourceDisposition::Projected,
            )?
        }
        Err(error) => admission
            .source(
                Some(kind),
                ExportSourceRole::Dependency,
                SourceDisposition::ReadFailed,
            )?
            .diagnosed(crate::export_source_failure::diagnosis(error)),
    };
    ledger.observe(observation);
    Ok(DependencyRead { tree, ledger })
}
