// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original law and security definitions.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, Language, LocalizationNote, SourceInventory,
};

use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::infraction_definition::{
    InfractionDefinition, InfractionSetDefinition, InfractionUse,
};
pub use crate::catalog_v3::infraction_parameters::{InfractionFlags, InfractionParameters};
pub use crate::catalog_v3::jurisdiction_definition::{
    ControlledSubstanceClass, ImpoundingDefinition, JurisdictionDefinition, JurisdictionPossession,
};
pub use crate::catalog_v3::law_kinds::security_network_manifest::SecurityNetworkManifest;
pub use crate::catalog_v3::law_kinds::security_network_manifest::security_network_protocol::{
    HostilityRules, RoomIdentifiers, SecurityClearance, SecurityNetworkProtocol,
    SecurityPermissions, SecurityProtocolOverride,
};
pub use crate::catalog_v3::law_kinds::security_network_manifest::security_network_variables::{
    SecurityNetworkVariable, SecurityVariableEffect, SecurityVariableEffects,
    SecurityVariableValue, VariableEffect, VariableValue,
};

#[path = "law_query_source.rs"]
mod source;

/// Selects an original law definition family.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum LawFamily {
    /// Includes all four law and security families.
    All,
    /// Includes original infraction definitions.
    Infractions,
    /// Includes reusable ordered sets of infraction uses.
    InfractionSets,
    /// Includes jurisdictions and their directly authored rules.
    Jurisdictions,
    /// Includes original security protocols and effects.
    SecurityNetworkManifests,
}

impl LawFamily {
    const fn includes(self, kind: Kind) -> bool {
        matches!(
            (self, kind),
            (Self::All, _)
                | (Self::Infractions, Kind::Infractions)
                | (Self::InfractionSets, Kind::InfractionSets)
                | (Self::Jurisdictions, Kind::Jurisdictions)
                | (
                    Self::SecurityNetworkManifests,
                    Kind::SecurityNetworkManifests
                )
        )
    }

    fn selected<Row>(self, kind: Kind, table: CatalogTable<Row>) -> Option<LawRows<Row>> {
        self.includes(kind).then_some(LawRows {
            availability: table.availability,
            rows: table.rows,
        })
    }
}

/// Selected definitions of one law family.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct LawRows<Row> {
    /// Availability of the declared source family.
    pub availability: DatasetAvailability,
    /// Canonical rows in GUID and physical Record order.
    pub rows: Vec<Row>,
}

/// Law definitions and their referenced text and source inventory.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct LawSnapshot<'source> {
    /// The installed Build when the Archive identifies it.
    pub build: Option<Build>,
    /// Original infraction definitions when requested.
    pub infractions: Option<LawRows<InfractionDefinition<'source>>>,
    /// Original infraction sets when requested.
    pub infraction_sets: Option<LawRows<InfractionSetDefinition<'source>>>,
    /// Original jurisdiction definitions when requested.
    pub jurisdictions: Option<LawRows<JurisdictionDefinition<'source>>>,
    /// Original security network manifests when requested.
    pub security_network_manifests: Option<LawRows<SecurityNetworkManifest<'source>>>,
    /// Canonical public-name entries referenced by these definitions.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Canonical protected text entries referenced by these definitions.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested languages, including unavailable locales.
    pub requested_languages: Vec<Language>,
    /// Requested languages successfully read for the selected records.
    pub available_languages: Vec<Language>,
    /// Notes from the localization payloads consulted by this query.
    pub localization_notes: Vec<LocalizationNote>,
    /// Physical sources and uses referenced by this result.
    pub source_inventory: SourceInventory,
    /// Unreadable or incomplete original Records and excluded leftovers.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies the selected original law definitions and text to a callback.
/// Publish consumer-owned results after the query succeeds.
///
/// # Errors
/// Returns Archive, source-evidence or cancellation errors.
pub fn with_law<T>(
    archive: &Archive,
    family: LawFamily,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LawSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_law(family, selection, cancel, inspect)
}

/// Supplies the selected original law definitions from a decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_law_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    family: LawFamily,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LawSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_law(family, selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies the selected law definitions with the session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns Archive, source-evidence or cancellation errors.
    pub fn with_law<T>(
        &self,
        family: LawFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LawSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_law(family, selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the selected law definitions using this parsed DataForge.
    ///
    /// # Errors
    /// Returns Archive, source-evidence or cancellation errors.
    pub fn with_law<T>(
        &self,
        family: LawFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LawSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            source::query(
                archive,
                entry,
                forge,
                family,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}
