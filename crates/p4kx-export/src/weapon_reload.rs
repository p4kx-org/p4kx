// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_assembly::Entities;
use p4kx_extraction::{Archive, CancelToken, Record, SourceUses, SourcedValue};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{EntryCapture, PhysicalEntryRef};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::catalog_item_components::item_definition::WeaponAttachment;
use crate::export_ledger::ExportLedger;

#[path = "weapon_reload_actor.rs"]
mod actor;
#[path = "weapon_reload_assets.rs"]
mod assets;
#[path = "weapon_reload_document.rs"]
mod document;
#[path = "weapon_reload_graph.rs"]
mod graph;
#[path = "weapon_reload_relation.rs"]
mod relation;
#[path = "weapon_reload_scope.rs"]
mod scope;
#[path = "weapon_reload_sources.rs"]
mod sources;

use assets::{Assets, FileSelector};
use document::check_cancel;
use graph::BranchLink;
use relation::{CandidateInput, FragmentMatch};
use scope::DatabaseScopes;
use sources::ActorBinding;

#[path = "weapon_reload_coverage.rs"]
mod coverage;

pub use actor::{MotionComponent as PlayerMotionComponent, PlayerMotion};
pub use assets::{
    AssetLink as MannequinAssetLink, AssetState as MannequinAssetState,
    Controller as MannequinController, Definition as MannequinTagDefinition,
    SelectorNode as MannequinSelectorNode,
};
pub use coverage::WeaponAnimationGraphCoverage;
pub use document::{
    Branch as MannequinSubDatabase, Database as MannequinDatabase, Node as MannequinNode,
    OptionRow as MannequinFragmentOption,
};
pub use graph::{BranchLink as MannequinBranch, BranchVisit as MannequinBranchVisit};
pub use relation::{
    FragmentMatch as MannequinFragmentSelector, FragmentStatus as MannequinFragmentStatus,
};
pub use relation::{Playback, TagEvidence, TagMatch, WeaponPlayerAnimationCandidate};
pub use scope::{
    DatabaseScopes as MannequinDatabaseScopes, Declaration as MannequinDeclaration,
    Scope as MannequinScope, ScopeImport as MannequinScopeImport,
};
pub use sources::{ActorBinding as MannequinActorBinding, WeaponAnimationSources};

pub(crate) struct WeaponAnimation<'source> {
    pub(crate) sources: WeaponAnimationSources<'source>,
    pub(crate) candidates: StagedCatalogTable,
}

pub(crate) fn read<'source>(
    archive: &Archive,
    entities: &Entities<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    weapons: &[WeaponAttachment<'source>],
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<WeaponAnimation<'source>, CatalogError> {
    let scratch = std::env::temp_dir();
    let mut candidates = StagedCatalogRows::new(&scratch, cancel);
    let players = entities.named_class_candidates("Player");
    let projected = project(archive, players, input, weapons, cancel, |row| {
        candidates.append(&row).map_err(CatalogError::from)
    })?;
    let candidates = candidates.finish(projected.availability)?;
    ledger.merge(projected.ledger);
    Ok(WeaponAnimation {
        sources: projected.sources,
        candidates,
    })
}

pub(crate) struct WeaponAnimationProjection<'source> {
    pub(crate) sources: WeaponAnimationSources<'source>,
    pub(crate) availability: DatasetAvailability,
    pub(crate) ledger: ExportLedger,
}

pub(crate) fn project<'source>(
    archive: &Archive,
    players: &[Record<'source>],
    input: &OriginalCatalogInput<'_, 'source>,
    weapons: &[WeaponAttachment<'source>],
    cancel: Option<&CancelToken>,
    emit: impl FnMut(WeaponPlayerAnimationCandidate) -> Result<(), CatalogError>,
) -> Result<WeaponAnimationProjection<'source>, CatalogError> {
    if weapons.is_empty() {
        return Ok(WeaponAnimationProjection {
            sources: WeaponAnimationSources::default(),
            availability: DatasetAvailability::Absent,
            ledger: ExportLedger::default(),
        });
    }
    project_graph(archive, players, input, weapons, cancel, emit)
}

pub(crate) fn project_sources<'source>(
    archive: &Archive,
    players: &[Record<'source>],
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<WeaponAnimationProjection<'source>, CatalogError> {
    project_graph(archive, players, input, &[], cancel, |_| Ok(()))
}

fn project_graph<'source>(
    archive: &Archive,
    players: &[Record<'source>],
    input: &OriginalCatalogInput<'_, 'source>,
    weapons: &[WeaponAttachment<'source>],
    cancel: Option<&CancelToken>,
    mut emit: impl FnMut(WeaponPlayerAnimationCandidate) -> Result<(), CatalogError>,
) -> Result<WeaponAnimationProjection<'source>, CatalogError> {
    let entry = EntryCapture::for_input(input);
    let mut assets = Assets::new(archive, &entry, cancel);
    let sources = entry.source_uses();
    let player = actor::read(players, input, cancel)?;
    let actor_bindings = bindings(&player, &mut assets)?;
    let roots = actor_bindings
        .iter()
        .filter_map(|row| row.database.source.clone())
        .collect::<Vec<_>>();
    let branches = graph::walk(&mut assets, roots, sources, cancel)?;
    let scopes = scope::databases(&mut assets, sources, cancel)?;
    let fragments = build_rows(
        &actor_bindings,
        &branches,
        &scopes,
        &assets,
        weapons,
        (sources, cancel),
        &mut emit,
    )?;
    let available = if assets.databases.is_empty() {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Available
    };
    Ok(WeaponAnimationProjection {
        sources: WeaponAnimationSources {
            player: Some(player),
            actor_bindings,
            controllers: assets.controllers.into_values().collect(),
            databases: assets.databases.into_values().collect(),
            tag_definitions: assets.definitions.into_values().collect(),
            scopes,
            branches,
            fragment_selectors: fragments,
        },
        availability: available,
        ledger: assets.ledger,
    })
}

fn bindings(
    player: &SourcedValue<Vec<PlayerMotion<'_>>>,
    assets: &mut Assets<'_, '_, '_>,
) -> Result<Vec<ActorBinding>, CatalogError> {
    let Some([actor]) = player.value().map(Vec::as_slice) else {
        return Ok(Vec::new());
    };
    actor
        .components
        .value()
        .into_iter()
        .flatten()
        .filter_map(|slot| slot.value().and_then(Option::as_ref))
        .map(|component| {
            Ok(ActorBinding {
                record: actor.source.record_index.clone(),
                component: component.ordinal.clone(),
                controller: assets.select(&component.action_controller, FileSelector::Direct)?,
                database: assets.select(&component.anim_database_3p, FileSelector::Direct)?,
            })
        })
        .collect()
}

fn build_rows(
    actors: &[ActorBinding],
    branches: &[BranchLink],
    scopes: &[DatabaseScopes],
    assets: &Assets<'_, '_, '_>,
    weapons: &[WeaponAttachment<'_>],
    (sources, cancel): (&SourceUses, Option<&CancelToken>),
    emit: &mut impl FnMut(WeaponPlayerAnimationCandidate) -> Result<(), CatalogError>,
) -> Result<Vec<FragmentMatch>, CatalogError> {
    let scope_by_entry = scopes
        .iter()
        .map(|scope| (&scope.database, scope))
        .collect::<BTreeMap<_, _>>();
    let branches_by_entry = (!weapons.is_empty()).then(|| graph::by_database(branches));
    let mut fragment_selectors = Vec::new();
    let indices = weapons_by_tag(weapons);
    for edge in branches {
        check_cancel(cancel)?;
        let target = edge.target.source.as_ref();
        let database = target.and_then(|target| assets.databases.get(target));
        let scope = target
            .and_then(|target| scope_by_entry.get(target))
            .copied();
        let fragments = relation::fragments(
            edge,
            scope_by_entry.get(&edge.source).copied(),
            scope,
            database.map(|database| database.options.as_slice()),
            sources,
        )?;
        if edge.is_resolved()
            && let (Some(database), Some(scope), Some(target), Some(by_entry)) =
                (database, scope, target, branches_by_entry.as_ref())
        {
            for actor in actors
                .iter()
                .filter(|actor| reachable(actor, &edge.source, by_entry))
            {
                for option in &database.options {
                    check_cancel(cancel)?;
                    let tags =
                        graph::attribute(&option.attributes, "Tags", &option.position, sources)?;
                    let positions = selected_weapons(&tags, &indices);
                    for index in positions {
                        let Some(weapon) = weapons.get(index) else {
                            continue;
                        };
                        let input = CandidateInput {
                            weapon,
                            actor: &actor.record,
                            component: &actor.component,
                            edge,
                            target_id: target,
                            target_scope: scope,
                            option,
                            fragment_results: &fragments,
                        };
                        if let Some(candidate) = input.project(sources)? {
                            emit(candidate)?;
                        }
                    }
                }
            }
        }
        fragment_selectors.extend(fragments);
    }
    Ok(fragment_selectors)
}

fn reachable(
    actor: &ActorBinding,
    selected: &PhysicalEntryRef,
    edges: &BTreeMap<&PhysicalEntryRef, Vec<&BranchLink>>,
) -> bool {
    let mut pending = actor.database.source.iter().collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    while let Some(next) = pending.pop() {
        if next == selected {
            return true;
        }
        if !visited.insert(next) {
            continue;
        }
        pending.extend(
            edges
                .get(next)
                .into_iter()
                .flatten()
                .filter_map(|edge| edge.target.source.as_ref()),
        );
    }
    false
}

fn weapons_by_tag(weapons: &[WeaponAttachment<'_>]) -> BTreeMap<String, Vec<usize>> {
    let mut names = BTreeMap::<_, Vec<_>>::new();
    for (index, weapon) in weapons.iter().enumerate() {
        if let Some(tags) = weapon.tags.value() {
            for tag in tags.split_whitespace().filter(|tag| tag.is_ascii()) {
                let positions = names.entry(tag.to_ascii_lowercase()).or_default();
                if positions.last() != Some(&index) {
                    positions.push(index);
                }
            }
        }
    }
    names
}

fn selected_weapons(
    option: &SourcedValue<String>,
    indices: &BTreeMap<String, Vec<usize>>,
) -> BTreeSet<usize> {
    option
        .value()
        .into_iter()
        .flat_map(|value| value.split('+'))
        .flat_map(|tag| {
            indices
                .get(&tag.trim().to_ascii_lowercase())
                .into_iter()
                .flatten()
                .copied()
        })
        .collect()
}
