// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForgeError, Record, RecordTree, TreeBuilder};

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::{CatalogError, Kind};

/// A dependency's actual canonical read and its production observations.
pub(crate) struct DependencyRead<'f> {
    pub(crate) tree: Result<RecordTree<'f>, DataForgeError>,
    pub(crate) ledger: ExportLedger,
}

/// Reads one dependency and inspects owned field values before optional projection helpers.
pub(crate) fn read<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    kind: Kind,
    cancel: Option<&CancelToken>,
) -> Result<DependencyRead<'f>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let tree = trees.build(record);
    let mut ledger = ExportLedger::default();
    let observation = match &tree {
        Ok(tree) => {
            ledger.merge(crate::export_field_walk::inspect(
                tree,
                record,
                Some(kind),
                ExportSourceRole::Dependency,
                cancel,
            )?);
            SourceObservation::new(
                kind,
                record,
                ExportSourceRole::Dependency,
                SourceDisposition::Projected,
            )
        }
        Err(error) => SourceObservation::new(
            kind,
            record,
            ExportSourceRole::Dependency,
            SourceDisposition::ReadFailed,
        )
        .diagnosed(crate::export_source_failure::diagnosis(error)),
    };
    ledger.observe(observation);
    Ok(DependencyRead { tree, ledger })
}
