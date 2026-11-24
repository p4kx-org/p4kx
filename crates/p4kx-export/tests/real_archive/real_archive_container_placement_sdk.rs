// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::io;

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::starmap::{
    ContainerInstanceKey, ContainerPlacementEvent, ContainerPlacementRootVisit,
    ContainerPlacementRowCounts, ContainerPlacementVisitSummary, ContainerTargetReadStatus,
    ContainerTargetStatus, DatasetAvailability, visit_container_placements,
};
use p4kx_extraction::{Archive, SourcedValue};

use crate::real_archive_catalog_table_parity::{
    StreamedPlacementDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input::with_archive;
use crate::real_archive_verification::print_line;

const PRODUCER: &str = "P4KX_CONTAINER_PLACEMENT_CATALOG_PRODUCER_REVISION";

#[test]
#[ignore = "installed Archive: configure the exact Build for both full placement tables"]
fn installed_container_placement_cold_visit() -> TestResult {
    with_archive(|archive| read_all(archive, |_| Ok(())).map(|_| ()))
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_container_placement_tables_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut evidence = StreamedPlacementDigests::new(scratch.path());
        let summary = read_all(archive, |event| evidence.observe(event))?;
        let counts =
            measurement.verify_streamed_placement_tables(summary.availability, evidence)?;
        let [instances, unresolved_links] = counts.rows;
        if instances != summary.rows.instances || unresolved_links != summary.rows.unresolved_links
        {
            return Err("the Normal counts disagree with the terminal placement visit".into());
        }
        print_line(&format!(
            "complete container placement SDK parity: instances={instances} unresolvedLinks={unresolved_links} sourceUses={} physicalSources={}",
            counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

fn read_all(
    archive: &Archive,
    mut inspect: impl FnMut(&ContainerPlacementEvent<'_>) -> TestResult,
) -> TestResult<ContainerPlacementVisitSummary> {
    let mut observed = Census::default();
    let summary = visit_container_placements(archive, None, |event| {
        observed
            .inspect(&event)
            .and_then(|()| inspect(&event))
            .map_err(|error| CatalogError::Write(io::Error::other(error.to_string())))
    })?;
    observed.finish(archive, &summary)?;
    print_line(&format!(
        "container placement SDK: availability={:?} roots={} instances={} unresolvedLinks={} descriptorReadOccurrences={} largestRootDescriptors={} verifiedMissing={} sourceUseOccurrences={} physicalSourceOccurrences={}",
        summary.availability,
        summary.visited_roots,
        summary.rows.instances,
        summary.rows.unresolved_links,
        observed.descriptors,
        observed.largest_root,
        observed.verified_missing,
        observed.uses,
        observed.sources,
    ));
    Ok(summary)
}

#[derive(Default)]
struct Census {
    open_root: Option<PendingRoot>,
    completed_roots: Vec<u32>,
    totals: ContainerPlacementRowCounts,
    descriptors: usize,
    largest_root: usize,
    verified_missing: usize,
    uses: usize,
    sources: usize,
}

struct PendingRoot {
    index: u32,
    rows: ContainerPlacementRowCounts,
}

impl Census {
    fn inspect(&mut self, event: &ContainerPlacementEvent<'_>) -> TestResult {
        match event {
            ContainerPlacementEvent::Instance(row) => {
                let root = self.pending(&row.instance_id)?;
                increment(&mut root.rows.instances, 1)
            }
            ContainerPlacementEvent::UnresolvedLink(row) => {
                if !matches!(
                    row.target_status.value(),
                    Some(ContainerTargetStatus::NotFound)
                ) || !matches!(
                    row.target_read_status.value(),
                    Some(ContainerTargetReadStatus::NotAttempted)
                ) {
                    return Err("the complete visit admits only unresolved links with verified target absence".into());
                }
                let root = self.pending(&row.instance_id)?;
                increment(&mut root.rows.unresolved_links, 1)
            }
            ContainerPlacementEvent::RootCompleted(root) => self.complete_root(root),
            _ => Err("unsupported placement visitor event".into()),
        }
    }

    fn pending(
        &mut self,
        key: &SourcedValue<ContainerInstanceKey>,
    ) -> TestResult<&mut PendingRoot> {
        let index = key
            .value()
            .ok_or("a placement callback lacks its original occurrence key")?
            .root_entry_index();
        let root = self.open_root.get_or_insert_with(|| PendingRoot {
            index,
            rows: ContainerPlacementRowCounts::default(),
        });
        if root.index != index {
            return Err(
                "a new physical root emitted rows before the previous root was sealed".into(),
            );
        }
        Ok(root)
    }

    fn complete_root(&mut self, root: &ContainerPlacementRootVisit) -> TestResult {
        let ordinal = self.completed_roots.len();
        let coverage = &root.coverage;
        let uses = root.source_inventory.use_count();
        let sources = root.source_inventory.source_count();
        require_complete_root(ordinal, root)?;
        let pending = self.open_root.take();
        let observed = pending
            .as_ref()
            .map_or_else(ContainerPlacementRowCounts::default, |open| open.rows);
        if observed != root.rows
            || pending.is_some_and(|open| open.index != root.root_entry_index)
            || self.completed_roots.contains(&root.root_entry_index)
            || coverage.verified_missing_containers != observed.unresolved_links
        {
            return Err("a root's physical identity, rows or verified missing count disagree with its callbacks".into());
        }
        if coverage.attempted_descriptors == 0
            || ((observed.instances != 0 || observed.unresolved_links != 0)
                && (uses == 0 || sources == 0))
        {
            return Err("a visited root lacks its physical descriptor observations".into());
        }
        self.completed_roots.push(root.root_entry_index);
        increment(&mut self.totals.instances, observed.instances)?;
        increment(&mut self.totals.unresolved_links, observed.unresolved_links)?;
        increment(&mut self.descriptors, coverage.attempted_descriptors)?;
        self.largest_root = self.largest_root.max(coverage.attempted_descriptors);
        increment(
            &mut self.verified_missing,
            coverage.verified_missing_containers,
        )?;
        increment(&mut self.uses, uses)?;
        increment(&mut self.sources, sources)?;
        print_line(&format!(
            "container placement root SDK: ordinal={ordinal} descriptors={} instances={} unresolvedLinks={} verifiedMissing={} inventoryUses={uses} physicalSources={sources}",
            coverage.attempted_descriptors,
            observed.instances,
            observed.unresolved_links,
            coverage.verified_missing_containers,
        ));
        Ok(())
    }

    fn finish(&self, archive: &Archive, summary: &ContainerPlacementVisitSummary) -> TestResult {
        if archive.build().is_none() || summary.root_inventory.build.as_ref() != archive.build() {
            return Err("the placement visit does not belong to the exact installed Build".into());
        }
        if !summary.is_complete() || self.open_root.is_some() {
            return Err("the canonical directory or a root traversal did not establish complete placement coverage".into());
        }
        let expected_roots: Vec<_> = summary
            .root_inventory
            .roots
            .iter()
            .map(|root| {
                Ok(root
                    .selected_entry()
                    .ok_or("a canonical winning path lacks a unique physical root")?
                    .entry_index())
            })
            .collect::<TestResult<_>>()?;
        if self.completed_roots != expected_roots
            || summary.visited_roots != self.completed_roots.len()
            || summary.rows != self.totals
        {
            return Err(
                "root order, admitted scope or row counts differ from the completed callbacks"
                    .into(),
            );
        }
        match (summary.availability, self.completed_roots.is_empty()) {
            (DatasetAvailability::Available, false) | (DatasetAvailability::Absent, true) => Ok(()),
            _ => {
                Err("the placement table availability differs from the admitted root scope".into())
            }
        }
    }
}

fn require_complete_root(ordinal: usize, root: &ContainerPlacementRootVisit) -> TestResult {
    let coverage = &root.coverage;
    if !coverage.is_complete() {
        return Err(format!(
            "incomplete placement coverage at root ordinal {ordinal}: descriptorInventoryComplete={} routesComplete={} unreadable={} unverifiedMissing={} malformed={} failedDependencies={}",
            coverage.descriptor_inventory_complete, coverage.routes_complete, coverage.unreadable_containers,
            coverage.unverified_missing_containers, coverage.malformed_children, coverage.source_entries.failed_dependencies,
        ).into());
    }
    Ok(())
}

fn increment(total: &mut usize, amount: usize) -> TestResult {
    *total = total
        .checked_add(amount)
        .ok_or("placement witness counter overflow")?;
    Ok(())
}
