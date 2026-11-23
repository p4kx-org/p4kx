// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{
    CancelToken, DataForgeError, Record, RecordTree, SourceUseRef, SourceUseRole, TreeBuilder,
};

use crate::build::original_input::{self, OriginalCatalogInput};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::catalog_item_components::read_item_purchase_rank;
use crate::entity_selection::Selected;
use crate::guid_text::CatalogGuid;
use crate::item::Item;
use crate::item_stats;
use crate::item_variant::{self, ComponentKeyRead, ItemComponentKey, ItemKey};
use crate::playability_rules::PurchaseRank;
use crate::record_fields;
use crate::record_header_field::RecordHeaderInput;
use crate::variant_decision::VariantDecision;
use crate::variant_evidence::VariantEvidence;
use crate::variants::{VariantBases, VariantMember, VariantRank};
use crate::{CatalogError, ExportAccountingError, Kind, Playability, VariantCounts};

#[path = "item_variant_survey_inputs.rs"]
mod inputs;
use self::inputs::{admission, observe_header};

#[path = "item_variant_survey_finish.rs"]
mod finishing;
use self::finishing::GroupComparison;

#[path = "item_variant_health.rs"]
mod health;
pub(crate) use self::health::project_for_catalog;

/// The physical rows requested by the caller and the component keys needed for them.
struct Selection<'source> {
    owners: HashSet<u32>,
    components: HashSet<ItemComponentKey<'source>>,
}

/// Applies unfiltered variant decisions to the physical Items emitted by the Catalog.
pub(crate) fn project<'source>(
    input: &OriginalCatalogInput<'source, 'source>,
    items: &mut [Item<'source>],
    cancel: Option<&CancelToken>,
) -> Result<VariantCounts, CatalogError> {
    let mut selection = Selection {
        owners: HashSet::new(),
        components: HashSet::new(),
    };
    for item in items.iter() {
        checkpoint(cancel)?;
        selection.owners.insert(index(item)?);
        if let Some(key) = item.variant_components {
            selection.components.insert(key);
        }
    }
    let mut variants = read(input, selection, cancel)?;
    let mut groups = HashSet::new();
    let mut count = 0;
    for item in items {
        checkpoint(cancel)?;
        let index = index(item)?;
        let decision = variants
            .remove(&index)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        if let Some(target) = decision.decision.value() {
            let base = target
                .target_record_index()
                .value()
                .copied()
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            groups.insert(base);
            count += 1;
        }
        item.variant_of = decision
            .decision
            .with_failed_source_uses(decision.failed_source_uses);
    }
    Ok(VariantCounts {
        groups: groups.len(),
        variants: count,
    })
}

fn index(item: &Item<'_>) -> Result<u32, CatalogError> {
    Ok(item
        .source
        .source()
        .record_index()
        .value()
        .copied()
        .ok_or(ExportAccountingError::MissingFieldInspection)?)
}

struct VariantRead {
    decision: VariantDecision,
    failed_source_uses: Vec<SourceUseRef>,
}

struct Owner<'source> {
    key: Option<ItemKey<'source>>,
    guid: CatalogGuid,
    evidence: VariantEvidence,
}

struct Cohort<'source> {
    selection: Selection<'source>,
    owners: HashMap<u32, Owner<'source>>,
    bases: VariantBases<'source, ItemKey<'source>>,
    group_origins: HashMap<ItemKey<'source>, GroupComparison>,
    failed_stats: HashMap<ItemComponentKey<'source>, Vec<VariantEvidence>>,
    census_evidence: Vec<VariantEvidence>,
    uncertain_types: HashMap<&'source str, Vec<VariantEvidence>>,
    requested_types: HashSet<&'source str>,
}

/// Reads the canonical unfiltered Item candidates required by the requested owners.
fn read<'source>(
    input: &OriginalCatalogInput<'source, 'source>,
    selection: Selection<'source>,
    cancel: Option<&CancelToken>,
) -> Result<HashMap<u32, VariantRead>, CatalogError> {
    if selection.owners.is_empty() {
        return Ok(HashMap::new());
    }
    let requested_types = selection
        .components
        .iter()
        .copied()
        .map(ItemComponentKey::kind)
        .collect();
    let mut state = Cohort {
        selection,
        owners: HashMap::new(),
        bases: VariantBases::new(),
        group_origins: HashMap::new(),
        failed_stats: HashMap::new(),
        census_evidence: Vec::new(),
        uncertain_types: HashMap::new(),
        requested_types,
    };
    let mut trees = TreeBuilder::new();
    let scan_all = !state.selection.components.is_empty();
    let mut inspected_headers = 0;
    for record in input.forge().records() {
        checkpoint(cancel)?;
        if !scan_all && !state.selection.owners.contains(&record.index()) {
            continue;
        }
        state.observe(input, &record, &mut trees, cancel)?;
        inspected_headers += 1;
    }
    state.finish(input, scan_all.then_some(inspected_headers))
}

impl<'source> Cohort<'source> {
    fn observe(
        &mut self,
        input: &OriginalCatalogInput<'source, 'source>,
        record: &Record<'source>,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let header = admission(record);
        let selected = self.selection.owners.contains(&record.index());
        let uses = &input.origins().uses;
        match header.selection {
            Ok(Some(Selected::Entity(Kind::Items))) => {}
            Err(_) => {
                let failed = observe_header(input, record, header.uses, false)?;
                self.census_evidence.push(failed.clone());
                self.retain_owner(record, selected, None, failed);
                return Ok(());
            }
            Ok(None)
                if header
                    .uses
                    .iter()
                    .any(|(_, role)| *role == SourceUseRole::Attempted) =>
            {
                let exclusion = observe_header(input, record, header.uses, true)?;
                self.census_evidence
                    .push(VariantEvidence::from_origin(exclusion.origin));
                return Ok(());
            }
            _ => return Ok(()),
        }
        let tree = match trees.build(record) {
            Ok(tree) => tree,
            Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
            Err(_) => {
                let body = input.origins().dataforge.at(
                    original_input::record_body_location(record.index()),
                    SourceUseRole::Attempted,
                )?;
                let incomplete = observe_header(input, record, header.uses, false)?;
                let body = VariantEvidence::from_uses(uses, &[body], false);
                let failed = VariantEvidence::join(uses, [&incomplete, &body], false)?;
                self.census_evidence.push(failed.clone());
                self.retain_owner(record, selected, None, failed);
                return Ok(());
            }
        };
        let candidate = CandidateRead {
            record,
            tree: &tree,
            admission: header.uses,
        };
        match item_variant::read_component_key(&tree) {
            ComponentKeyRead::Comparable(key)
                if selected || self.selection.components.contains(&key) =>
            {
                self.compare(input, candidate, trees, key, cancel)
            }
            ComponentKeyRead::Comparable(_) => Ok(()),
            ComponentKeyRead::Ineligible | ComponentKeyRead::UnresolvedLinks if !selected => Ok(()),
            status => self.observe_incomplete_key(input, candidate, status, cancel),
        }
    }

    fn observe_incomplete_key(
        &mut self,
        input: &OriginalCatalogInput<'source, 'source>,
        candidate: CandidateRead<'_, 'source>,
        status: ComponentKeyRead<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let CandidateRead {
            record,
            tree,
            admission,
        } = candidate;
        let selected = self.selection.owners.contains(&record.index());
        let known_type = item_variant::known_item_type(tree);
        if !selected && known_type.is_some_and(|kind| !self.requested_types.contains(kind)) {
            return Ok(());
        }
        let scope = input.record_scope(record, tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let components = item_variant::capture_component_key(tree, &originals, status)?;
        let header = observe_header(input, record, admission, true)?;
        let known = VariantEvidence::join(&input.origins().uses, [&header, &components], true)?;
        if matches!(status, ComponentKeyRead::Unavailable) {
            let affected = known_type.map_or(&mut self.census_evidence, |kind| {
                self.uncertain_types.entry(kind).or_default()
            });
            affected.push(known.clone());
        }
        self.retain_owner(record, selected, None, known);
        Ok(())
    }

    fn compare(
        &mut self,
        input: &OriginalCatalogInput<'source, 'source>,
        candidate: CandidateRead<'_, 'source>,
        trees: &mut TreeBuilder<'source>,
        component_key: ItemComponentKey<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let CandidateRead {
            record,
            tree,
            admission,
        } = candidate;
        let selected = self.selection.owners.contains(&record.index());
        let scope = input.record_scope(record, tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let components = item_variant::capture_component_key(
            tree,
            &originals,
            ComponentKeyRead::Comparable(component_key),
        )?;
        let header = observe_header(input, record, admission, true)?;
        let stats = item_stats::read_scoped(input, record, tree, &originals, trees, cancel)?;
        let uses = &input.origins().uses;
        let stats_origin =
            VariantEvidence::join(uses, [&components, &stats.evidence, &header], true)?;
        let Some(key) = component_key.with_stats(stats.stats.as_ref()) else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if !stats_origin.origin.source_set_complete() {
            self.failed_stats
                .entry(component_key)
                .or_default()
                .push(stats_origin.clone());
            self.retain_owner(record, selected, None, stats_origin);
            return Ok(());
        }
        let rank = read_item_purchase_rank(input, tree, &originals)?;
        let rank_evidence = VariantEvidence::from_origin(rank.origin().clone());
        let comparison = VariantEvidence::join(
            uses,
            [&stats_origin, &rank_evidence],
            rank.value().is_some(),
        )?;
        if comparison.origin.source_set_complete()
            && let Some(purchase) = rank.value()
        {
            self.bases.observe(
                key,
                VariantRank {
                    member: VariantMember {
                        guid: CatalogGuid(record.guid()),
                        record_index: record.index(),
                    },
                    class_name: record_fields::class_name(tree),
                    playability: (*purchase == PurchaseRank::Player).then_some(Playability::Player),
                },
            );
        }
        self.group_origins
            .entry(key)
            .or_insert_with(|| GroupComparison::new(record.index()))
            .observe(record.index(), comparison.clone());
        self.retain_owner(record, selected, Some(key), comparison);
        Ok(())
    }

    fn retain_owner(
        &mut self,
        record: &Record<'source>,
        selected: bool,
        key: Option<ItemKey<'source>>,
        evidence: VariantEvidence,
    ) {
        if selected {
            self.owners.insert(
                record.index(),
                Owner {
                    key,
                    guid: CatalogGuid(record.guid()),
                    evidence,
                },
            );
        }
    }
}

struct CandidateRead<'read, 'source> {
    record: &'read Record<'source>,
    tree: &'read RecordTree<'source>,
    admission: Vec<(RecordHeaderInput, SourceUseRole)>,
}
