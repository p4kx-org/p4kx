// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::interactions::{
    InteractionSnapshot, InteractionVisitSummary, visit_interactions,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::Archive;

use super::component_family_parity::{expected_leftovers, physical_slot};
use super::equipment_input;
use crate::real_archive_catalog_table_parity::{
    PhysicalSlot, StreamedComponentDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

const DATASET: &str = "/configurations/interactions";
const PRODUCER: &str = "P4KX_INTERACTION_CATALOG_PRODUCER_REVISION";
const LEFTOVERS: &str = "P4KX_INTERACTION_SDK_EXPECTED_LEFTOVERS";

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_interaction_visitor_matches_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected_leftovers = expected_leftovers(LEFTOVERS)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut evidence = StreamedComponentDigests::new(scratch.path());
        let summary = read_all(archive, expected_leftovers, |snapshot| {
            evidence.retain_owner(
                snapshot.rows(),
                |row| {
                    let owner = row.owner();
                    physical_slot(owner.record_index(), owner.component_ordinal())
                },
                snapshot.source_inventory(),
            )
        })?;
        let counts = measurement.verify_streamed_component_table(
            DATASET,
            summary.availability(),
            evidence,
        )?;
        print_line(&format!(
            "complete interaction visitor SDK parity: rows={:?} sourceUses={} physicalSources={}",
            counts.rows, counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and exact Build"]
fn installed_interaction_visitor_cold() -> TestResult {
    let expected_leftovers = expected_leftovers(LEFTOVERS)?;
    equipment_input::with_archive(|archive| {
        read_all(archive, expected_leftovers, |_| Ok(())).map(|_| ())
    })
}

fn read_all(
    archive: &Archive,
    expected_leftovers: usize,
    mut inspect: impl FnMut(&InteractionSnapshot<'_>) -> TestResult,
) -> TestResult<InteractionVisitSummary> {
    let mut observed = VisitCensus::default();
    let summary = visit_interactions(
        archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |index, snapshot| -> TestResult {
            observed.inspect(archive, index, snapshot)?;
            inspect(snapshot)
        },
    )?;
    observed.finish(&summary, expected_leftovers)?;
    print_line(&format!(
        "interaction visitor SDK: dataset={DATASET} availability={:?} owners={} rows={} sourceUseOccurrences={} physicalSourceOccurrences={} skippedLeftovers={}",
        summary.availability(),
        observed.owners,
        observed.rows,
        observed.uses,
        observed.sources,
        summary.coverage().skipped_leftovers,
    ));
    Ok(summary)
}

#[derive(Default)]
struct VisitCensus {
    last_owner: Option<u32>,
    last_slot: Option<PhysicalSlot>,
    owners: usize,
    rows: usize,
    uses: usize,
    sources: usize,
}

impl VisitCensus {
    fn inspect(
        &mut self,
        archive: &Archive,
        index: u32,
        snapshot: &InteractionSnapshot<'_>,
    ) -> TestResult {
        if archive.build().is_none()
            || snapshot.build() != archive.build()
            || !snapshot.requested_languages().is_empty()
            || self.last_owner.is_some_and(|previous| previous >= index)
        {
            return Err(
                "the interaction visitor has a foreign Build, locale or unordered owner".into(),
            );
        }
        require_coverage(snapshot.coverage(), 0)?;
        self.last_owner = Some(index);
        for row in snapshot.rows() {
            let owner = row.owner();
            let slot = physical_slot(owner.record_index(), owner.component_ordinal())?;
            if slot.record_index != index || self.last_slot.is_some_and(|previous| previous >= slot)
            {
                return Err("the interaction visitor emitted a foreign, duplicate or unordered physical slot".into());
            }
            self.last_slot = Some(slot);
        }
        let uses = snapshot.source_inventory().use_count();
        let sources = snapshot.source_inventory().source_count();
        if !snapshot.rows().is_empty() && (uses == 0 || sources == 0) {
            return Err(
                "an interaction owner with component rows lacks original source evidence".into(),
            );
        }
        increment(&mut self.owners, 1)?;
        increment(&mut self.rows, snapshot.rows().len())?;
        increment(&mut self.uses, uses)?;
        increment(&mut self.sources, sources)
    }

    fn finish(&self, summary: &InteractionVisitSummary, expected_leftovers: usize) -> TestResult {
        match (summary.availability(), self.rows) {
            (DatasetAvailability::Available, _) | (DatasetAvailability::Absent, 0) => {}
            _ => {
                return Err(
                    "the interaction family does not have an established availability".into(),
                );
            }
        }
        if summary.row_count() != self.rows || summary.owner_count() != self.owners {
            return Err(
                "the interaction terminal counts differ from physical visitor callbacks".into(),
            );
        }
        require_coverage(summary.coverage(), expected_leftovers)
    }
}

fn increment(total: &mut usize, count: usize) -> TestResult {
    *total = total
        .checked_add(count)
        .ok_or("interaction witness counter overflow")?;
    Ok(())
}

fn require_coverage(coverage: &CatalogRecordCoverage, expected_leftovers: usize) -> TestResult {
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || coverage.skipped_leftovers != expected_leftovers
    {
        return Err(format!(
            "incomplete interaction coverage (unreadableHeaders={} unreadableBodies={} incompleteFields={} skippedLeftovers={} expectedLeftovers={expected_leftovers})",
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
            coverage.skipped_leftovers,
        ).into());
    }
    Ok(())
}

#[test]
fn interaction_witness_requires_known_leftovers_and_complete_observations() {
    let mut coverage = CatalogRecordCoverage::default();
    assert!(
        require_coverage(&coverage, 0).is_ok(),
        "empty complete evidence"
    );
    coverage.skipped_leftovers = 2;
    assert!(
        require_coverage(&coverage, 2).is_ok(),
        "bound policy exclusions are complete"
    );
    assert!(
        require_coverage(&coverage, 1).is_err(),
        "an additional policy exclusion must be explained"
    );
    assert!(
        require_coverage(&coverage, 3).is_err(),
        "missing policy exclusions also change the bound selection"
    );
    coverage.incomplete_fields.push(3);
    assert!(
        require_coverage(&coverage, 2).is_err(),
        "unreadable source fields always block complete parity"
    );
}
