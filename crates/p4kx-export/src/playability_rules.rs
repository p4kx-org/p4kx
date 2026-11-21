// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{
    CancelToken, FieldOrigin, Guid, Record, RecordTree, StructValue, TreeBuilder, Value,
    is_placeholder_key,
};
use rayon::prelude::*;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::export_ledger::ExportLedger;
use crate::playability::Playability;
use crate::record_fields::{
    boolean, component, elements, locale_key, number, reference, structure, text,
};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogFailure, CatalogReadFailure, Kind};

#[path = "playability_dependency_source.rs"]
mod dependency_source;

const MANIFEST_TYPE: &str = "CrewManifest";
const TAG_TYPE: &str = "Tag";
const UNMANNED_TAG: &str = "Unmanned";
const AI_RECORD_TAG: &str = "SystemsDesign";
const READY: &str = "ReadyToInclude";
const NOT_INCLUDED: &str = "DoNotInclude";
const SPACESHIPS: &str = "libs/foundry/records/entities/spaceships/";
const MISSION_PREFIXES: [&str; 2] = [
    "libs/foundry/records/entities/scitem/mission_entities/",
    "libs/foundry/records/entities/scitem/missionstorage/",
];

enum Found {
    Crewed(Guid),
    Unmanned(Guid),
}

/// The canonical manifest and tag evidence used by the ship playability projection.
#[derive(Debug, Default)]
pub(crate) struct Evidence {
    crewed: HashSet<Guid>,
    unmanned: HashSet<Guid>,
    unreadable: HashSet<Guid>,
    proofs: HashMap<u32, FieldOrigin>,
}

fn evidence_record(record: &Record<'_>) -> bool {
    record
        .type_name()
        .is_ok_and(|name| name == MANIFEST_TYPE || name == TAG_TYPE)
}

fn project_evidence(tree: &RecordTree<'_>) -> Option<Found> {
    if tree.type_name() == MANIFEST_TYPE {
        (!elements(Some(tree.root()), "crew").is_empty()).then(|| Found::Crewed(tree.guid()))
    } else {
        (text(Some(tree.root()), "tagName") == Some(UNMANNED_TAG))
            .then(|| Found::Unmanned(tree.guid()))
    }
}

impl Evidence {
    /// Reads the crew manifests and tags required by playability indexing.
    pub(crate) fn read(records: &[Record<'_>]) -> (Self, Vec<CatalogFailure>) {
        let found = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                evidence_record(record).then(|| {
                    trees
                        .build(record)
                        .map(|tree| project_evidence(&tree))
                        .map_err(|source| CatalogFailure::of(record, &source))
                })
            })
            .flatten()
            .collect();
        Self::gather(found, |failure: &CatalogFailure| failure.guid)
    }

    /// Reads playability dependencies and retains their actual canonical field inspections.
    #[cfg(test)]
    pub(crate) fn read_catalog<'f>(
        records: &[Record<'f>],
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<CatalogReadFailure<'f>>, ExportLedger), CatalogError> {
        Self::read_observed(records, None, cancel)
    }

    pub(crate) fn read_captured<'f>(
        records: &[Record<'f>],
        input: &OriginalCatalogInput<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<CatalogReadFailure<'f>>, ExportLedger), CatalogError> {
        Self::read_observed(records, Some(input), cancel)
    }

    /// Reads the physical manifest and tag aliases reached by the selected Ships.
    pub(crate) fn read_for_ships<'tree, 'source: 'tree>(
        ships: impl IntoIterator<Item = &'tree RecordTree<'source>>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, ExportLedger), CatalogError> {
        let mut targets = HashSet::new();
        for tree in ships {
            checkpoint(cancel)?;
            targets.extend(manifest_guid(tree));
            targets.extend(tag_guids(tree));
        }
        let mut records = Vec::new();
        for guid in targets {
            checkpoint(cancel)?;
            records.extend(input.forge().records_by_guid(guid));
        }
        records.sort_by_key(Record::index);
        let (evidence, _, ledger) = Self::read_captured(&records, input, cancel)?;
        Ok((evidence, ledger))
    }

    fn read_observed<'f>(
        records: &[Record<'f>],
        input: Option<&OriginalCatalogInput<'_, 'f>>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<CatalogReadFailure<'f>>, ExportLedger), CatalogError> {
        let found = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                if !evidence_record(record) {
                    return Ok(None);
                }
                let admission = match input {
                    Some(input) => RecordObservation::capture(input, record)?,
                    None => RecordObservation::accounting(record),
                };
                let read = crate::export_dependency_read::read(
                    input,
                    trees,
                    &admission,
                    Kind::Ships,
                    cancel,
                )?;
                let proof = input
                    .map(|input| {
                        dependency_source::observe(
                            input,
                            &admission,
                            read.tree.as_ref().ok(),
                            cancel,
                        )
                    })
                    .transpose()?;
                let outcome = read
                    .tree
                    .map(|tree| project_evidence(&tree))
                    .map_err(|error| CatalogReadFailure::of(record, &error));
                Ok(Some((outcome, read.ledger, record.index(), proof)))
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, CatalogError>>()?;
        let mut ledger = ExportLedger::default();
        let mut outcomes = Vec::new();
        let mut proofs = HashMap::new();
        for (outcome, observed, index, proof) in found {
            ledger.merge(observed);
            outcomes.push(outcome);
            if let Some(proof) = proof {
                proofs.insert(index, proof);
            }
        }
        let (mut evidence, failures) =
            Self::gather(outcomes, |failure: &CatalogReadFailure<'f>| failure.guid());
        evidence.proofs = proofs;
        Ok((evidence, failures, ledger))
    }

    fn gather<E>(
        found: Vec<Result<Option<Found>, E>>,
        guid: impl Fn(&E) -> Guid,
    ) -> (Self, Vec<E>) {
        let mut evidence = Self::default();
        let mut failures = Vec::new();
        for result in found {
            match result {
                Ok(Some(Found::Crewed(guid))) => {
                    evidence.crewed.insert(guid);
                }
                Ok(Some(Found::Unmanned(guid))) => {
                    evidence.unmanned.insert(guid);
                }
                Ok(None) => {}
                Err(failure) => {
                    evidence.unreadable.insert(guid(&failure));
                    failures.push(failure);
                }
            }
        }
        (evidence, failures)
    }
}

fn inclusion_mode<'f>(tree: &RecordTree<'f>) -> Option<&'f str> {
    let Some(Value::Array(entries)) = tree.field("StaticEntityClassData") else {
        return None;
    };
    entries.iter().find_map(inclusion_text)
}

pub(crate) fn inclusion_text<'source>(entry: &Value<'source>) -> Option<&'source str> {
    match entry {
        Value::Struct(value) if value.type_name() == "EAEntityDataParams" => {
            text(Some(value), "inclusionMode")
        }
        _ => None,
    }
}

pub(crate) fn spacecraft(path: &str) -> bool {
    under_path(path, SPACESHIPS)
}

fn under_path(path: &str, prefix: &str) -> bool {
    path.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

fn manifest_guid(tree: &RecordTree<'_>) -> Option<Guid> {
    reference(
        component(tree, "SEntityComponentCrewManifestParams"),
        "crewManifest",
    )
    .map(|guid| guid.0)
}

fn tag_guids<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> impl Iterator<Item = Guid> + use<'tree, 'source> {
    elements(Some(tree.root()), "tags")
        .iter()
        .filter_map(|tag| {
            if let Value::Reference(tag) = tag {
                Some(tag.guid())
            } else {
                None
            }
        })
}

fn explodes_on_death(vehicle: Option<&StructValue<'_>>) -> bool {
    boolean(
        structure(vehicle, "aiEngineeringOptions"),
        "powerPlantsCanExplodeOnDeath",
    ) == Some(true)
}

/// Returns the playability of the ship or ground vehicle `tree` with its `VehicleComponentParams`
/// `vehicle`, `None` when no rule applies.
///
/// The first rule that holds decides:
///
/// 1. `ai`: `SEntityComponentCrewManifestParams.crewManifest` links to a `CrewManifest` that lists
///    a crew; or, for a record path under `entities/spaceships/`, either
///    `VehicleComponentParams.criticalPartExplosionChance` is above zero or
///    `VehicleComponentParams.aiEngineeringOptions.powerPlantsCanExplodeOnDeath` is true while
///    `EAEntityDataParams.inclusionMode` is not `ReadyToInclude`; or the Record tag (`$tag`) is
///    `SystemsDesign`.
/// 2. `mission`: the Record carries a `Tag` named `Unmanned`.
/// 3. `special`: `inclusionMode` is `DoNotInclude`.
/// 4. `player`: `inclusionMode` is `ReadyToInclude`.
///
/// Any other `inclusionMode` value matches neither rule 3 nor rule 4. A crew manifest or a tag
/// the Record links to that could not be read gives `None` before any rule, as its evidence
/// is missing.
pub(crate) fn ship(
    tree: &RecordTree<'_>,
    vehicle: Option<&StructValue<'_>>,
    evidence: &Evidence,
) -> Option<Playability> {
    ship_decision(tree, vehicle, evidence).0
}

pub(crate) fn ship_decision(
    tree: &RecordTree<'_>,
    vehicle: Option<&StructValue<'_>>,
    evidence: &Evidence,
) -> (Option<Playability>, bool) {
    let manifest = manifest_guid(tree);
    let unreadable_link = manifest.is_some_and(|guid| evidence.unreadable.contains(&guid))
        || tag_guids(tree).any(|guid| evidence.unreadable.contains(&guid));
    if unreadable_link {
        return (None, true);
    }
    let inclusion = inclusion_mode(tree);
    let ready = inclusion == Some(READY);
    let spaceship = spacecraft(tree.record_path());
    let hull_explodes = spaceship && explodes_on_death(vehicle);
    let crewed = manifest.is_some_and(|guid| evidence.crewed.contains(&guid));
    let critical = spaceship
        && number(vehicle, "criticalPartExplosionChance").is_some_and(|chance| chance > 0.0);
    let ai_authored = tree.tag() == Some(AI_RECORD_TAG);
    let decision = if crewed || critical || ai_authored || (hull_explodes && !ready) {
        Some(Playability::Ai)
    } else if tag_guids(tree).any(|guid| evidence.unmanned.contains(&guid)) {
        Some(Playability::Mission)
    } else if inclusion == Some(NOT_INCLUDED) {
        Some(Playability::Special)
    } else if ready {
        Some(Playability::Player)
    } else {
        None
    };
    (decision, false)
}

fn real_key(key: Option<&str>) -> bool {
    key.is_some_and(|key| !is_placeholder_key(key))
}

/// Returns the playability of the item `tree`, `None` when no rule applies.
///
/// `name_key` is the localisation key of `AttachDef.Localization.Name`. The first rule that holds
/// decides:
///
/// 1. `player`: `SCItemPurchasableParams.displayName` is a localisation key that is not a
///    placeholder.
/// 2. `mission`: the record path lies under `entities/scitem/mission_entities/` or
///    `entities/scitem/missionstorage/`.
/// 3. `special`: `name_key` is no key or a placeholder, so the item has no player-visible name.
pub(crate) fn item(tree: &RecordTree<'_>, name_key: Option<&str>) -> Option<Playability> {
    let shop = locale_key(component(tree, "SCItemPurchasableParams"), "displayName");
    item_inputs(shop, tree.record_path(), name_key).value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemRule {
    Purchasable,
    MissionPath,
    Name,
}

pub(crate) struct ItemDecision {
    pub(crate) value: Option<Playability>,
    pub(crate) rule: ItemRule,
}

pub(crate) fn item_inputs(shop: Option<&str>, path: &str, name_key: Option<&str>) -> ItemDecision {
    let (value, rule) = if real_key(shop) {
        (Some(Playability::Player), ItemRule::Purchasable)
    } else if MISSION_PREFIXES
        .iter()
        .any(|prefix| under_path(path, prefix))
    {
        (Some(Playability::Mission), ItemRule::MissionPath)
    } else {
        (
            (!real_key(name_key)).then_some(Playability::Special),
            ItemRule::Name,
        )
    };
    ItemDecision { value, rule }
}

#[cfg(test)]
#[path = "playability_evidence_tests.rs"]
mod evidence_tests;
