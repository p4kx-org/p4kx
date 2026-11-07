// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Record, RecordTree, StructValue, TreeBuilder,
};
use rayon::prelude::*;

use crate::blueprint::as_struct;
use crate::contract_missions::ContractEntry;
use crate::contract_starmap::StarmapObject;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::guid_text::CatalogGuid;
use crate::mission_entry_source::{MissionEntryContext, MissionEntrySource};
use crate::mission_index::MissionIndex;
use crate::mission_intros::{Grants, record_grants, resolve};
use crate::mission_layers::{Layer, items, one};
use crate::mission_places::{Places, Where};
use crate::mission_property_set::MissionPropertySets;
use crate::mission_property_set_registry::MissionPropertySetRegistry;
use crate::mission_results::granted_tags;
use crate::mission_siblings::{self, Siblings};
use crate::record_fields::{identity, text};
use crate::selection::{Kind, is_leftover};
use crate::spawn_layers::RecordSpawns;
use crate::spawn_pools::SpawnPools;
use crate::texts::Texts;
use crate::{CatalogError, CatalogFailure};

#[path = "mission_property_set_collision.rs"]
mod property_set_collision;

const CONTAINERS: [&str; 4] = [
    "contracts",
    INTRO_CONTRACTS,
    "legacyContracts",
    "PVPBountyContract",
];
const INTRO_CONTRACTS: &str = "introContracts";

/// The contracts of a DataForge and what was left out.
pub(crate) struct Missions<'f> {
    pub(crate) ledger: ExportLedger,
    pub(crate) contracts: Vec<ContractEntry<'f>>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) pools: SpawnPools<'f>,
    pub(crate) siblings: Siblings<'f>,
    pub(crate) property_sets: MissionPropertySets<'f>,
}

/// A source Record that cannot be read or projected into contract rows.
type PieceError = Box<dyn std::error::Error + Send + Sync>;

struct ContractPiece<'f> {
    contracts: Vec<ContractEntry<'f>>,
    skipped: usize,
    grants: Grants,
    pools: SpawnPools<'f>,
    property_sets: MissionPropertySets<'f>,
}

struct ContractSource<'t, 'f> {
    tree: &'t RecordTree<'f>,
    spawns: &'t RecordSpawns<'t, 'f>,
    property_sets: &'t MissionPropertySetRegistry<'t, 'f>,
}

impl<'t, 'f> ContractSource<'t, 'f> {
    fn handler(
        &self,
        handler: &'t StructValue<'f>,
        context: &MissionEntryContext<'_, 'f>,
        contracts: &mut Vec<ContractEntry<'f>>,
        grants: &mut Grants,
    ) -> Result<usize, PieceError> {
        let handler_layer = Layer::read(one(Some(handler), "contractParams"));
        let handler_where = Where::read(one(Some(handler), "defaultAvailability"), "prerequisites");
        let handler_spawns = self.spawns.handler(handler)?;
        let mut skipped = 0_usize;
        for container in CONTAINERS {
            for contract in items(Some(handler), container).iter().filter_map(as_struct) {
                let name = text(Some(contract), "debugName");
                let Some(id) = identity(contract)
                    .filter(|_| !is_leftover(self.tree.record_path(), name.unwrap_or_default()))
                else {
                    skipped = skipped.saturating_add(1);
                    continue;
                };
                if container == INTRO_CONTRACTS {
                    record_grants(grants, CatalogGuid(id), &granted_tags(contract));
                }
                let parent = MissionEntrySource {
                    tree: self.tree,
                    handler,
                    handler_layer: &handler_layer,
                    handler_where: &handler_where,
                    spawns: &handler_spawns,
                    contract,
                    id: CatalogGuid(id),
                    name,
                    property_sets: self.property_sets,
                };
                skipped = skipped.saturating_add(parent.entries(context, contracts)?);
            }
        }
        Ok(skipped)
    }
}

enum Piece<'f> {
    Contracts(ContractPiece<'f>),
    Failed(CatalogFailure),
}

struct ObservedPiece<'f> {
    piece: Piece<'f>,
    ledger: ExportLedger,
}

/// Reads each of `records` in parallel; a Record that cannot be read is a failure.
fn read_pieces<'f>(
    records: &[Record<'f>],
    context: &MissionEntryContext<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<ObservedPiece<'f>>, CatalogError> {
    records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            piece(trees, record, context, cancel)
        })
        .filter_map(Result::transpose)
        .collect()
}

/// Reads the contracts of the `ContractGenerator` Records `filter` matches: for each contract an
/// entry, and one for each of its sub contracts, ordered by the text of the GUIDs. The pickup
/// places are the starmap objects `starmap` lists.
///
/// A contract or sub contract whose debug name or Record is a development leftover, one without an
/// `id`, and an entry whose `id` pair an earlier entry already has, is left out and counted.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    texts: &Texts<'f>,
    starmap: &[StarmapObject<'f>],
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Missions<'f>, CatalogError> {
    let rule = Kind::Contracts.records();
    let records: Vec<Record<'f>> = forge
        .records()
        .filter(|record| {
            record
                .record_path()
                .is_ok_and(|path| rule.matches_path(path) && filter.is_none_or(|f| f.matches(path)))
        })
        .collect();
    let mut index = MissionIndex::read_catalog(forge, filter, cancel)?;
    let (pools, failures, ledger) = index.take_template_results();
    let mut missions = Missions {
        ledger,
        contracts: Vec::new(),
        skipped: 0,
        failures,
        pools,
        siblings: mission_siblings::read(forge, (texts, &index), filter, cancel)?,
        property_sets: MissionPropertySets::default(),
    };
    crate::mission_ledger::inventory(&mut missions.ledger);
    if records.is_empty() {
        return Ok(missions);
    }
    let places = Places::new(starmap);
    let context = MissionEntryContext {
        index: &index,
        texts,
        places: &places,
        unknowns: &missions.siblings.contract_templates.unknowns,
    };
    let mut grants = Grants::new();
    for observed in read_pieces(&records, &context, cancel)? {
        missions.take(observed, &mut grants)?;
    }
    property_set_collision::finalize(
        &missions.property_sets,
        &mut missions.contracts,
        &mut missions.ledger,
    )?;
    missions.order();
    missions
        .ledger
        .skipped_rows(Kind::Contracts, missions.skipped);
    resolve(&mut missions.contracts, &mut grants);
    Ok(missions)
}

impl<'f> Missions<'f> {
    fn take(
        &mut self,
        observed: ObservedPiece<'f>,
        grants: &mut Grants,
    ) -> Result<(), CatalogError> {
        self.ledger.merge(observed.ledger);
        match observed.piece {
            Piece::Contracts(ContractPiece {
                contracts,
                skipped,
                grants: granted,
                pools: found,
                property_sets,
            }) => {
                self.property_sets
                    .merge(property_sets)
                    .map_err(CatalogError::Json)?;
                self.pools.merge(found);
                self.contracts.extend(contracts);
                self.skipped = self.skipped.saturating_add(skipped);
                for (tag, intros) in granted {
                    grants.entry(tag).or_default().extend(intros);
                }
            }
            Piece::Failed(failure) => self.failures.push(failure),
        }
        Ok(())
    }
}

impl Missions<'_> {
    /// Orders the entries by the text of their GUIDs and drops an entry whose `guid` and
    /// `subContractGuid` an earlier one has, counting it as skipped.
    fn order(&mut self) {
        self.contracts.sort_by_cached_key(|entry| {
            (
                entry.guid.text(),
                entry.sub_contract_guid.map(CatalogGuid::text),
                entry.record_guid.text(),
            )
        });
        let listed = self.contracts.len();
        self.contracts.dedup_by(|later, earlier| {
            later.guid.0 == earlier.guid.0 && later.sub_contract_guid == earlier.sub_contract_guid
        });
        self.skipped = self
            .skipped
            .saturating_add(listed.saturating_sub(self.contracts.len()));
    }
}

fn piece<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    context: &MissionEntryContext<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Option<ObservedPiece<'f>>, CatalogError> {
    let rule = Kind::Contracts.records();
    match record.type_name() {
        Ok(kind) if kind != rule.type_name => return Ok(None),
        Err(error) => {
            return Ok(Some(failed(
                record,
                &error,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
                ExportLedger::default(),
            )));
        }
        _ => {}
    }
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return Ok(Some(failed(
                record,
                &error,
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
                ExportLedger::default(),
            )));
        }
    };
    let mut ledger = crate::export_field_walk::inspect(
        &tree,
        record,
        Some(Kind::Contracts),
        ExportSourceRole::Selected,
        cancel,
    )?;
    match project(&tree, context) {
        Ok(piece) => {
            crate::mission_ledger::observe(
                &mut ledger,
                record,
                ExportSourceRole::Selected,
                SourceDisposition::Projected,
                None,
            );
            Ok(Some(ObservedPiece {
                piece: Piece::Contracts(piece),
                ledger,
            }))
        }
        Err(error) => Ok(Some(failed(
            record,
            error.as_ref(),
            ExportSourceRole::Selected,
            SourceDisposition::ProjectionFailed,
            ledger,
        ))),
    }
}

fn failed<'f>(
    record: &Record<'_>,
    error: &(dyn std::error::Error + 'static),
    role: ExportSourceRole,
    disposition: SourceDisposition,
    mut ledger: ExportLedger,
) -> ObservedPiece<'f> {
    crate::mission_ledger::observe(&mut ledger, record, role, disposition, Some(error));
    ObservedPiece {
        piece: Piece::Failed(CatalogFailure::of(record, error)),
        ledger,
    }
}

fn project<'f>(
    tree: &RecordTree<'f>,
    context: &MissionEntryContext<'_, 'f>,
) -> Result<ContractPiece<'f>, PieceError> {
    let mut contracts = Vec::new();
    let mut skipped = 0_usize;
    let mut grants = Grants::new();
    let spawns = RecordSpawns::new(context.index.tags(), tree);
    let property_sets = MissionPropertySetRegistry::new(
        tree,
        context.index.tags(),
        context.texts,
        context.unknowns,
    )?;
    let source = ContractSource {
        tree,
        spawns: &spawns,
        property_sets: &property_sets,
    };
    for handler in items(Some(tree.root()), "generators")
        .iter()
        .filter_map(as_struct)
    {
        skipped = skipped.saturating_add(source.handler(
            handler,
            context,
            &mut contracts,
            &mut grants,
        )?);
    }
    let (property_sets, descriptions) = property_sets.finish();
    let mut pools = spawns.into_pools();
    pools.merge(descriptions);
    Ok(ContractPiece {
        contracts,
        skipped,
        grants,
        pools,
        property_sets,
    })
}
