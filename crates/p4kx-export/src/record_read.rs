// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, RecordTree, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::WholeInspectionFold;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::record_observation::RecordObservation;
use crate::selection::{Kind, is_kind_leftover};
use crate::source_fields::FieldError;
use crate::{CatalogError, CatalogReadFailure};

#[path = "record_read_cursor.rs"]
mod cursor;

pub(crate) use self::cursor::{KindReadAccounting, visit_kind_by_record_guid};

// Several Records per worker amortize the join and bound pending results.
const RECORDS_PER_WORKER: usize = 64;

/// Returns the number of Records held for one parallel read window.
pub(crate) fn window_size(records: usize) -> usize {
    rayon::current_num_threads()
        .saturating_mul(RECORDS_PER_WORKER)
        .min(records)
        .max(1)
}

fn kind_window_size(kind: Kind, records: usize) -> usize {
    match kind {
        Kind::ContractTemplates | Kind::MissionBrokerEntries | Kind::MissionLocationTemplates => {
            rayon::current_num_threads().min(records).max(1)
        }
        _ => window_size(records),
    }
}

/// The entities of one kind of a DataForge and what was left out.
pub(crate) struct KindRead<'f, T> {
    pub(crate) items: Vec<T>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) ledger: ExportLedger,
}

/// A Record that cannot be read: the error of the DataForge or of an identifier of a description.
pub(crate) type ReadError = Box<dyn std::error::Error + Send + Sync>;

/// A selected Record projection and the diagnostics of its typed fields.
pub(crate) struct RecordProjection<T> {
    pub(crate) value: T,
    pub(crate) notes: Vec<FieldError>,
}

enum Piece<'f, T> {
    Item(Box<T>),
    Skipped,
    Failed(CatalogReadFailure<'f>),
}

struct Observed<'f, T> {
    piece: Piece<'f, T>,
    source: SourceObservation,
    fields: ExportLedger,
}

/// Reads the Records of `kind` that `filter` matches with `build`, ordered by the GUID `guid`
/// returns; a development leftover is left out and counted, a Record that cannot be read is
/// reported as a failure.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled, or a source-evidence error
/// when the Forge or canonical read does not belong to the admitted input.
pub(crate) fn read_kind<'f, T: Send>(
    forge: &'f DataForge<'f>,
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    (filter, cancel, inspections): (
        Option<&Filter>,
        Option<&CancelToken>,
        Option<&mut WholeInspectionFold<'_>>,
    ),
    build: impl Fn(&Record<'f>, &RecordTree<'f>) -> Result<T, ReadError> + Sync,
    guid: impl Fn(&T) -> CatalogGuid,
) -> Result<KindRead<'f, T>, CatalogError> {
    read_kind_with_notes(
        forge,
        input,
        kind,
        (filter, cancel, inspections),
        |admission, tree| {
            let value = build(admission.record(), tree)?;
            Ok(RecordProjection {
                value,
                notes: Vec::new(),
            })
        },
        guid,
    )
}

/// Reads selected Records and records each typed field diagnosis on its original admission.
pub(crate) fn read_kind_with_notes<'f, T: Send>(
    forge: &'f DataForge<'f>,
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    (filter, cancel, inspections): (
        Option<&Filter>,
        Option<&CancelToken>,
        Option<&mut WholeInspectionFold<'_>>,
    ),
    build: impl Fn(
        &RecordObservation<'_, 'f>,
        &RecordTree<'f>,
    ) -> Result<RecordProjection<T>, ReadError>
    + Sync,
    guid: impl Fn(&T) -> CatalogGuid,
) -> Result<KindRead<'f, T>, CatalogError> {
    let mut items = Vec::new();
    let read = cursor::visit_kind_with_notes(
        forge,
        input,
        kind,
        (filter, cancel, inspections),
        cursor::ReadOrder::Physical,
        build,
        |value| {
            items.push(value);
            Ok(())
        },
    )?;
    sort_by_guid(&mut items, guid);
    Ok(read.into_read(items))
}

fn piece<'f, T>(
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    build: &impl Fn(
        &RecordObservation<'_, 'f>,
        &RecordTree<'f>,
    ) -> Result<RecordProjection<T>, ReadError>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Observed<'f, T>>, CatalogError> {
    let rule = kind.records();
    let selection: Result<Option<bool>, p4kx_extraction::DataForgeError> = (|| {
        let path = record.record_path()?;
        if record.type_name()? != rule.type_name {
            return Ok(None);
        }
        Ok(Some(is_kind_leftover(kind, path, record.name()?)))
    })();
    let picked = match selection {
        Ok(None) => return Ok(None),
        Ok(Some(false)) => return project(input, kind, trees, record, build, cancel).map(Some),
        Ok(Some(true)) => {
            let admission = RecordObservation::capture(input, record)?;
            Observed {
                piece: Piece::Skipped,
                source: admission.source(
                    Some(kind),
                    ExportSourceRole::Selected,
                    SourceDisposition::SkippedBeforeRead,
                )?,
                fields: ExportLedger::default(),
            }
        }
        Err(error) => {
            let admission = RecordObservation::capture(input, record)?;
            failed(
                kind,
                &admission,
                &error,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )?
        }
    };
    Ok(Some(picked))
}

fn project<'f, T>(
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    build: &impl Fn(
        &RecordObservation<'_, 'f>,
        &RecordTree<'f>,
    ) -> Result<RecordProjection<T>, ReadError>,
    cancel: Option<&CancelToken>,
) -> Result<Observed<'f, T>, CatalogError> {
    let admission = RecordObservation::capture(input, record)?;
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return failed(
                kind,
                &admission,
                &error,
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
            );
        }
    };
    let mut fields = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        Some(kind),
        ExportSourceRole::Selected,
        cancel,
    )?;
    let mut observed = match build(&admission, &tree) {
        Ok(projected) => {
            crate::source_projection_notes::record(
                kind,
                ExportSourceRole::Selected,
                &admission,
                &tree,
                projected.notes,
                &mut fields,
            )?;
            Observed {
                piece: Piece::Item(Box::new(projected.value)),
                source: admission.source(
                    Some(kind),
                    ExportSourceRole::Selected,
                    SourceDisposition::Projected,
                )?,
                fields: ExportLedger::default(),
            }
        }
        Err(error) => match error.downcast::<CatalogError>() {
            Ok(error) => return Err(*error),
            Err(error) => failed(
                kind,
                &admission,
                error.as_ref(),
                ExportSourceRole::Selected,
                SourceDisposition::ProjectionFailed,
            )?,
        },
    };
    observed.fields = fields;
    Ok(observed)
}

fn failed<'f, T>(
    kind: Kind,
    admission: &RecordObservation<'_, 'f>,
    error: &(dyn std::error::Error + 'static),
    role: ExportSourceRole,
    disposition: SourceDisposition,
) -> Result<Observed<'f, T>, CatalogError> {
    Ok(Observed {
        piece: Piece::Failed(CatalogReadFailure::of(admission.record(), error)),
        source: admission
            .source(Some(kind), role, disposition)?
            .diagnosed(crate::export_source_failure::diagnosis(error)),
        fields: ExportLedger::default(),
    })
}
#[cfg(test)]
#[path = "record_read_original_tests.rs"]
mod original_tests;

#[cfg(test)]
#[path = "record_read_callback_error_tests.rs"]
mod callback_error_tests;

#[cfg(test)]
#[path = "record_read_path_tests.rs"]
mod path_boundary_tests;

#[cfg(test)]
#[path = "record_read_inspections_tests.rs"]
mod inspection_tests;
