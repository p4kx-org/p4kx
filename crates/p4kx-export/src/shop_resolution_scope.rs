// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, FieldOrigin, ProvenanceMode, SourceInventory,
};

use super::{Lists, ShopDeclarations, record_registry, shop_list};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, with_entry_capture_mode};
use crate::catalog_table::DatasetAvailability;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_containers::{Containers, Inventory};
use crate::shop_inventories::{Inventories, is_placeholder};
use crate::shop_inventory_resolution::{InventoryResolver, ShopInventoryResolution};
use crate::shop_registry::{self, RegistryNeed, RegistryShop};
use crate::shop_summary::ShopSummary;
use crate::{CatalogError, Kind, export_health_entries};

/// Selects admitted shop inventory resolution rows.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ShopResolutionSelection<'query> {
    /// Every admitted registry placement.
    All,
    /// The exact canonical key from `shopKey`, including the full normalized ID chain.
    Key(&'query str),
}

impl ShopResolutionSelection<'_> {
    fn includes(self, key: &str) -> bool {
        match self {
            Self::All => true,
            Self::Key(wanted) => key == wanted,
        }
    }
}

/// Archive searches needed to resolve at least one selected registry row.
#[derive(Debug)]
#[non_exhaustive]
pub struct ShopResolutionDiscovery {
    /// Original inventory directory survey and its observed completeness.
    pub inventory_directory: FieldOrigin,
    /// Whether all admitted declaration sources were scanned and attributed.
    pub declarations_complete: bool,
    /// Unique inventory payloads consulted for the selected rows, including failed reads.
    pub inventory_payloads: usize,
}

/// Registry and Archive coverage for this particular resolution query.
#[derive(Debug)]
#[non_exhaustive]
pub struct ShopInventoryResolutionCoverage {
    /// Whether registry selection and admission completed, including a proven absence.
    pub registry_complete: bool,
    /// Declaration and inventory discovery, when any registry rows require resolution.
    pub discovery: Option<ShopResolutionDiscovery>,
    /// Read and projection outcomes for registry, declarations and selected inventory dependencies.
    pub entries: ExportEntryCounts,
}

/// Canonical offline inventory resolution rows and their source inventory.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct ShopInventoryResolutionSnapshot {
    /// The Build identified by this Archive, when available.
    pub build: Option<Build>,
    /// Availability of the source table, independently of how many keys match.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical shop-key order.
    pub rows: Vec<ShopInventoryResolution>,
    /// Original references used by these rows and their coverage.
    pub source_inventory: SourceInventory,
    /// Observations made within this query's scope.
    pub coverage: ShopInventoryResolutionCoverage,
}

/// Reads canonical offline shop inventory resolution from the Archive.
///
/// Both selections use canonical registry admission. Resolving any selected placement searches all
/// admitted container and prefab declarations for competing sources, and reads only unique
/// non-placeholder inventory payloads that can affect a selected row. Source failures stay visible
/// in coverage and in the affected rows. The query uses neither DataForge nor starmap linking.
///
/// # Errors
/// Returns an Archive-change, source-accounting, allocation or cancellation error.
pub fn read_shop_inventory_resolutions(
    archive: &Archive,
    selection: ShopResolutionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ShopInventoryResolutionSnapshot, CatalogError> {
    read_shop_inventory_resolutions_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads shop inventory resolutions with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_shop_inventory_resolutions`.
pub fn read_shop_inventory_resolutions_with_provenance_mode(
    archive: &Archive,
    selection: ShopResolutionSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ShopInventoryResolutionSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture_mode(archive, provenance_mode, |capture| {
        let resolved = read_scope(archive, selection, capture, cancel)?;
        let sources = export_health_entries::compile(resolved.ledger.entries)?;
        let entries = export_health_entries::counts(Kind::Shops, &sources.sources)?;
        let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
        Ok(ShopInventoryResolutionSnapshot {
            build: archive.build().cloned(),
            availability: resolved.availability,
            rows: resolved.rows,
            source_inventory,
            coverage: ShopInventoryResolutionCoverage {
                registry_complete: resolved.registry_complete,
                discovery: resolved.discovery,
                entries,
            },
        })
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

struct ScopeRead {
    availability: DatasetAvailability,
    registry_complete: bool,
    discovery: Option<ShopResolutionDiscovery>,
    rows: Vec<ShopInventoryResolution>,
    ledger: ExportLedger,
}

type SelectedPlacement<'registry> = (&'registry RegistryShop, shop_list::ShopPlacement);

fn read_scope(
    archive: &Archive,
    selection: ShopResolutionSelection<'_>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopeRead, CatalogError> {
    let mut summary = ShopSummary::default();
    let mut read = ScopeRead {
        availability: DatasetAvailability::Unavailable,
        registry_complete: false,
        discovery: None,
        rows: Vec::new(),
        ledger: ExportLedger::default(),
    };
    read.ledger.inventory(Kind::Shops);
    let registry = shop_registry::read_source(
        archive,
        &ArchiveFiles::new(archive),
        RegistryNeed::Optional,
        &mut summary,
        &mut read.ledger,
        capture,
    )?;
    let complete_directory = crate::export_archive_health::inventory(archive).is_none();
    let Some(shop_registry::RegistryDocument {
        rows: registry,
        source: registry_source,
        complete: registry_complete,
    }) = registry
    else {
        if summary.registry_missing && complete_directory {
            read.registry_complete = true;
            read.availability = DatasetAvailability::Absent;
        }
        return Ok(read);
    };
    read.availability = DatasetAvailability::Available;
    let mut placements = Vec::new();
    let disposition = shop_list::visit_registry(&registry, &mut summary, |raw, placement, _| {
        checkpoint(cancel)?;
        if selection.includes(&placement.key) {
            placements
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            placements.push((raw, placement));
        }
        Ok(())
    })?;
    read.registry_complete =
        complete_directory && registry_complete && disposition == SourceDisposition::Projected;
    record_registry(registry_source, disposition, &mut read.ledger);
    if !placements.is_empty() {
        let declarations = ShopDeclarations::read(archive, cancel, capture, &mut summary)?;
        read.resolve(archive, declarations, &placements, capture, cancel)?;
    }
    Ok(read)
}

impl ScopeRead {
    fn resolve(
        &mut self,
        archive: &Archive,
        declarations: ShopDeclarations,
        placements: &[SelectedPlacement<'_>],
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let declarations_complete = declarations.complete();
        let ShopDeclarations {
            containers,
            prefabs,
            ..
        } = declarations;
        let needed = needed_bodies(&containers, placements);
        let (mut inventories, inventory_payloads) =
            Inventories::prepare_resolution(archive, &needed, capture, &mut self.ledger, cancel)?;
        let lists = selected_lists(&containers, &mut inventories, &needed, &mut self.ledger)?;
        let resolver = InventoryResolver::new(
            &containers,
            &inventories,
            &lists,
            &prefabs,
            capture,
            declarations_complete,
        );
        self.rows
            .try_reserve_exact(placements.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for (raw, placement) in placements {
            checkpoint(cancel)?;
            self.rows
                .push(resolver.row(raw, &placement.key, &placement.entity_guid)?);
        }
        shop_list::sort_resolutions(&mut self.rows);
        let inventory_directory = inventories.directory_origin().clone();
        self.discovery = Some(ShopResolutionDiscovery {
            inventory_directory,
            declarations_complete,
            inventory_payloads,
        });
        self.ledger.merge(containers.ledger);
        self.ledger.merge(prefabs.ledger);
        Ok(())
    }
}

fn needed_bodies<'scope>(
    containers: &'scope Containers,
    placements: &[SelectedPlacement<'_>],
) -> BTreeSet<&'scope str> {
    placements
        .iter()
        .filter_map(
            |(_, placement)| match containers.inventory(&placement.entity_guid) {
                Inventory::One(stem) if !is_placeholder(stem) => Some(stem),
                _ => None,
            },
        )
        .collect()
}

fn selected_lists<'scope>(
    containers: &'scope Containers,
    inventories: &mut Inventories,
    needed: &BTreeSet<&str>,
    ledger: &mut ExportLedger,
) -> Result<Lists<'scope>, CatalogError> {
    let mut lists = Lists::new();
    for (stem, selection) in containers.inventory_selections()? {
        if needed.contains(stem) && inventories.files(stem).len() <= 1 {
            lists.insert(stem, inventories.items(&selection, ledger)?);
        }
    }
    Ok(lists)
}
