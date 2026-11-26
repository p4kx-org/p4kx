// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use p4kx_extraction::{CancelToken, Language};
use serde::Serialize;

use super::ReducedError;
use super::completeness;
use super::item::Item;
use super::manufacturer_definitions;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use super::ship::Ship;
use super::ship_cargo::ShipCargo;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogSequence, StagedCatalogTable};
use crate::contract_entities::Paint;
use crate::guid_text::CatalogGuid;
use crate::paint::PaintCandidate;
use crate::ship::ShipIdentity;
use crate::ship_store_pledge::{ShipStorePledges, ship_store_pledges};
use crate::ship_store_slugs::StoreSlugs;
use crate::source_value_catalog::SourceRecordReference;

const ITEMS: &str = "items";
const SHIPS: &str = "ships";
const PAINTS: &str = "paints";
const STORE_SLUGS: &str = "shipStoreSlugs";

pub(super) struct Entities {
    pub(super) items: StagedCatalogSequence,
    pub(super) ships: StagedCatalogSequence,
    pub(super) paints: StagedCatalogSequence,
    pub(super) ship_store_slugs: StagedCatalogSequence,
    pub(super) ship_store_pledges: ShipStorePledges,
    pub(super) manufacturers: StagedCatalogTable,
    pub(super) ship_cargo: BTreeMap<u32, ShipCargo>,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Entities, ReducedError> {
    if ![Language::English, Language::German]
        .into_iter()
        .all(|language| parsed.languages().contains(&language))
        || parsed.languages().len() != 2
    {
        return Err(ReducedError::IncompleteScope("texts"));
    }
    let (manufacturers, manufacturer_targets) =
        manufacturer_definitions::collect(parsed, scratch, cancel, texts)?;
    let ships = collect_ships(parsed, scratch, cancel, texts, &manufacturer_targets)?;
    let (items, paints) = collect_items(parsed, scratch, cancel, &ships.targets)?;
    Ok(Entities {
        items,
        ships: ships.rows,
        paints,
        ship_store_slugs: ships.store_slugs,
        ship_store_pledges: ship_store_pledges()?,
        manufacturers,
        ship_cargo: ships.cargo,
    })
}

struct StagedShips {
    rows: StagedCatalogSequence,
    store_slugs: StagedCatalogSequence,
    targets: RecordTargets,
    cargo: BTreeMap<u32, ShipCargo>,
}

fn collect_ships(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
    manufacturer_targets: &RecordTargets,
) -> Result<StagedShips, ReducedError> {
    let mapping = StoreSlugs::embedded()?;
    parsed.with_ships(CatalogRecordSelection::All, cancel, |snapshot| {
        let sources = snapshot.sources();
        completeness::all_fields(SHIPS, sources.availability(), sources.coverage())?;
        if !sources.dependency_coverage().is_complete() {
            return Err(ReducedError::IncompleteScope(SHIPS));
        }
        let mut targets = RecordTargets::default();
        for row in snapshot.rows() {
            checkpoint(cancel)?;
            targets.insert(SHIPS, row.source())?;
        }
        let mut staged = StagedCatalogRows::new(scratch, cancel);
        let mut references = TextRefs::default();
        let mut identities = Vec::new();
        let mut cargo = BTreeMap::new();
        for row in snapshot.rows() {
            targets.verify_variant(
                SHIPS,
                row.variant_of()
                    .and_then(p4kx_extraction::SourcedValue::value),
            )?;
            staged.append(&Ship::project(row, &mut references, |reference| {
                verify_manufacturer(reference, manufacturer_targets)
            })?)?;
            identities.push(ShipIdentity::read(row.source())?);
            let base = ShipCargo::from(row.base_facts());
            let index = base
                .ship_record_index
                .value
                .ok_or(ReducedError::IncompleteScope(SHIPS))?;
            if cargo.insert(index, base).is_some() {
                return Err(ReducedError::IncompleteScope(SHIPS));
            }
        }
        texts.add(
            references,
            sources.public_names(),
            sources.game_texts(),
            cancel,
        )?;
        let ships = staged.finish_sequence()?;
        completeness::rows(SHIPS, snapshot.rows().len(), ships.len())?;
        let slugs = collect_store_slugs(&mapping, &identities, scratch, cancel)?;
        Ok(StagedShips {
            rows: ships,
            store_slugs: slugs,
            targets,
            cargo,
        })
    })?
}

fn collect_store_slugs(
    mapping: &StoreSlugs,
    ships: &[ShipIdentity<'_>],
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogSequence, ReducedError> {
    let resolved = mapping.resolve(
        ships.iter().map(|ship| (ship.guid.get(), ship.class_name)),
        false,
    );
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for row in &resolved.exported {
        staged.append(row)?;
    }
    let rows = staged.finish_sequence()?;
    completeness::rows(STORE_SLUGS, resolved.exported.len(), rows.len())?;
    Ok(rows)
}

fn verify_manufacturer(
    reference: &SourceRecordReference<'_>,
    targets: &RecordTargets,
) -> Result<(), ReducedError> {
    if let Some(index) = reference.resolved_index() {
        let guid = reference
            .guid()
            .value()
            .copied()
            .ok_or(ReducedError::IncompleteScope(SHIPS))?;
        targets.verify(manufacturer_definitions::PATH, index, guid)?;
    }
    Ok(())
}

fn collect_items(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    ship_targets: &RecordTargets,
) -> Result<(StagedCatalogSequence, StagedCatalogSequence), ReducedError> {
    parsed.with_items(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::all_fields(ITEMS, snapshot.availability(), snapshot.coverage())?;
        let mut targets = RecordTargets::default();
        for item in snapshot.rows() {
            checkpoint(cancel)?;
            targets.insert(ITEMS, item.source().source())?;
        }
        let mut staged = StagedCatalogRows::new(scratch, cancel);
        let mut candidates = Vec::new();
        for item in snapshot.rows() {
            targets.verify_variant(ITEMS, item.variant_of().value())?;
            staged.append(&Item::from(item))?;
            if !item.paint.source_set_complete {
                return Err(ReducedError::IncompleteScope(PAINTS));
            }
            if crate::paint::is_paint(item) {
                let index = item
                    .source()
                    .source()
                    .record_index()
                    .value()
                    .copied()
                    .ok_or(ReducedError::IncompleteScope(PAINTS))?;
                candidates.push(Candidate {
                    index,
                    guid: item.identity.guid,
                    tags: &item.paint.tags,
                    required_tags: &item.paint.required_tags,
                });
            }
        }
        let items = staged.finish_sequence()?;
        completeness::rows(ITEMS, snapshot.rows().len(), items.len())?;
        let paints = collect_paints(parsed, &candidates, scratch, cancel, ship_targets)?;
        Ok((items, paints))
    })?
}

struct Candidate<'source> {
    index: u32,
    guid: CatalogGuid,
    tags: &'source [&'source str],
    required_tags: &'source [&'source str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaintRow<'source> {
    #[serde(flatten)]
    compatibility: &'source Paint,
    item_record_index: u32,
    ship_record_indices: Vec<u32>,
}

fn collect_paints(
    parsed: &ParsedCatalogQueries<'_>,
    candidates: &[Candidate<'_>],
    scratch: &Path,
    cancel: Option<&CancelToken>,
    ships: &RecordTargets,
) -> Result<StagedCatalogSequence, ReducedError> {
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    if !candidates.is_empty() {
        parsed.with_paint_matches(cancel, |matches, index, complete| {
            if !complete {
                return Err(ReducedError::IncompleteScope(PAINTS));
            }
            for paint in candidates {
                checkpoint(cancel)?;
                let candidate = PaintCandidate::from_authored(
                    paint.index,
                    paint.guid,
                    paint.tags,
                    paint.required_tags,
                );
                let (row, owners) = candidate.project_fit(matches, index, cancel)?;
                let mut ship_record_indices = Vec::with_capacity(owners.len());
                for owner in owners {
                    ships.verify(PAINTS, owner.record_index, owner.guid.get())?;
                    ship_record_indices.push(owner.record_index);
                }
                staged.append(&PaintRow {
                    compatibility: &row,
                    item_record_index: paint.index,
                    ship_record_indices,
                })?;
            }
            Ok::<_, ReducedError>(())
        })?;
    }
    let rows = staged.finish_sequence()?;
    completeness::rows(PAINTS, candidates.len(), rows.len())?;
    Ok(rows)
}
