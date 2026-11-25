// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, Build, CancelToken, ProvenanceMode, SourceInventory};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::starmap_name_owners;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, with_entry_capture_mode};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::starmap_placement::StarmapPlacement;
use crate::export_archive_health;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_health_entries;
use crate::guid_text::CatalogGuid;
use crate::starmap::ObjectPlacement;
use crate::starmap_containers::Placements;
use crate::starmap_containers::proof::PlacementEvidence;
use crate::{CatalogError, ExportAccountingError, Kind, StarmapTraversalBudget};

/// Outcome of selecting a physical Starmap Record and resolving its world placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StarmapObjectPlacementStatus {
    /// The complete Record selection admits no object at this index.
    NotAdmitted,
    /// The selected definition or placement routes could not be established completely.
    Unavailable,
    /// The admitted object has no observed placement in the complete traversal.
    Unplaced,
    /// The complete traversal established one canonical placement.
    Placed,
    /// Two routes place the object at incompatible world positions.
    Ambiguous,
}

/// Coverage of the canonical system root and descriptor traversal.
#[derive(Debug)]
#[non_exhaustive]
pub struct StarmapObjectPlacementCoverage {
    /// Whether the original Archive directory establishes the complete root inventory.
    pub directory_complete: bool,
    /// Whether all required descriptor selections and source enumeration completed.
    pub descriptor_inventory_complete: bool,
    /// Whether all discovered routes were traversable within the selected allowance.
    pub routes_complete: bool,
    /// Number of canonical system roots considered.
    pub systems: usize,
    /// Occurrences with an unavailable or unparseable container body.
    pub unreadable_containers: usize,
    /// Missing references whose absence a complete source lookup proved.
    pub verified_missing_containers: usize,
    /// Missing references whose absence could not be established completely.
    pub unverified_missing_containers: usize,
    /// Malformed or stopped Child placement operations.
    pub malformed_children: usize,
    /// Physical and authored Entry outcomes counted by the canonical Entry health compiler.
    pub source_entries: ExportEntryCounts,
}

impl StarmapObjectPlacementCoverage {
    /// Whether placement and further matching occurrences can be decided.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.directory_complete
            && self.descriptor_inventory_complete
            && self.routes_complete
            && self.unreadable_containers == 0
            && self.unverified_missing_containers == 0
            && self.malformed_children == 0
            && self.source_entries.failed_required_reads == 0
            && self.source_entries.failed_projections == 0
            && self.source_entries.failed_dependencies == 0
            && self.source_entries.failed_selection_candidates == 0
    }

    fn measure(archive: &Archive, placements: &mut Placements) -> Result<Self, CatalogError> {
        let ledger = std::mem::take(&mut placements.ledger);
        let counts = placements.counts;
        let entries = export_health_entries::compile(ledger.entries)?;
        Ok(Self {
            directory_complete: export_archive_health::inventory(archive).is_none(),
            descriptor_inventory_complete: !ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects),
            routes_complete: !placements.placement_routes_incomplete,
            systems: counts.systems,
            unreadable_containers: counts.unreadable,
            verified_missing_containers: counts.verified_missing,
            unverified_missing_containers: counts
                .missing
                .checked_sub(counts.verified_missing)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            malformed_children: counts.malformed,
            source_entries: export_health_entries::counts(Kind::StarmapObjects, &entries.sources)?,
        })
    }
}

/// Canonical placement for one physically selected Starmap Record.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct StarmapObjectPlacementSnapshot {
    /// Build metadata present in the watched Archive.
    pub build: Option<Build>,
    /// Requested physical Record index in the selected Build.
    pub record_index: u32,
    /// Availability of the canonical object source declaration.
    pub availability: DatasetAvailability,
    /// Outcome of the selected Record and the placement join.
    pub status: StarmapObjectPlacementStatus,
    /// One canonical row for a conclusive admitted Record, including null placement cells.
    pub rows: Vec<StarmapPlacement>,
    /// Coverage of the physically selected Record.
    pub record_coverage: CatalogRecordCoverage,
    /// Coverage of the placement traversal when the Record was admitted.
    pub traversal: Option<StarmapObjectPlacementCoverage>,
    /// Original sources and uses for this query.
    pub source_inventory: SourceInventory,
}

/// Reads canonical world placement for one physical Starmap Record.
/// The GUID joins against every canonical system root in catalog traversal order.
///
/// # Errors
/// Returns an error for unreadable DataForge input, cancellation, Archive changes or contradictory evidence.
pub fn read_starmap_object_placement(
    archive: &Archive,
    record_index: u32,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<StarmapObjectPlacementSnapshot, CatalogError> {
    read_starmap_object_placement_with_provenance_mode(
        archive,
        record_index,
        budget,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads a starmap object placement with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_starmap_object_placement`.
pub fn read_starmap_object_placement_with_provenance_mode(
    archive: &Archive,
    record_index: u32,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<StarmapObjectPlacementSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    with_entry_capture_mode(archive, provenance_mode, |capture| {
        let owner = read_owner(archive, capture, record_index, cancel)?;
        let (status, rows, traversal) = match &owner.admitted {
            Some(guid) => read_placement(
                archive,
                capture,
                record_index,
                *guid,
                &owner.origins,
                budget,
                cancel,
            )?,
            None if owner.selection_complete() => {
                (StarmapObjectPlacementStatus::NotAdmitted, Vec::new(), None)
            }
            None => (StarmapObjectPlacementStatus::Unavailable, Vec::new(), None),
        };
        let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
        watch.check()?;
        checkpoint(cancel)?;
        Ok(StarmapObjectPlacementSnapshot {
            build: archive.build().cloned(),
            record_index,
            availability: owner.availability,
            status,
            rows,
            record_coverage: owner.coverage,
            traversal,
            source_inventory,
        })
    })
}

struct ObjectOwner {
    availability: DatasetAvailability,
    coverage: CatalogRecordCoverage,
    admitted: Option<CatalogGuid>,
    origins: CatalogOrigins,
}

impl ObjectOwner {
    const fn selection_complete(&self) -> bool {
        self.coverage.unreadable_headers.is_empty()
            && self.coverage.unreadable_bodies.is_empty()
            && self.coverage.incomplete_fields.is_empty()
    }
}

fn read_owner(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<ObjectOwner, CatalogError> {
    let mut admitted = None;
    let survey = starmap_name_owners::visit(
        archive,
        capture,
        CatalogRecordSelection::RecordIndex(record_index),
        cancel,
        |_, name| {
            admitted = name.guid.value().copied();
            Ok(())
        },
    )?;
    Ok(ObjectOwner {
        availability: survey.availability,
        coverage: survey.coverage,
        admitted,
        origins: survey.origins,
    })
}

type PlacementResult = (
    StarmapObjectPlacementStatus,
    Vec<StarmapPlacement>,
    Option<StarmapObjectPlacementCoverage>,
);

fn read_placement(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    record_index: u32,
    guid: CatalogGuid,
    origins: &CatalogOrigins,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<PlacementResult, CatalogError> {
    let mut placements = Placements::read_for_object(archive, cancel, budget, capture, guid.get())?;
    let coverage = StarmapObjectPlacementCoverage::measure(archive, &mut placements)?;
    if !coverage.is_complete() {
        return Ok((
            StarmapObjectPlacementStatus::Unavailable,
            Vec::new(),
            Some(coverage),
        ));
    }
    let placement = ObjectPlacement::select(guid, &placements);
    let status = match &placement.proof {
        Some(PlacementEvidence::Selected(_)) => StarmapObjectPlacementStatus::Placed,
        Some(PlacementEvidence::Ambiguous(_)) => StarmapObjectPlacementStatus::Ambiguous,
        None => StarmapObjectPlacementStatus::Unplaced,
    };
    let row = StarmapPlacement::from_placement(record_index, guid, placement, origins)?;
    Ok((status, vec![row], Some(coverage)))
}
