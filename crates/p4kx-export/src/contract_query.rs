// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::{CatalogLanguageSelection, CatalogRecordSelection};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::texts::{TextSources, Texts};

#[path = "contract_query_coverage.rs"]
mod coverage;
#[path = "contract_query_dependencies.rs"]
mod dependencies;
#[path = "contract_query_finish.rs"]
mod finish;
#[path = "contract_query_interpret.rs"]
mod interpret;
#[path = "contract_query_key.rs"]
mod key;
#[path = "contract_query_key_plan.rs"]
mod key_plan;
#[path = "contract_query_plan.rs"]
mod plan;
#[path = "contract_query_read.rs"]
mod read;
#[path = "contract_query_rows.rs"]
mod rows;
#[path = "contract_summary_query.rs"]
mod summary;
#[path = "contract_query_survey.rs"]
mod survey;

pub use key::{ContractKey, with_contract, with_contract_from};
pub use summary::{ContractSummaries, ContractSummary};

pub use coverage::{
    ContractDependencyCoverage, ContractDiagnosticExample, ContractQueryCoverage,
    ContractQueryDiagnostic, ShadowedContract,
};
pub use rows::{
    AutoSpawnSettings, AutoSpawns, BlueprintReward, BlueprintSource, CatalogRecordFailure,
    ClassFilter, ClassTarget, ClassTargets, ClueDetails, ClueItemDetails, ContractBase,
    ContractCooldown, ContractEntry, ContractFaction, ContractFacts, ContractOccurrence,
    ContractWithTitles, CooldownLayers, DefinitionReference, DependentProperty, EntitySpawnGroup,
    EntitySpawnOption, EntitySpawnPool, EntitySpawnSlot, FactLayer, FactionFilter, FixedReward,
    FloatOption, FunctionFilter, FunctionTarget, FunctionTargets, GiverFaction, GiverSource,
    HaulingItem, HaulingItemKind, HaulingOrder, HaulingRequirement, HaulingSource, Hostility,
    IntegerOption, Investigation, InvestigationClue, InvestigationClueItem, InvestigationLocation,
    InvestigationName, InvestigationNameSource, InvestigationSuspect, ItemToken, ItemTokenOutcome,
    LayeredFacts, LocationCondition, LocationCount, LocationPrerequisite, LocationQuery,
    LocationQueryLink, MissionItemGap, MissionPropertyDeclaration, MissionPropertySet,
    MissionPropertyText, MissionReward, MissionTitles, NpcSpawnGroup, NpcSpawnName, NpcSpawnOption,
    NpcSpawnPool, OfferedAt, OptionMetadata, OtherLocationFilter, OwnedReference,
    OwnedReferenceStatus, PickupPlace, ProjectedMissionText, PropertyReference,
    PropertyReferenceStatus, PropertyTarget, PropertyTargets, PropertyValue, PublishedNpcPool,
    RelativeLocationFilter, RelativeTarget, ReputationGain, RequiredIntro, ScopedLocation,
    ShipSpawnGroup, ShipSpawnOption, ShipSpawnPool, ShipSpawnSlot, SizeFilter, SizeTarget,
    SizeTargets, SourcePointer, SourceTagList, SpawnClosetSearchOptions, SpawnInventoryItem,
    SpawnLevel, SpawnPoolLink, SpawnTagText, StandingLevel, StandingRequirement, Stated,
    StringOption, TagOption, TagPaths, TagTerm, TimeTrialSplit,
};

/// Canonical contracts selected by physical owners or exact logical identity and required joins.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContractSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the `ContractGenerator` source family.
    pub availability: DatasetAvailability,
    /// Selected globally canonical contract rows in ordinary catalog order.
    pub rows: Vec<ContractEntry<'source>>,
    /// Canonical titles and descriptions for the same physical contract occurrences.
    pub mission_titles: Vec<MissionTitles<'source>>,
    /// Original selected-owner definitions in the logical query's source closure.
    /// Use `with_contract_generators` to read source-only owners without admitted contract rows.
    pub generators: Vec<crate::mission_generator::GeneratorDefinition<'source>>,
    /// Original outside-root Generator definitions required for canonical selection or intro joins.
    pub dependency_generators: Vec<crate::mission_generator::GeneratorDefinition<'source>>,
    /// Canonical authored sets for owner selections, or only linked sets for an exact logical key.
    pub mission_property_sets: Vec<MissionPropertySet<'source>>,
    /// Ship spawn descriptions projected from admitted source dependencies.
    pub ship_spawns: Vec<ShipSpawnPool>,
    /// NPC spawn descriptions projected from admitted source dependencies.
    pub npc_spawns: Vec<PublishedNpcPool>,
    /// Entity spawn descriptions projected from admitted source dependencies.
    pub entity_spawns: Vec<EntitySpawnPool>,
    /// Authored location queries projected from admitted source dependencies.
    pub location_queries: Vec<LocationQuery>,
    /// Public short texts referenced by returned rows and dependency tables.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Protected texts referenced by returned rows and dependency tables.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested localization policy, including unavailable locales.
    pub requested_languages: Vec<Language>,
    /// Requested localization payloads actually available and read.
    pub available_languages: Vec<Language>,
    /// Notes from the localization inputs actually consulted.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original physical inputs and uses retained by this query.
    pub source_inventory: SourceInventory,
    /// Explicit root, dependency, failure and duplicate-selection coverage.
    pub coverage: ContractQueryCoverage<'source>,
}

/// Supplies canonical contracts owned by selected Generator Records and their join dependencies.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence, Archive change or cancellation errors.
pub fn with_contracts<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_contracts(selection, cancel, inspect)
}

/// Supplies canonical contracts from an already decoded Entry of the same Archive.
///
/// # Errors
/// Returns the errors of `with_contracts`, including source-origin mismatch.
pub fn with_contracts_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_contracts(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical contracts using this session's retained original DataForge.
    ///
    /// # Errors
    /// Returns the errors of `with_contracts`.
    pub fn with_contracts<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contracts(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies canonical contracts using this view's retained DataForge indexes.
    ///
    /// # Errors
    /// Returns the errors of `with_contracts`.
    pub fn with_contracts<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(
                archive,
                entry,
                forge,
                plan::Lookup::Owners(selection),
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: plan::Lookup,
    languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read_parsed(
        archive,
        entry,
        forge,
        QueryRequest {
            selection,
            retention: read::Retention::Details,
        },
        languages,
        cancel,
        |query| inspect(query.into_full()),
    )
}

struct QueryOutput<'source> {
    build: Option<Build>,
    availability: DatasetAvailability,
    projected: read::Projected<'source>,
    public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    requested_languages: Vec<Language>,
    available_languages: Vec<Language>,
    localization_notes: Vec<LocalizationNote>,
    source_inventory: SourceInventory,
}

#[derive(Clone, Copy)]
struct QueryRequest {
    selection: plan::Lookup,
    retention: read::Retention,
}

fn read_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    request: QueryRequest,
    languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(QueryOutput<'_>) -> T,
) -> Result<T, CatalogError> {
    let QueryRequest {
        selection,
        retention,
    } = request;
    let mut origins = entry.records(archive)?;
    let plan = plan::Plan::read(
        forge,
        &OriginalCatalogInput::new(forge, &origins),
        selection,
        cancel,
    )?;
    let requested = if plan.has_rows { languages } else { &[] };
    let sources = TextSources::read_languages(archive, requested)?;
    checkpoint(cancel)?;
    origins.attach_texts(archive, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry);
    let projected = read::read(forge, &projection, plan, retention, cancel)?;
    let text_rows = projection.finish_rows()?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(QueryOutput {
        build: archive.build().cloned(),
        availability: DatasetAvailability::from_source_declarations(forge, |name| {
            name == "ContractGenerator"
        }),
        projected,
        public_names: text_rows.public_names,
        game_texts: text_rows.game_texts,
        requested_languages: languages.to_vec(),
        available_languages: requested
            .iter()
            .copied()
            .filter(|language| sources.entry(*language).is_some())
            .collect(),
        localization_notes: texts.notes().to_vec(),
        source_inventory,
    }))
}

impl<'source> QueryOutput<'source> {
    fn into_full(self) -> ContractSnapshot<'source> {
        ContractSnapshot {
            build: self.build,
            availability: self.availability,
            rows: self.projected.rows,
            mission_titles: self.projected.titles,
            generators: self.projected.generators,
            dependency_generators: self.projected.dependency_generators,
            mission_property_sets: self.projected.property_sets,
            ship_spawns: self.projected.ships,
            npc_spawns: self.projected.npcs,
            entity_spawns: self.projected.entities,
            location_queries: self.projected.locations,
            public_names: self.public_names,
            game_texts: self.game_texts,
            requested_languages: self.requested_languages,
            available_languages: self.available_languages,
            localization_notes: self.localization_notes,
            source_inventory: self.source_inventory,
            coverage: self.projected.coverage,
        }
    }
}
