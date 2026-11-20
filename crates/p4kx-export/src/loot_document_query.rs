// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, SourceInventory, SourcedValue};

use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability, ParsedCatalogQueries,
};
use crate::export_entry_counts::ExportEntryCounts;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::{CatalogError, Filter, StarmapTraversalBudget};

#[path = "loot_document_query_read.rs"]
mod read;

/// Selects reached documents by physical Archive and nested Entry indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LootDocumentSelection {
    /// Every document admitted within the requested Catalog Record scope.
    All,
    /// Documents in one exact original containing Entry.
    Container {
        /// Original physical containing Entry index.
        container_entry_index: u32,
    },
    /// One exact `EntXML` Entry inside its original containing Entry.
    Document {
        /// Original physical containing Entry index.
        container_entry_index: u32,
        /// Original nested Entry index inside the selected container.
        document_entry_index: u32,
    },
}

/// Physical document selection and the Catalog scope to replay.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct LootDocumentQuery<'filter> {
    /// Physical selection of the returned documents.
    pub selection: LootDocumentSelection,
    /// Original Catalog Record selection whose Loot reachability is queried.
    pub catalog_filter: Option<&'filter Filter>,
    /// Optional caller-selected descriptor and Child traversal allowances.
    pub placement_budget: StarmapTraversalBudget,
}

impl LootDocumentQuery<'_> {
    /// Selects documents under the normal unfiltered Catalog policy.
    #[must_use]
    pub const fn new(selection: LootDocumentSelection) -> Self {
        Self {
            selection,
            catalog_filter: None,
            placement_budget: StarmapTraversalBudget::new(),
        }
    }
}

/// Admission of the original Starmap or harvest provider Record scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LootDocumentAdmission {
    /// An exact original Record path activates the Catalog's Loot seed policy.
    Selected,
    /// The complete Record-path selection has no record that activates it.
    Absent,
    /// An unreadable Record path prevented a complete search.
    Unavailable,
}

/// Admission of the selected physical containing Entry within the Loot walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LootDocumentReachability {
    /// The request enumerated all reached owners within the selected scope.
    Enumeration,
    /// The selected physical containing Entry is a seed or a reached target.
    Reached,
    /// Complete evidence excludes the selected owner.
    NotReached,
    /// The requested owner could not be reached or excluded with complete evidence.
    Unavailable,
}

/// Physical evidence and completeness of one scoped Loot document query.
#[derive(Debug)]
#[non_exhaustive]
pub struct LootDocumentCoverage {
    /// Whether the root Archive directory can support a complete lookup.
    pub directory_complete: bool,
    /// Whether the selected Record paths activate the placement dependency.
    pub admission: LootDocumentAdmission,
    /// One positively selected original Record and the header inputs establishing admission.
    pub admission_record: Option<SourcedValue<u32>>,
    /// The selected containing Entry's observed relationship to the traversal.
    pub reachability: LootDocumentReachability,
    /// The relevant world descriptors and placement routes were fully observed.
    pub placements_complete: bool,
    /// The selected document sources were fully read or proved absent.
    pub documents_complete: bool,
    /// The number of distinct Loot outer Entry reads attempted in the request.
    pub visited_loot_containers: usize,
    /// Physical world descriptor input operations and failures.
    pub placement_entries: ExportEntryCounts,
    /// Physical Loot link and container input operations and failures.
    pub loot_entries: ExportEntryCounts,
}

/// Selected canonical Loot document rows with their scoped evidence.
#[derive(Debug)]
#[non_exhaustive]
pub struct LootDocumentSnapshot {
    /// The source Build when the Archive identifies it.
    pub build: Option<Build>,
    /// The requested physical selection.
    pub selection: LootDocumentSelection,
    /// Whether the query observed the family within the requested source scope.
    pub availability: DatasetAvailability,
    /// Canonical rows ordered by containing Entry position then nested Entry index.
    pub rows: Vec<LootEntityContainerDocument>,
    /// Scope, actual source operations and incomplete reachability.
    pub coverage: LootDocumentCoverage,
    /// Actual original traversal uses and immediate contributors to selected rows.
    pub source_inventory: SourceInventory,
}

/// Reads the selected original Loot document scope from an installed Archive.
///
/// # Errors
/// Returns input, original source accounting, Archive change or cancellation errors.
pub fn read_loot_documents(
    archive: &Archive,
    query: LootDocumentQuery<'_>,
    cancel: Option<&CancelToken>,
) -> Result<LootDocumentSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_loot_documents(query, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads the given selection using this session's original Archive and DataForge Entry.
    ///
    /// # Errors
    /// Returns input, original source accounting, Archive change or cancellation errors.
    pub fn read_loot_documents(
        &self,
        query: LootDocumentQuery<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<LootDocumentSnapshot, CatalogError> {
        self.parsed(cancel)?.read_loot_documents(query, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads the given selection using the reusable parsed DataForge view.
    ///
    /// # Errors
    /// Returns original source accounting, Archive change or cancellation errors.
    pub fn read_loot_documents(
        &self,
        query: LootDocumentQuery<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<LootDocumentSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::run(archive, entry, forge, query, cancel)
        })
    }
}
