// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, RecordTree, TreeBuilder};
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::{KindRead, Piece, ReadError, RecordProjection, kind_window_size, piece};
use crate::build::original_input::OriginalCatalogInput;
use crate::export_field_inspection::WholeInspectionFold;
use crate::export_ledger::ExportLedger;
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogReadFailure, Kind};

/// Read accounting retained independently of the successful projections.
#[derive(Default)]
pub(crate) struct KindReadAccounting<'source> {
    skipped: usize,
    failures: Vec<CatalogReadFailure<'source>>,
    ledger: ExportLedger,
}

impl<'source> KindReadAccounting<'source> {
    pub(crate) fn into_read<T>(self, items: Vec<T>) -> KindRead<'source, T> {
        KindRead {
            items,
            skipped: self.skipped,
            failures: self.failures,
            ledger: self.ledger,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ReadOrder {
    Physical,
    Guid,
}

/// Visits successful projections in canonical physical GUID order, with physical ties preserved.
///
/// The caller's output order must be the admitted Record GUID.
pub(crate) fn visit_kind_by_record_guid<'source, T: Send>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    selection: Selection<'_, '_>,
    build: impl Fn(&Record<'source>, &RecordTree<'source>) -> Result<T, ReadError> + Sync,
    emit: impl FnMut(T) -> Result<(), CatalogError>,
) -> Result<KindReadAccounting<'source>, CatalogError> {
    visit_kind_with_notes(
        forge,
        input,
        kind,
        selection,
        ReadOrder::Guid,
        |admission, tree| {
            let value = build(admission.record(), tree)?;
            Ok(RecordProjection {
                value,
                notes: Vec::new(),
            })
        },
        emit,
    )
}

pub(crate) type Selection<'read, 'earlier> = (
    Option<&'read Filter>,
    Option<&'read CancelToken>,
    Option<&'read mut WholeInspectionFold<'earlier>>,
);

pub(super) fn visit_kind_with_notes<'f, T: Send>(
    forge: &'f DataForge<'f>,
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    (filter, cancel, mut inspections): Selection<'_, '_>,
    order: ReadOrder,
    build: impl Fn(
        &RecordObservation<'_, 'f>,
        &RecordTree<'f>,
    ) -> Result<RecordProjection<T>, ReadError>
    + Sync,
    mut emit: impl FnMut(T) -> Result<(), CatalogError>,
) -> Result<KindReadAccounting<'f>, CatalogError> {
    input.check_forge(forge)?;
    let rule = kind.records();
    let mut records: Vec<Record<'f>> = forge
        .records()
        .filter(|record| {
            record
                .record_path()
                .is_ok_and(|path| rule.matches_path(path) && filter.is_none_or(|f| f.matches(path)))
                && (!matches!(order, ReadOrder::Guid)
                    || !matches!(record.type_name(), Ok(name) if name != rule.type_name))
        })
        .collect();
    if matches!(order, ReadOrder::Guid) {
        sort_by_guid(&mut records, |record| CatalogGuid(record.guid()));
    }

    let mut read = KindReadAccounting::default();
    read.ledger.inventory(kind);
    let mut pending = Vec::new();
    for window in records.chunks(kind_window_size(kind, records.len())) {
        window
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                piece(input, kind, trees, record, &build, cancel)
            })
            .collect_into_vec(&mut pending);
        #[expect(
            clippy::iter_with_drain,
            reason = "reuse the indexed Rayon result buffer across Record windows"
        )]
        for result in pending.drain(..) {
            let Some(mut observed) = result? else {
                continue;
            };
            if let Some(inspections) = inspections.as_deref_mut() {
                inspections.merge_matching(&mut observed.fields.field_inspections)?;
            }
            read.ledger.observe(observed.source);
            read.ledger.merge(observed.fields);
            match observed.piece {
                Piece::Item(item) => emit(*item)?,
                Piece::Skipped => read.skipped = read.skipped.saturating_add(1),
                Piece::Failed(failure) => read.failures.push(failure),
            }
        }
    }
    Ok(read)
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use p4kx_extraction::{DataForge, Record, RecordTree};
    use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

    use super::visit_kind_by_record_guid;
    use crate::Kind;
    use crate::build::original_input::OriginalCatalogInput;
    use crate::build::original_input::test_support::{self, TestResult};
    use crate::export_diagnostic::ExportSourceRole;
    use crate::export_field_inspection::{FieldInspection, WholeInspectionFold};
    use crate::export_health_records::sources;
    use crate::guid_text::CatalogGuid;
    use crate::record_observation::RecordObservation;
    use crate::record_read::{KindRead, read_kind};

    #[test]
    fn physical_guid_cursor_matches_sorted_rows_and_keeps_equal_guid_occurrences() -> TestResult {
        test_support::with_input(&database(), |forge, input| {
            let kind = Kind::MissionBrokerEntries;
            let build = |record: &Record<'_>, tree: &RecordTree<'_>| {
                Ok((record.index(), CatalogGuid(tree.guid())))
            };
            let expected = read_kind(forge, input, kind, (None, None, None), build, |row| row.1)?;
            let mut actual = Vec::new();
            let accounting =
                visit_kind_by_record_guid(forge, input, kind, (None, None, None), build, |row| {
                    actual.push(row);
                    Ok(())
                })?;
            assert_eq!(
                actual.iter().map(|row| row.0).collect::<Vec<_>>(),
                [1, 2, 0],
                "equal-GUID rows use the original physical tie order"
            );
            assert_eq!(actual, expected.items, "the old physical-GUID row order");
            let actual = accounting.into_read(actual);
            assert_eq!(actual.skipped, expected.skipped, "same policy accounting");
            assert_eq!(
                actual.failures.len(),
                0,
                "the duplicate GUID is not a read failure"
            );
            Ok(())
        })
    }

    #[test]
    fn physical_guid_cursor_keeps_canonical_failure_and_inspection_evidence() -> TestResult {
        test_support::with_input(&database(), |forge, input| {
            let kind = Kind::MissionBrokerEntries;
            let build = |record: &Record<'_>, tree: &RecordTree<'_>| {
                if record.index() == 1 {
                    Ok((record.index(), CatalogGuid(tree.guid())))
                } else {
                    Err(std::io::Error::other("unsupported fixture projection").into())
                }
            };
            let mut original = dependency_inspections(forge, input)?;
            let mut staged = original.clone();
            let expected = read_kind(
                forge,
                input,
                kind,
                (
                    None,
                    None,
                    Some(&mut WholeInspectionFold::new(&mut original)),
                ),
                build,
                |row| row.1,
            )?;
            let mut rows = Vec::new();
            let accounting = visit_kind_by_record_guid(
                forge,
                input,
                kind,
                (None, None, Some(&mut WholeInspectionFold::new(&mut staged))),
                build,
                |row| {
                    rows.push(row);
                    Ok(())
                },
            )?;
            assert_eq!(
                original, staged,
                "each prior whole proof folds by its physical index"
            );
            compare_accounting(expected, accounting.into_read(rows))
        })
    }

    fn compare_accounting<T: Eq + Debug>(
        mut expected: KindRead<'_, T>,
        mut actual: KindRead<'_, T>,
    ) -> TestResult {
        assert_eq!(
            actual.items, expected.items,
            "only successful rows are emitted"
        );
        assert_eq!(
            sources(actual.ledger.sources)?,
            sources(expected.ledger.sources)?,
            "the same canonical physical source roles and dispositions"
        );
        assert_eq!(
            actual.ledger.field_inspections, expected.ledger.field_inspections,
            "prior exact whole-source proofs absorbed the same read observations"
        );
        expected
            .failures
            .sort_by_key(|failure| failure.record().index());
        actual
            .failures
            .sort_by_key(|failure| failure.record().index());
        assert_eq!(
            actual.failures.len(),
            2,
            "both represented projection failures remain"
        );
        assert_eq!(
            actual.failures, expected.failures,
            "exact physical failures and causes"
        );
        Ok(())
    }

    fn dependency_inspections(
        forge: &DataForge<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> TestResult<Vec<FieldInspection>> {
        let mut earlier = Vec::new();
        for record in forge.records() {
            let tree = record.tree()?;
            let admission = RecordObservation::capture(input, &record)?;
            earlier.extend(
                crate::export_field_walk::inspect_observed(
                    Some(input),
                    &tree,
                    &admission,
                    Some(Kind::Contracts),
                    ExportSourceRole::Dependency,
                    None,
                )?
                .field_inspections,
            );
        }
        Ok(earlier)
    }

    fn database() -> Vec<u8> {
        let ty = "MissionBrokerEntry";
        let mut source = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new(ty).field("number", TestType::Single));
        for (guid, label, number) in [(2, "first", 0.5), (1, "second", 1.5), (1, "third", 2.5)] {
            source = source.record(
                TestRecord::new(
                    guid,
                    ty,
                    format!("{ty}.{label}"),
                    format!("libs/foundry/records/missionbroker/{label}.xml"),
                )
                .value("number", TestValue::Single(number)),
            );
        }
        source.build()
    }
}
