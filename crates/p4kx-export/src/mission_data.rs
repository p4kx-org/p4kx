// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, StructValue, TreeBuilder, Value, ValueError};

use crate::contract_mission_data::{
    ContractScenario, MissionLocality, MissionType, ScenarioFactionTiers, ScenarioProgression,
    ScenarioTier,
};
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{key, one};
use crate::record_fields::{
    boolean, class_name, integer, number, reference, references, structs, text,
};
use crate::texts::Texts;

/// The `badgeToAward` the game stores for a tier without a badge.
const NO_BADGE: &str = "None";

/// Returns the mission type the Record tree `tree` describes, its name resolved by `texts`.
pub(crate) fn mission_type<'f>(tree: &RecordTree<'f>, texts: &Texts<'f>) -> MissionType<'f> {
    MissionType {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        type_name: texts.localised(key(Some(tree.root()), "LocalisedTypeName")),
    }
}

/// Returns the mission locality the Record tree `tree` describes.
pub(crate) fn mission_locality<'f>(tree: &RecordTree<'f>) -> MissionLocality<'f> {
    MissionLocality {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        starmap_object_guids: references(Some(tree.root()), "availableLocations"),
    }
}

/// Returns the contract scenario the Record tree `tree` describes.
pub(crate) fn contract_scenario<'f>(tree: &RecordTree<'f>) -> ContractScenario<'f> {
    ContractScenario {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        faction_reward_tiers: structs(Some(tree.root()), "factionRewardTiers")
            .map(faction_tiers)
            .collect(),
    }
}

fn faction_tiers<'f>(tiers: &StructValue<'f>) -> ScenarioFactionTiers<'f> {
    let faction = tiers.field("faction").and_then(|value| match value {
        // A Reference with a location points into a Record, not at one.
        Value::Reference(target) if target.location().is_none() => Some(target),
        _ => None,
    });
    ScenarioFactionTiers {
        faction_guid: match tiers.field("faction") {
            Some(Value::Error(ValueError::DanglingReference { guid })) => Some(CatalogGuid(*guid)),
            _ => faction.map(|target| CatalogGuid(target.guid())),
        },
        faction_reputation_guid: None,
        faction_name: faction.and_then(|target| target.record_path().rsplit('/').next()),
        points_as_percent: boolean(Some(tiers), "convertTiersPointsToPercent"),
        progressions: structs(Some(tiers), "tierProgressions")
            .map(progression)
            .collect(),
    }
}

fn progression<'f>(progression: &StructValue<'f>) -> ScenarioProgression<'f> {
    ScenarioProgression {
        min_multiplier_threshold: number(Some(progression), "minMultiplierThreshold"),
        completion_type: one(Some(progression), "completionType").map(StructValue::type_name),
        period_scenario_guid: reference(Some(progression), "periodMissionScenario"),
        tiers: structs(Some(progression), "tierRewards")
            .map(tier)
            .collect(),
    }
}

fn tier<'f>(tier: &StructValue<'f>) -> ScenarioTier<'f> {
    ScenarioTier {
        min_points: integer(Some(tier), "minPoints"),
        badge_to_award: text(Some(tier), "badgeToAward").filter(|badge| *badge != NO_BADGE),
        global_points_multiplier: number(Some(tier), "globalPointsMultiplier"),
        blueprint_pool_guids: references(Some(tier), "blueprintPool"),
    }
}

/// Sets the `faction_reputation_guid` of every faction in `scenarios` from the
/// `factionReputationRef` of its `Faction` Record; a Record that is missing, unreadable or names
/// none leaves `None`.
pub(crate) fn link_scenario_factions<'f>(
    forge: &'f DataForge<'f>,
    scenarios: &mut [ContractScenario<'f>],
    input: &'f crate::build::original_input::OriginalCatalogInput<'f, 'f>,
    cancel: Option<&p4kx_extraction::CancelToken>,
) -> Result<crate::export_ledger::ExportLedger, crate::CatalogError> {
    let capture = std::sync::Arc::new(
        crate::export_builder_capture::BuilderCapture::for_input_kind(
            input,
            crate::Kind::ContractScenarios,
            cancel,
        ),
    );
    let mut trees = TreeBuilder::with_read_observer(capture.clone());
    for tiers in scenarios
        .iter_mut()
        .flat_map(|scenario| scenario.faction_reward_tiers.iter_mut())
    {
        tiers.faction_reputation_guid = tiers
            .faction_guid
            .and_then(|faction| forge.record_by_guid(faction.0))
            .and_then(|record| trees.build(&record).ok())
            .and_then(|tree| reference(Some(tree.root()), "factionReputationRef"));
    }
    capture.finish()
}
