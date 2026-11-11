// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, Record, RecordTree, SourceEvidenceError, Value,
};

use super::{ReadError, read_kind};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::export_ledger::SourceDisposition;
use crate::guid_text::CatalogGuid;
use crate::selection::Kind;

#[path = "record_read_original_fixture.rs"]
mod fixture;

type Projected<'source> = (Record<'source>, u32);

#[test]
fn kind_callback_retains_the_actual_record_tree_pair_and_physical_aliases() -> TestResult {
    test_support::with_input(&fixture::database(false), |forge, input| {
        prove_original_aliases(forge)?;
        let read = read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            project,
            |(record, _)| CatalogGuid(record.guid()),
        )?;
        check_projected(&read, forge);
        Ok(())
    })
}

fn prove_original_aliases(forge: &DataForge<'_>) -> TestResult {
    let first = forge.record(1).ok_or("original first physical Record")?;
    let alias = forge.record(2).ok_or("original alias physical Record")?;
    assert_eq!(
        first.guid(),
        alias.guid(),
        "genuine duplicate original GUID"
    );
    assert_ne!(
        first.index(),
        alias.index(),
        "distinct physical original Records"
    );
    assert_eq!(
        first.tree()?.field("value"),
        Some(&Value::UInt32(11)),
        "first body"
    );
    assert_eq!(
        alias.tree()?.field("value"),
        Some(&Value::UInt32(33)),
        "alias body"
    );
    Ok(())
}

fn check_projected(read: &super::KindRead<'_, Projected<'_>>, forge: &DataForge<'_>) {
    assert!(
        read.failures.is_empty(),
        "all selected original bodies are valid"
    );
    assert_eq!(
        read.skipped, 1,
        "development leftover remains excluded before read"
    );
    let physical: Vec<_> = read
        .items
        .iter()
        .map(|(record, _)| record.index())
        .collect();
    let values: Vec<_> = read.items.iter().map(|(_, value)| *value).collect();
    assert_eq!(
        physical,
        [1, 2, 0],
        "stable GUID ordering preserves physical aliases"
    );
    assert_eq!(
        values,
        [11, 33, 22],
        "each callback retained its own original body"
    );
    assert!(
        read.items.iter().all(|(record, _)| record.is_from(forge)),
        "actual owner"
    );
    assert_eq!(
        read.ledger.sources.len(),
        4,
        "wrong-type candidate remains outside"
    );
}
fn project<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> Result<Projected<'source>, ReadError> {
    assert!(
        tree.is_from(record),
        "callback receives the actual canonical owner"
    );
    assert_eq!(
        record.index(),
        tree.record_index(),
        "original physical index"
    );
    assert_eq!(record.guid(), tree.guid(), "original GUID");
    assert_eq!(
        record.record_path()?,
        tree.record_path(),
        "original normalized path"
    );
    let Some(Value::UInt32(value)) = tree.field("value") else {
        return Err("expected original UInt32".into());
    };
    Ok((*record, *value))
}

#[test]
fn kind_callback_rejects_equal_headers_from_a_foreign_input_even_when_filtered() -> TestResult {
    let bytes = fixture::database(false);
    let foreign = DataForge::open(&bytes)?;
    test_support::with_input(&bytes, |forge, input| {
        let own = forge.record(1).ok_or("original admitted Record")?;
        let other = foreign.record(1).ok_or("original foreign Record")?;
        assert_eq!(own.guid(), other.guid(), "genuine equal GUIDs");
        assert_eq!(own.index(), other.index(), "genuine equal indices");
        assert_eq!(
            own.record_path()?,
            other.record_path()?,
            "genuine equal paths"
        );
        assert!(
            !other.is_from(forge),
            "visible equality does not authorize another input"
        );
        let before = input.origins().uses.freeze()?;
        let filter = Filter::new("no-selected-record");
        for filter in [None, Some(&filter)] {
            reject_foreign(&foreign, input, filter);
        }
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "rejection adds no source proof"
        );
        Ok(())
    })
}

fn reject_foreign(
    foreign: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    filter: Option<&Filter>,
) {
    let calls = AtomicUsize::new(0);
    let result = read_kind(
        foreign,
        input,
        Kind::ContractTemplates,
        (filter, None),
        |record, _tree| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(CatalogGuid(record.guid()))
        },
        |guid| *guid,
    );
    assert!(
        matches!(
            result,
            Err(CatalogError::SourceEvidence(
                SourceEvidenceError::ForeignEvidence
            ))
        ),
        "foreign admission is fatal before projection, including an empty filtered selection"
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "no foreign projection callback"
    );
}

#[test]
fn kind_callback_preserves_canonical_and_projection_failure_owners() -> TestResult {
    test_support::with_input(&fixture::database(true), |forge, input| {
        let original = forge.record(0).ok_or("original malformed Record")?;
        assert!(
            matches!(
                original.tree(),
                Err(DataForgeError::StructLayout {
                    declared: 5,
                    fields: 4,
                    ..
                })
            ),
            "genuine original canonical failure precedes exporter assertions"
        );
        canonical_failures(forge, input)
    })?;
    projection_failure()
}

fn canonical_failures(forge: &DataForge<'_>, input: &OriginalCatalogInput<'_, '_>) -> TestResult {
    let calls = AtomicUsize::new(0);
    let read = read_kind(
        forge,
        input,
        Kind::ContractTemplates,
        (None, None),
        |record, _tree| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(CatalogGuid(record.guid()))
        },
        |guid| *guid,
    )?;
    assert_eq!(
        read.failures.len(),
        3,
        "all selected physical failures remain present"
    );
    assert_eq!(
        read.skipped, 1,
        "leftover policy precedes malformed-body read"
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "canonical failures never enter projection"
    );
    for failure in &read.failures {
        assert!(
            failure.record().is_from(forge),
            "failure retains the actual original Record"
        );
        assert!(
            matches!(
                failure.canonical_error(),
                Some(DataForgeError::StructLayout {
                    declared: 5,
                    fields: 4,
                    ..
                })
            ),
            "the original typed cause survives without another canonical read"
        );
    }
    Ok(())
}
fn projection_failure() -> TestResult {
    test_support::with_input(&fixture::database(false), |forge, input| {
        let read = read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            |record, tree| {
                let item = project(record, tree)?;
                if record.index() == 2 {
                    return Err("original alias projection refused".into());
                }
                Ok(item)
            },
            |(record, _)| CatalogGuid(record.guid()),
        )?;
        assert_eq!(
            read.items.len(),
            2,
            "other actual projections remain present"
        );
        assert_eq!(
            read.failures.len(),
            1,
            "one exact original callback failure"
        );
        let failure = read.failures.first().ok_or("original projection failure")?;
        assert_eq!(
            failure.record().index(),
            2,
            "failure is the physical alias, not its GUID twin"
        );
        assert!(
            failure.record().is_from(forge),
            "projection failure keeps its actual owner"
        );
        assert_eq!(
            failure.reason(),
            "original alias projection refused",
            "unchanged human reason"
        );
        assert!(
            read.ledger
                .sources
                .iter()
                .any(|source| source.record_index == 2
                    && source.disposition == SourceDisposition::ProjectionFailed),
            "existing disposition matches the failed actual projection"
        );
        Ok(())
    })
}

#[test]
fn kind_callback_keeps_filter_before_read_and_selected_cancellation() -> TestResult {
    test_support::with_input(&fixture::database(true), |forge, input| {
        let original = forge
            .record(0)
            .ok_or("original malformed filtered Record")?;
        assert!(
            original.tree().is_err(),
            "fixture has an actual malformed body"
        );
        filtered_read(forge, input)?;
        cancelled_read(forge, input);
        Ok(())
    })
}

fn filtered_read(forge: &DataForge<'_>, input: &OriginalCatalogInput<'_, '_>) -> TestResult {
    let filter = Filter::new("no-selected-record");
    let read = read_kind(
        forge,
        input,
        Kind::ContractTemplates,
        (Some(&filter), None),
        project,
        |(record, _)| CatalogGuid(record.guid()),
    )?;
    assert!(read.items.is_empty(), "ordinary filter selects no rows");
    assert!(
        read.failures.is_empty(),
        "excluded malformed bodies remain unread"
    );
    assert!(
        read.ledger.sources.is_empty(),
        "no fabricated filtered observations"
    );
    Ok(())
}

fn cancelled_read(forge: &DataForge<'_>, input: &OriginalCatalogInput<'_, '_>) {
    let cancel = CancelToken::new();
    cancel.cancel();
    let calls = AtomicUsize::new(0);
    let result = read_kind(
        forge,
        input,
        Kind::ContractTemplates,
        (None, Some(&cancel)),
        |record, _tree| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(CatalogGuid(record.guid()))
        },
        |guid| *guid,
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "existing first selected stop"
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "cancellation prevents projection"
    );
}
