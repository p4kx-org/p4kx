// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_assembly::PaintTagIndex;
use p4kx_extraction::CancelToken;
use rayon::prelude::*;

use crate::catalog_v3::tag_catalog::TagDependencies;
use crate::contract::{Item, Paint};
use crate::paint_port::VehicleFacts;
use crate::{CatalogError, ExportAccountingError, PaintSummary};

#[path = "paint_matching.rs"]
mod matching;

use matching::ShipPaintMatches;
pub(crate) use matching::{PaintOwner, PaintRequirements};

const PAINT_TYPE: &str = "Paints";

/// Returns whether `kind` is the `AttachDef.Type` of a paint, ignoring ASCII case.
pub(crate) const fn is_paint_type(kind: &str) -> bool {
    kind.eq_ignore_ascii_case(PAINT_TYPE)
}

/// Returns whether the selected item is a paint.
pub(crate) fn is_paint(item: &Item<'_>) -> bool {
    item.paint.item_type.is_some_and(is_paint_type)
}

/// The paints of a catalog and what was counted deriving them.
pub(crate) struct Paints<'f> {
    pub(crate) ledger: crate::export_ledger::ExportLedger,
    pub(crate) failures: Vec<crate::CatalogReadFailure<'f>>,
    pub(crate) list: Vec<Paint>,
    pub(crate) summary: PaintSummary,
    pub(crate) unreadable_files: Vec<String>,
}

/// Returns the paints among `items` with the ships of `vehicles` they fit.
///
/// A paint is an item whose `AttachDef.Type` is `Paints`. It fits a ship when a paint port of the
/// ship accepts it: every tag in the port's required tags is among the paint's `Tags` or above
/// one of them in the tag tree, and every tag in the paint's own `RequiredTags` is among the
/// port's tags and the ship Record's `AttachDef.Tags` or above one of them. The ports are the
/// `SItemPortDef` entries of the ship's Record and the `ItemPort` elements of its vehicle
/// implementation.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn paints<'f>(
    tags: Option<TagDependencies<'f>>,
    items: &[Item<'_>],
    vehicles: &[VehicleFacts],
    sources: &crate::export_ledger::ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Paints<'f>, CatalogError> {
    let candidates: Vec<_> = items
        .iter()
        .filter(|item| is_paint(item))
        .map(PaintCandidate::read)
        .collect::<Result<_, _>>()?;
    let mut ledger = crate::paint_ledger::inputs(
        sources,
        candidates
            .iter()
            .map(|candidate| candidate.owner.record_index),
        vehicles,
    )?;
    if candidates.is_empty() != tags.is_none() {
        return Err(ExportAccountingError::ConflictingTagTrace.into());
    }
    let Some(tags) = tags else {
        return Ok(Paints {
            ledger,
            failures: Vec::new(),
            list: Vec::new(),
            summary: PaintSummary::default(),
            unreadable_files: Vec::new(),
        });
    };
    let matches = ShipPaintMatches::read(vehicles)?;
    let mut unreadable_files: Vec<_> = vehicles
        .iter()
        .filter_map(|vehicle| vehicle.implementation.unreadable.clone())
        .collect();
    let unreadable = unreadable_files.len();
    unreadable_files.sort_unstable();
    unreadable_files.dedup();
    ledger.merge(tags.ledger);
    let index = PaintTagIndex::new(tags.tree.tree());
    let mut paints: Vec<Paint> = candidates
        .par_iter()
        .map(|candidate| candidate.project(&matches, &index, cancel))
        .collect::<Result<_, _>>()?;
    paints.sort_by_cached_key(|paint| paint.item_guid.text());
    let summary = summarise(&paints, unreadable);
    Ok(Paints {
        ledger,
        failures: tags.failures,
        list: paints,
        summary,
        unreadable_files,
    })
}

pub(crate) struct PaintCandidate<'item, 'source> {
    owner: PaintOwner,
    tags: &'item [&'source str],
    required_tags: &'item [&'source str],
}

impl<'item, 'source> PaintCandidate<'item, 'source> {
    fn read(item: &'item Item<'source>) -> Result<Self, ExportAccountingError> {
        let record_index = item
            .source
            .source()
            .record_index()
            .value()
            .copied()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        Ok(Self::from_authored(
            record_index,
            item.identity.guid,
            &item.paint.tags,
            &item.paint.required_tags,
        ))
    }

    pub(crate) const fn from_authored(
        record_index: u32,
        guid: crate::guid_text::CatalogGuid,
        tags: &'item [&'source str],
        required_tags: &'item [&'source str],
    ) -> Self {
        Self {
            owner: PaintOwner { record_index, guid },
            tags,
            required_tags,
        }
    }

    fn project(
        &self,
        matches: &ShipPaintMatches,
        index: &PaintTagIndex<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Paint, CatalogError> {
        self.project_accepted(
            matches.fit(self.tags, self.required_tags, index).collect(),
            cancel,
        )
        .map(|(row, _)| row)
    }

    pub(crate) fn project_accepted(
        &self,
        mut accepted: Vec<PaintOwner>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Paint, Vec<PaintOwner>), CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        accepted.sort_by_cached_key(|ship| (ship.guid.text(), ship.record_index));
        let row = Paint {
            item_guid: self.owner.guid,
            ship_guids: accepted.iter().map(|ship| ship.guid).collect(),
        };
        Ok((row, accepted))
    }
}

fn summarise(paints: &[Paint], unreadable_vehicles: usize) -> PaintSummary {
    let with_paints: HashSet<[u8; 36]> = paints
        .iter()
        .flat_map(|paint| paint.ship_guids.iter().map(|guid| guid.text()))
        .collect();
    let assigned = paints
        .iter()
        .filter(|paint| !paint.ship_guids.is_empty())
        .count();
    PaintSummary {
        paints: paints.len(),
        assigned,
        unassigned: paints.len().saturating_sub(assigned),
        ships: with_paints.len(),
        unreadable_vehicles,
    }
}
