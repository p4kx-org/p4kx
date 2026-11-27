// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_item_components::source_getters;
use super::super::prepared_definition::PreparedDefinition;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_row::CatalogRow;
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::{CatalogError, ExportAccountingError};

#[path = "scenario_progress_faction.rs"]
mod faction;

use faction::FactionReader;

pub(crate) const SOURCE_TYPE: &str = "ScenarioProgress";

type OrderedCells<Row> = SourcedValue<Vec<SourcedValue<Row>>>;
type ReferenceCell<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// A physical `ScenarioProgress` record and its ordered reward settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContractScenario<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    faction_reward_tiers: OrderedCells<ScenarioFactionTiers<'source>>,
}

impl<'source> ContractScenario<'source> {
    source_getters! {
        /// Borrows the original physical `ScenarioProgress` identity.
        source: CanonicalRecordHeader<'source>,
        /// Borrows every original faction settings slot in source order.
        faction_reward_tiers: OrderedCells<ScenarioFactionTiers<'source>>,
    }
}

/// The authored reward settings and reached reputation link for one faction slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ScenarioFactionTiers<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction: ReferenceCell<'source>,
    faction_name: SourcedValue<&'source str>,
    faction_reputation: SourcedValue<CatalogRecordReference>,
    points_as_percent: SourcedValue<bool>,
    progressions: OrderedCells<ScenarioProgression<'source>>,
}

impl<'source> ScenarioFactionTiers<'source> {
    source_getters! {
        /// Borrows the authored Faction reference and its resolution.
        faction: ReferenceCell<'source>,
        /// Borrows the final path segment of the selected physical Faction.
        faction_name: SourcedValue<&'source str>,
        /// Borrows the reputation reference authored by that physical Faction.
        faction_reputation: SourcedValue<CatalogRecordReference>,
        /// Borrows the authored point conversion flag.
        points_as_percent: SourcedValue<bool>,
        /// Borrows the ordered original progression slots.
        progressions: OrderedCells<ScenarioProgression<'source>>,
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        factions: &mut FactionReader<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let faction = fields.record_link("faction", notes)?;
        let linked = factions.read(&faction)?;
        Ok(Self {
            faction,
            faction_name: linked.name,
            faction_reputation: linked.reputation,
            points_as_percent: fields.boolean("convertTiersPointsToPercent", notes)?,
            progressions: fields.declared_owned_array(
                "tierProgressions",
                notes,
                ScenarioProgression::read,
            )?,
        })
    }
}

/// One original progression and its ordered reward slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ScenarioProgression<'source> {
    min_multiplier_threshold: SourcedValue<ResourceNumber>,
    completion_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    period_mission_scenario: ReferenceCell<'source>,
    tiers: OrderedCells<ScenarioTier<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ScenarioProgression<'source> {
    source_getters! {
        /// Borrows the original multiplier threshold and its numeric storage.
        min_multiplier_threshold: SourcedValue<ResourceNumber>,
        /// Borrows the concrete completion settings type.
        completion_type: SourcedValue<&'source str>,
        /// Borrows the original periodic scenario Reference.
        period_mission_scenario: ReferenceCell<'source>,
        /// Borrows the original ordered reward slots.
        tiers: OrderedCells<ScenarioTier<'source>>,
        /// Borrows the original completion pointer and other unprojected members.
        authored_fields: SourceArenaOutput<'source>,
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let min_multiplier_threshold = fields.authored_number("minMultiplierThreshold", notes)?;
        let completion_type =
            fields.declared_pointer("completionType", notes, |value, _| value.source_type())?;
        let period_mission_scenario = fields.record_link("periodMissionScenario", notes)?;
        let tiers = fields.declared_owned_array("tierRewards", notes, ScenarioTier::read)?;
        let authored_fields = fields.authored_members_except([
            ("minMultiplierThreshold", min_multiplier_threshold.state()),
            ("periodMissionScenario", period_mission_scenario.state()),
            ("tierRewards", tiers.state()),
        ])?;
        Ok(Self {
            min_multiplier_threshold,
            completion_type,
            period_mission_scenario,
            tiers,
            authored_fields,
        })
    }
}

/// One authored reward tier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ScenarioTier<'source> {
    min_points: SourcedValue<ResourceNumber>,
    badge_to_award: SourcedValue<&'source str>,
    global_points_multiplier: SourcedValue<ResourceNumber>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    blueprint_pools: SourcedValue<Vec<ReferenceCell<'source>>>,
}

impl<'source> ScenarioTier<'source> {
    source_getters! {
        /// Borrows the authored point threshold and original numeric storage.
        min_points: SourcedValue<ResourceNumber>,
        /// Borrows the authored badge label, including the no-badge sentinel.
        badge_to_award: SourcedValue<&'source str>,
        /// Borrows the authored multiplier and original numeric storage.
        global_points_multiplier: SourcedValue<ResourceNumber>,
        /// Borrows the original ordered blueprint pool references.
        blueprint_pools: SourcedValue<Vec<ReferenceCell<'source>>>,
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min_points: fields.authored_integer("minPoints", notes)?,
            badge_to_award: fields.borrowed_label("badgeToAward", notes)?,
            global_points_multiplier: fields.authored_number("globalPointsMultiplier", notes)?,
            blueprint_pools: fields.record_link_array("blueprintPool", notes)?,
        })
    }
}

pub(crate) type PreparedContractScenario<'source> =
    PreparedDefinition<'source, ContractScenario<'source>>;

pub(crate) struct ScenarioRead<'source> {
    pub(crate) scenario: PreparedContractScenario<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) dependencies: ExportLedger,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScenarioRead<'source>, CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let mut factions = FactionReader::new(input, cancel);
    let (scenario, notes) = PreparedDefinition::read_original(
        tree,
        input,
        admission,
        cancel,
        |source, fields, notes| {
            let faction_reward_tiers =
                fields.declared_owned_array("factionRewardTiers", notes, |fields, notes| {
                    ScenarioFactionTiers::read(fields, &mut factions, notes)
                })?;
            Ok(ContractScenario {
                source,
                faction_reward_tiers,
            })
        },
    )?;
    Ok(ScenarioRead {
        scenario,
        notes,
        dependencies: factions.finish(),
    })
}

pub(crate) fn ordered_rows(
    mut rows: Vec<PreparedContractScenario<'_>>,
) -> Vec<ContractScenario<'_>> {
    PreparedDefinition::order(&mut rows);
    rows.into_iter()
        .map(PreparedDefinition::into_definition)
        .collect()
}

pub(crate) fn catalog_rows<'rows>(
    rows: &'rows [PreparedContractScenario<'_>],
) -> impl Iterator<Item = CatalogRow> + 'rows {
    PreparedDefinition::catalog_rows(rows, crate::Kind::ContractScenarios)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedContractScenario<'source>],
) -> impl Iterator<
    Item = (
        crate::Kind,
        p4kx_extraction::Guid,
        &'source str,
        &'rows [crate::catalog_number_capture::CapturedNumber],
    ),
> {
    PreparedDefinition::sources(crate::Kind::ContractScenarios, rows)
}
