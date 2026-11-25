// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Visits complete Template and Broker definitions with each owner's source inventory.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::{BrokerDefinition, TemplateDefinition, owner_definitions};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::mission_flow_owner::FlowOwnerKind;

/// Selects physical Records in an original Template or Broker definition family.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum MissionOwnerDefinitionSelection {
    /// Original Contract Templates.
    ContractTemplates(CatalogRecordSelection),
    /// Original Mission Broker Entries.
    MissionBrokerEntries(CatalogRecordSelection),
}

impl MissionOwnerDefinitionSelection {
    const fn split(self) -> (FlowOwnerKind, CatalogRecordSelection) {
        match self {
            Self::ContractTemplates(records) => (FlowOwnerKind::ContractTemplate, records),
            Self::MissionBrokerEntries(records) => (FlowOwnerKind::MissionBrokerEntry, records),
        }
    }
}

/// The canonical definition of a physical mission owner.
#[derive(Debug)]
#[non_exhaustive]
pub enum MissionOwnerDefinitionRow<'source> {
    /// One authored Contract Template.
    ContractTemplate(TemplateDefinition<'source>),
    /// One authored Mission Broker Entry.
    MissionBrokerEntry(BrokerDefinition<'source>),
}

/// One original mission definition with the source evidence needed to resolve its origins.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionOwnerDefinitionSnapshot<'source> {
    /// The canonical definition and its original source arena and declaration links.
    pub row: MissionOwnerDefinitionRow<'source>,
    /// Original physical sources and uses captured for the definition.
    pub source_inventory: SourceInventory,
    /// Whether the observed fields meet Normal projection rules.
    pub fields_complete: bool,
}

/// Terminal availability and coverage of the requested definitions.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionOwnerDefinitionVisitSummary {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the selected original definition family.
    pub availability: DatasetAvailability,
    /// The number of readable physical owner definitions supplied.
    pub owner_count: usize,
    /// Original owner admission and read coverage.
    pub coverage: CatalogRecordCoverage,
}

/// Visits original definitions in canonical GUID order, preserving physical ties.
///
/// All and GUID selections use canonical admission. An exact Record index also admits an
/// in-scope development leftover. Commit consumer-owned changes after terminal success.
///
/// # Errors
///
/// Returns input, Archive-change, cancellation or visitor errors.
pub fn visit_mission_owner_definitions<E: From<CatalogError>>(
    archive: &Archive,
    selection: MissionOwnerDefinitionSelection,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(&MissionOwnerDefinitionSnapshot<'source>) -> Result<(), E>,
) -> Result<MissionOwnerDefinitionVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_mission_owner_definitions(selection, cancel, visit)
}

/// Visits original definitions from a previously decoded Entry of the same Archive.
///
/// # Errors
///
/// Returns query or visitor errors, including a mismatch for a foreign Entry.
pub fn visit_mission_owner_definitions_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: MissionOwnerDefinitionSelection,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(&MissionOwnerDefinitionSnapshot<'source>) -> Result<(), E>,
) -> Result<MissionOwnerDefinitionVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_mission_owner_definitions(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits original definitions of selected physical mission owners in this session.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`visit_mission_owner_definitions`].
    pub fn visit_mission_owner_definitions<E: From<CatalogError>>(
        &self,
        selection: MissionOwnerDefinitionSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(&MissionOwnerDefinitionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<MissionOwnerDefinitionVisitSummary, E> {
        self.parsed(cancel)?
            .visit_mission_owner_definitions(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits original definitions of selected physical mission owners in the parsed DataForge.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`visit_mission_owner_definitions`].
    pub fn visit_mission_owner_definitions<E: From<CatalogError>>(
        &self,
        selection: MissionOwnerDefinitionSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(&MissionOwnerDefinitionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<MissionOwnerDefinitionVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            visit_definitions(archive, entry, forge, selection, cancel, visit)
        })
    }
}

fn visit_definitions<'source, E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    selection: MissionOwnerDefinitionSelection,
    cancel: Option<&CancelToken>,
    mut visit: impl FnMut(&MissionOwnerDefinitionSnapshot<'source>) -> Result<(), E>,
) -> Result<MissionOwnerDefinitionVisitSummary, E> {
    let (family, records) = selection.split();
    let kind = family.catalog_kind();
    let plan = owner_definitions::plan(forge, kind, records, cancel)?;
    let mut owner_count = 0;
    let coverage = plan.visit::<E>(cancel, |owner, coverage| {
        let origins = CatalogOrigins::for_archive_records(archive, entry)?;
        let input = OriginalCatalogInput::new(forge, &origins);
        let read = owner_definitions::read_owner(&input, owner, coverage, family, cancel)?;
        let row = match family {
            FlowOwnerKind::ContractTemplate => {
                MissionOwnerDefinitionRow::ContractTemplate(read.value.into())
            }
            FlowOwnerKind::MissionBrokerEntry => {
                MissionOwnerDefinitionRow::MissionBrokerEntry(read.value.into())
            }
        };
        let snapshot = MissionOwnerDefinitionSnapshot {
            row,
            source_inventory: origins.uses.finish_with(|| checkpoint(cancel))?,
            fields_complete: read.selection_complete,
        };
        checkpoint(cancel)?;
        visit(&snapshot)?;
        owner_count += 1;
        Ok(())
    })?;
    let availability = if owner_count == 0 {
        let original = kind.records().type_name;
        DatasetAvailability::from_source_declarations(forge, |name| name == original)
    } else {
        DatasetAvailability::Available
    };
    Ok(MissionOwnerDefinitionVisitSummary {
        build: archive.build().cloned(),
        availability,
        owner_count,
        coverage,
    })
}
