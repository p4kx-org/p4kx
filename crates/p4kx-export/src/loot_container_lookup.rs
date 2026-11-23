// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;

use p4kx_extraction::{CancelToken, SourceUses, SourceValueState};

use super::super::selection::{ContainerKey, SelectedPlacement};
use crate::CatalogError;
use crate::catalog_queries::loot::LootContainerQuery;
use crate::catalog_query_session::checkpoint;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::{NoObject, Objects};
use crate::loot_walk::{self, Placed, Summary};
use crate::starmap_walk::{Opened, OpenedParent};

pub(super) struct Candidate<'source> {
    pub(super) root: &'source Opened,
    placed: &'source Placed,
}

pub(super) fn collect_candidates<'source>(
    roots: impl Iterator<Item = &'source Opened>,
    summaries: &'source HashMap<usize, Summary>,
    query: LootContainerQuery<'_>,
) -> Vec<Candidate<'source>> {
    let mut candidates = Vec::new();
    let mut by_seed = HashMap::new();
    for root in roots {
        let first = by_seed.entry(root.entry).or_insert_with(|| {
            loot_walk::visit_below(root.entry, summaries, |placed| {
                if placed.config == query.multi_config_guid && placed.path == query.container {
                    ControlFlow::Break(placed)
                } else {
                    ControlFlow::Continue(())
                }
            })
            .break_value()
        });
        if let Some(placed) = *first {
            candidates.push(Candidate { root, placed });
        }
    }
    candidates
}

pub(super) fn first_match<'source>(
    candidates: &[Candidate<'source>],
    objects: &Objects<'_>,
    desired: &ContainerKey<'_>,
    sources: &SourceUses,
    survey_complete: bool,
    cancel: Option<&CancelToken>,
) -> Result<(Option<SelectedPlacement<'source>>, bool), CatalogError> {
    let mut complete = true;
    for &Candidate { root, placed } in candidates {
        checkpoint(cancel)?;
        let location = objects.location(root.under);
        complete &= parent_complete(objects, root, location, sources, survey_complete)?;
        let candidate = SelectedPlacement {
            root,
            placed,
            location,
        };
        if candidate.key() == *desired {
            return Ok((Some(candidate), complete));
        }
    }
    Ok((None, complete))
}

fn parent_complete(
    objects: &Objects<'_>,
    root: &Opened,
    location: Result<CatalogGuid, NoObject>,
    sources: &SourceUses,
    survey_complete: bool,
) -> Result<bool, CatalogError> {
    if survey_complete
        && matches!(root.parent, OpenedParent::DirectoryFallback)
        && location == Err(NoObject::NoneAbove)
    {
        return Ok(true);
    }
    let field = objects.location_origin(root.under, location, sources)?;
    let (_, route_complete) = root.parent.inputs()?;
    Ok(route_complete
        && field.origin().source_set_complete()
        && matches!(
            field.state(),
            SourceValueState::Present | SourceValueState::Absent
        ))
}
