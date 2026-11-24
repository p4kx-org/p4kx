// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Record, TreeBuilder};

use crate::catalog_queries::CatalogRecordCoverage;
use crate::entity_selection::{self, Selected};
use crate::scan::{CRAFTER_FIELD, CRAFTER_STRUCT};
use crate::{CatalogError, Kind};

pub(super) fn entity_candidates<'source, Candidate>(
    forge: &'source DataForge<'source>,
    mut may_match: Candidate,
) -> impl FnMut(
    &Record<'source>,
    &mut TreeBuilder<'source>,
    &mut CatalogRecordCoverage,
) -> Result<bool, CatalogError>
+ use<'source, Candidate>
where
    Candidate: FnMut(&Record<'source>, &mut TreeBuilder<'source>) -> bool,
{
    let crafter = forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT);
    move |record, trees, coverage| {
        let classified = record.type_name().and_then(|source_type| {
            if source_type == p4kx_assembly::ENTITY_TYPE {
                entity_selection::select(record, None)
            } else {
                Ok(None)
            }
        });
        match classified {
            Err(_) => coverage.unreadable_headers.push(record.index()),
            Ok(Some(Selected::Leftover(_))) => coverage.skipped_leftovers += 1,
            Ok(Some(Selected::Entity(kind))) => {
                if kind != Kind::Crafters {
                    return Ok(may_match(record, trees));
                }
                match crafter
                    .as_ref()
                    .map_or(Ok(false), |probe| trees.holds(probe, record))
                {
                    Ok(holds) => return Ok(holds && may_match(record, trees)),
                    Err(_) => coverage.unreadable_headers.push(record.index()),
                }
            }
            Ok(None) => {}
        }
        Ok(false)
    }
}
