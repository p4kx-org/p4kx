// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to authored mission definitions.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote, RecordTree,
    SourceInventory,
};

use super::unlocalized_record_query::plan::{OrderedOwnerPlan, Plan, rule_allows};
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::catalog_v3::mission_scenario_definition;
use crate::catalog_v3::mission_sources::{item_definition, module_declaration};
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub use crate::broker_definition::BrokerDefinition;
pub use crate::catalog_v3::mission_sources::module_property::{
    ModuleMissionProperty, ModuleMissionPropertyValue,
};
pub use crate::template_configuration::{MissionFlowRef, MissionPropertyRef};
pub use crate::template_definition::TemplateDefinition;
#[path = "mission_owner_definition_query.rs"]
mod owner_definitions;
#[path = "mission_owner_definition_visit.rs"]
pub mod owner_visit;

pub use item_definition::{MissionItemDefinition, MissionStringVariant, MissionStringVariants};
pub use mission_scenario_definition::MissionScenarioDefinition;
pub use module_declaration::{
    AcquiringStrategy, MissionModuleDeclaration, ModuleLocationEntities, ModuleSpawnedEntities,
    ModuleType, ModuleVariable,
};

/// An authored mission definition family.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum MissionDefinitionFamily {
    /// Declared `MissionScenario` Records.
    Scenarios,
    /// Declared `ModuleDeclaration` Records.
    Modules,
    /// Declared `MissionItem` Records.
    Items,
    /// Declared `ContractTemplate` Records.
    ContractTemplates,
    /// Declared `MissionBrokerEntry` Records.
    MissionBrokerEntries,
}

impl MissionDefinitionFamily {
    const fn kind(self) -> Kind {
        match self {
            Self::Scenarios => Kind::MissionScenarios,
            Self::Modules => Kind::MissionModuleDeclarations,
            Self::Items => Kind::MissionItems,
            Self::ContractTemplates => Kind::ContractTemplates,
            Self::MissionBrokerEntries => Kind::MissionBrokerEntries,
        }
    }
}

enum DefinitionPlan<'source> {
    Scenarios(Plan<'source>),
    Modules(Plan<'source>),
    Items(Plan<'source>),
    ContractTemplates(OrderedOwnerPlan<'source>),
    MissionBrokerEntries(OrderedOwnerPlan<'source>),
}

impl<'source> DefinitionPlan<'source> {
    fn select(
        forge: &'source DataForge<'source>,
        selection: MissionDefinitionSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let kind = selection.family.kind();
        let prepared = || Plan::read(forge, kind, selection.records, cancel, None);
        let ordered = || owner_definitions::plan(forge, kind, selection.records, cancel);
        Ok(match selection.family {
            MissionDefinitionFamily::Scenarios => Self::Scenarios(prepared()?),
            MissionDefinitionFamily::Modules => Self::Modules(prepared()?),
            MissionDefinitionFamily::Items => Self::Items(prepared()?),
            MissionDefinitionFamily::ContractTemplates => Self::ContractTemplates(ordered()?),
            MissionDefinitionFamily::MissionBrokerEntries => Self::MissionBrokerEntries(ordered()?),
        })
    }
}

/// The family and physical Record selection for a mission definition query.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MissionDefinitionSelection {
    /// Original definition family to read.
    pub family: MissionDefinitionFamily,
    /// Physical owner selection within the chosen family.
    /// For Templates and Brokers, `All` and `Guid` use canonical admission and an exact `RecordIndex`
    /// also admits an in-scope development leftover.
    pub records: CatalogRecordSelection,
}

impl MissionDefinitionSelection {
    /// Selects Records in one authored family.
    #[must_use]
    pub const fn new(family: MissionDefinitionFamily, records: CatalogRecordSelection) -> Self {
        Self { family, records }
    }
}

/// The selected canonical definitions in GUID and physical Record order.
#[derive(Debug)]
#[non_exhaustive]
pub enum MissionDefinitionRows<'source> {
    /// Selected physical scenario definitions.
    Scenarios(Vec<MissionScenarioDefinition<'source>>),
    /// Selected physical module declarations.
    Modules(Vec<MissionModuleDeclaration<'source>>),
    /// Selected physical mission item definitions.
    Items(Vec<MissionItemDefinition<'source>>),
    /// Selected physical Contract Template definitions.
    ContractTemplates(Vec<TemplateDefinition<'source>>),
    /// Selected physical Mission Broker Entry definitions.
    MissionBrokerEntries(Vec<BrokerDefinition<'source>>),
}

/// Selected mission definitions and the original evidence referenced by those rows.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionDefinitionSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the selected physical family.
    pub availability: DatasetAvailability,
    /// Selected canonical definitions.
    pub rows: MissionDefinitionRows<'source>,
    /// Protected text rows referenced by selected definitions.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Locales requested by the query session.
    pub requested_languages: Vec<Language>,
    /// Available locales whose Entries were read for this selection.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the localization inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Coverage of physical Record selection, reads and fields.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> MissionDefinitionSnapshot<'source> {
    /// Resolves a protected text among the rows referenced by these definitions.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|row| row.id.value() == Some(reference))
    }
}

/// Supplies one selected mission definition family by physical Record identity.
/// Publish consumer-owned changes after the query returns successfully.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_mission_definitions<T>(
    archive: &Archive,
    selection: MissionDefinitionSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_mission_definitions(selection, cancel, inspect)
}

/// Supplies mission definitions for a decoded DataForge Entry of the same Archive.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_mission_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: MissionDefinitionSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_mission_definitions(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected mission definitions through the Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_definitions`].
    pub fn with_mission_definitions<T>(
        &self,
        selection: MissionDefinitionSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mission_definitions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected mission definitions through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_definitions`].
    pub fn with_mission_definitions<T>(
        &self,
        selection: MissionDefinitionSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(
                archive,
                entry,
                forge,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: MissionDefinitionSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let kind = selection.family.kind();
    let plan = DefinitionPlan::select(forge, selection, cancel)?;
    let languages = match &plan {
        DefinitionPlan::Items(plan) if !plan.owners.is_empty() => requested_languages,
        _ => &[],
    };
    let sources = TextSources::read_languages(archive, languages)?;
    checkpoint(cancel)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(kind);
    let DefinitionRows {
        availability,
        rows,
        coverage,
    } = read_rows(plan, forge, &projection, cancel)?;
    let text_rows = projection.finish_rows()?;
    let available_languages = languages
        .iter()
        .copied()
        .filter(|language| sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(MissionDefinitionSnapshot {
        build: archive.build().cloned(),
        availability,
        rows,
        game_texts: text_rows.game_texts,
        requested_languages: requested_languages.to_vec(),
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage,
    }))
}

struct DefinitionRows<'source> {
    availability: DatasetAvailability,
    rows: MissionDefinitionRows<'source>,
    coverage: CatalogRecordCoverage,
}

fn read_rows<'source>(
    plan: DefinitionPlan<'source>,
    forge: &DataForge<'_>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<DefinitionRows<'source>, CatalogError> {
    let result = match plan {
        DefinitionPlan::Scenarios(plan) => {
            let read = mission_scenario_definition::read;
            let (rows, coverage) = prepared(plan, projection, cancel, read)?;
            split(
                (mission_scenario_definition::table(forge, rows), coverage),
                MissionDefinitionRows::Scenarios,
            )
        }
        DefinitionPlan::Modules(plan) => {
            let read = module_declaration::read;
            let (rows, coverage) = prepared(plan, projection, cancel, read)?;
            split(
                (module_declaration::table(forge, rows), coverage),
                MissionDefinitionRows::Modules,
            )
        }
        DefinitionPlan::Items(plan) => {
            let read = item_definition::read;
            let (rows, coverage) = prepared(plan, projection, cancel, read)?;
            split(
                (item_definition::table(forge, rows), coverage),
                MissionDefinitionRows::Items,
            )
        }
        DefinitionPlan::ContractTemplates(plan) => split(
            owner_definitions::read(
                forge,
                plan,
                projection.original(),
                FlowOwnerKind::ContractTemplate,
                cancel,
            )?,
            MissionDefinitionRows::ContractTemplates,
        ),
        DefinitionPlan::MissionBrokerEntries(plan) => split(
            owner_definitions::read(
                forge,
                plan,
                projection.original(),
                FlowOwnerKind::MissionBrokerEntry,
                cancel,
            )?,
            MissionDefinitionRows::MissionBrokerEntries,
        ),
    };
    Ok(result)
}

fn split<'source, Row>(
    (table, coverage): (CatalogTable<Row>, CatalogRecordCoverage),
    select: impl FnOnce(Vec<Row>) -> MissionDefinitionRows<'source>,
) -> DefinitionRows<'source> {
    DefinitionRows {
        availability: table.availability,
        rows: select(table.rows),
        coverage,
    }
}

fn prepared<'source, Row>(
    plan: Plan<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
    mut read: impl FnMut(
        &RecordTree<'source>,
        &CatalogProjection<'_, 'source>,
        &RecordObservation<'_, 'source>,
        Option<&CancelToken>,
    )
        -> Result<(PreparedDefinition<'source, Row>, Vec<FieldError>), CatalogError>,
) -> Result<(Vec<PreparedDefinition<'source, Row>>, CatalogRecordCoverage), CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = plan;
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let observed = RecordObservation::capture(projection.original(), &owner.record)?;
        let (row, notes) = read(&owner.tree, projection, &observed, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    Ok((rows, coverage))
}
