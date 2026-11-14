// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Authored Generator arrays are checked against raw DataForge values independently of the mapper.

macro_rules! require_equal {
    ($actual:expr, $expected:expr, $failure:expr $(,)?) => {
        error::require($actual == $expected, $failure)?
    };
}

mod compare;
mod error;
mod health;
mod published;
mod requirements;
mod source;
mod tests;
mod values;

use std::collections::HashSet;
use std::time::SystemTime;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, ExportAccountingError, build_catalog_from,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, Filter, Record, TreeBuilder};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_verification;
use error::{
    AccountingFault, ArchiveFault, CatalogFault, CountFault, Failure, ReadFault, SetupFault,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type OracleResult<T = ()> = Result<T, Failure>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn generator_property_sets_match_raw_sources_and_contract_joins() -> OracleResult {
    verify()
}

fn verify() -> OracleResult {
    let requirements = requirements::Requirements::from_environment()?;
    let path = Failure::classify(real_archive_path(), Failure::Setup(SetupFault::ArchivePath))?;
    let key = Failure::classify(real_archive_key(), Failure::Setup(SetupFault::ArchiveKey))?;
    let archive = Failure::classify(
        Archive::open_with_key(path, Some(key)),
        Failure::ArchiveDcb(ArchiveFault::Open),
    )?;
    let watch = ArchiveWatch::new(&archive);
    check_watch(watch.check())?;
    let entry = Failure::classify(
        DataForgeEntry::read(&archive),
        Failure::ArchiveDcb(ArchiveFault::DcbRead),
    )?;
    check_watch(watch.check())?;
    let filter = Filter::new("libs/foundry/records/contracts/contractgenerator/*");
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options).map_err(catalog_failure)?;
    let catalog: Value = Failure::classify(
        serde_json::from_slice(built.json()),
        Failure::OutputParse(error::OutputFault::JsonDecode),
    )?;
    let published = published::Published::read(&catalog)?;
    let forge = Failure::classify(entry.open(), Failure::ArchiveDcb(ArchiveFault::DcbOpen))?;
    let mut trees = TreeBuilder::new();
    let mut checked = Counts::default();
    let health = health::read(&catalog)?;
    for record in forge.records() {
        check_watch(watch.poll())?;
        if is_generator(&record)? {
            compare::check(&record, &mut trees, &published, &health, &mut checked)?;
        }
    }
    health::reconcile(&health, checked.sources, checked.failed)?;
    requirements.check(checked.scoped)?;
    check_watch(watch.check())?;
    check_counts(&checked, published.sets.len(), published.contracts.len())
}

fn check_watch<E>(result: Result<(), E>) -> OracleResult {
    Failure::classify(result, Failure::ArchiveDcb(ArchiveFault::SourceChanged))
}

fn catalog_failure(error: CatalogError) -> Failure {
    Failure::CatalogBuild(match error {
        CatalogError::Accounting(error) => CatalogFault::Accounting(accounting_failure(&error)),
        CatalogError::IncompleteExport(_) => CatalogFault::UnexpectedPublicationRefusal,
        _ => CatalogFault::Build,
    })
}

const fn accounting_failure(error: &ExportAccountingError) -> AccountingFault {
    match error {
        ExportAccountingError::MissingCatalogRows => AccountingFault::MissingCatalogRows,
        ExportAccountingError::ConflictingCatalogRows => AccountingFault::ConflictingCatalogRows,
        ExportAccountingError::MissingCatalogRowSource { .. } => {
            AccountingFault::MissingCatalogRowSource
        }
        ExportAccountingError::MissingPhysicalReferenceTarget { .. } => {
            AccountingFault::MissingPhysicalReferenceTarget
        }
        ExportAccountingError::PoisonedEvidence => AccountingFault::PoisonedEvidence,
        ExportAccountingError::MissingInventory { .. } => AccountingFault::MissingInventory,
        ExportAccountingError::UnobservedFailure => AccountingFault::UnobservedFailure,
        ExportAccountingError::MissingFieldInspection => AccountingFault::MissingFieldInspection,
        ExportAccountingError::ConflictingFieldInspection => {
            AccountingFault::ConflictingFieldInspection
        }
        ExportAccountingError::FieldPath(_) => AccountingFault::FieldPath,
        ExportAccountingError::ConflictingDisposition { .. } => {
            AccountingFault::ConflictingDisposition
        }
        ExportAccountingError::Unreconciled { .. } => AccountingFault::Unreconciled,
        ExportAccountingError::ConflictingSourceDatum => AccountingFault::ConflictingSourceDatum,
        ExportAccountingError::UnfilledLootValue { .. } => AccountingFault::UnfilledLootValue,
        ExportAccountingError::UnsupportedLootSourceValue => {
            AccountingFault::UnsupportedLootSourceValue
        }
        ExportAccountingError::CounterOverflow => AccountingFault::CounterOverflow,
        ExportAccountingError::ConflictingTagTrace => AccountingFault::ConflictingTagTrace,
        ExportAccountingError::UnfilledTransformationNode => {
            AccountingFault::UnfilledTransformationNode
        }
        ExportAccountingError::HealthCaptureClosed => AccountingFault::HealthCaptureClosed,
        _ => AccountingFault::Unknown,
    }
}

fn is_generator(record: &Record<'_>) -> OracleResult<bool> {
    let failure = Failure::IndependentRead(ReadFault::Metadata);
    Ok(
        Failure::classify(record.type_name(), failure)? == "ContractGenerator"
            && Failure::classify(record.record_path(), failure)?
                .replace('\\', "/")
                .to_ascii_lowercase()
                .starts_with("libs/foundry/records/contracts/contractgenerator/"),
    )
}

fn check_counts(checked: &Counts, expected_sets: usize, expected_rows: usize) -> OracleResult {
    require_equal!(
        checked.sets.len(),
        expected_sets,
        Failure::CountReconcile(CountFault::Sets)
    );
    require_equal!(
        checked.rows.len(),
        expected_rows,
        Failure::CountReconcile(CountFault::Contracts)
    );
    error::require(
        !checked.sets.is_empty() && !checked.rows.is_empty(),
        Failure::CountReconcile(CountFault::EmptyCoverage),
    )?;
    real_archive_verification::print_line(&format!(
        "verified {} authored property arrays, {} contract joins and {} scoped-location values against raw sources; {} physical Generator Records, {} independently unreadable and {} ambiguous source arrays",
        checked.sets.len(),
        checked.rows.len(),
        checked.scoped,
        checked.sources,
        checked.failed,
        checked.quarantined
    ));
    Ok(())
}

#[derive(Default)]
struct Counts {
    sets: HashSet<String>,
    rows: HashSet<(String, Option<String>)>,
    scoped: usize,
    sources: usize,
    failed: usize,
    quarantined: usize,
}
