// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, Language, LocalizationNote, SourceInventory};

use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::commodity_contract::{CommodityDefinition, CommodityPresentation};

/// Selected Commodities with matching definitions, presentations and source evidence.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct CommoditySnapshot<'source> {
    /// The Build identified at the Archive boundary.
    pub build: Option<Build>,
    /// Availability of the Catalog's Commodity source declarations.
    pub availability: DatasetAvailability,
    /// Physical Commodity definitions in canonical order.
    pub definitions: Vec<CommodityDefinition<'source>>,
    /// Presentations in the same order as the physical definitions.
    pub presentations: Vec<CommodityPresentation>,
    /// Public names referenced by the selected definitions or presentations.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Game texts referenced by the selected definitions or presentations.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// The session's requested languages, including unavailable payloads.
    pub requested_languages: Vec<Language>,
    /// Requested languages whose payloads were available and consulted.
    pub available_languages: Vec<Language>,
    /// Diagnostics from localization payloads consulted by the query.
    pub localization_notes: Vec<LocalizationNote>,
    /// The original source uses and their physical source inventory.
    pub source_inventory: SourceInventory,
    /// Coverage of directly selected physical owners.
    pub coverage: CatalogRecordCoverage,
    /// Coverage of original existence, membership and fallback dependencies.
    pub dependency_coverage: CatalogRecordCoverage,
}
