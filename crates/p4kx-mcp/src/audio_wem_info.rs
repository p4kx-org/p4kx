// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{EntryOrigin, SourceInput};

use crate::archives::Opened;
use crate::audio_params::WemParams;
use crate::audio_wem_input::{Candidate, Held, candidates, read};
use crate::audio_wem_shapes::{WemInfoResult, WemShape};
use crate::budget::fit_prefix;
use crate::error::ToolError;
use crate::page::Page;

use super::payload_origins::{EvidenceError, PayloadPlan};

#[path = "audio_wem_info_origins.rs"]
mod origins;

/// Describes the first complete WEM and returns one page of its known locations.
pub(crate) fn info(
    opened: &Opened,
    params: &WemParams,
    page: Page,
) -> Result<WemInfoResult, ToolError> {
    let (id, places) = candidates(opened, params.wem_id, params.path.as_deref())?;
    let total = places.len();
    page.check(total)?;
    let selection = select(opened, id, &places)?;
    let available = page.slice(&places);
    let from_path = params.wem_id.is_none();
    let archive = opened.archive();
    let archive_path = archive.path().display().to_string();
    let response = |count| {
        let rows = available.get(..count).ok_or(EvidenceError::InvalidPrefix)?;
        let next_cursor = page.next_cursor_after(count, total)?;
        let (field_origins, provenance) =
            origins::capture(archive, &selection, rows, from_path, next_cursor.is_some())?;
        Ok(WemInfoResult {
            archive: archive_path.clone(),
            wem_id: id,
            locations_total: total,
            locations: rows.iter().map(|place| place.location.clone()).collect(),
            next_cursor,
            described: selection.candidate.location.clone(),
            wem: selection.description.shape.clone(),
            field_origins,
            provenance,
        })
    };
    let (_, result) = fit_prefix(available.len(), response, || {
        available.first().map_or_else(
            || "WEM description".to_owned(),
            |place| format!("WEM location {}", place.location.path),
        )
    })?;
    Ok(result)
}

struct Selection<'a> {
    candidate: &'a Candidate,
    description: Description,
    choice_incomplete: bool,
}

struct Description {
    shape: WemShape,
    payload: PayloadPlan,
    owner: Option<EntryOrigin>,
    parsed: bool,
}

fn select<'a>(
    opened: &Opened,
    id: Option<u32>,
    places: &'a [Candidate],
) -> Result<Selection<'a>, ToolError> {
    let mut fallback = None;
    let mut failure = None;
    for (ordinal, place) in places.iter().enumerate() {
        let outcome = read(opened, place, |held| {
            Ok(describe(held, id, place.location.len))
        });
        match outcome {
            Ok(description) => {
                let choice_incomplete = ordinal != 0 || (!description.parsed && places.len() > 1);
                let selection = Selection {
                    candidate: place,
                    description,
                    choice_incomplete,
                };
                if selection.description.parsed {
                    return Ok(selection);
                }
                fallback.get_or_insert(selection);
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    fallback.ok_or_else(|| failure.unwrap_or(ToolError::WemSelector))
}

fn describe(held: Held<'_>, id: Option<u32>, size: u64) -> Description {
    match held {
        Held::Parsed(parsed) => Description {
            shape: parsed.wem.shape(id, size),
            payload: PayloadPlan::parsed(&parsed.wem.origin_plan(), size, parsed.embedded.as_ref()),
            owner: owned_archive(parsed.source),
            parsed: true,
        },
        Held::Prefetch(embedded) => Description {
            shape: super::unreadable(id, size, None),
            payload: PayloadPlan::embedded_failure(&embedded.wem, None),
            owner: owned_archive(embedded.source),
            parsed: false,
        },
        Held::Unreadable(embedded, error) => Description {
            shape: super::unreadable(id, size, Some(&error)),
            payload: PayloadPlan::embedded_failure(&embedded.wem, Some(&error)),
            owner: owned_archive(embedded.source),
            parsed: false,
        },
    }
}

fn owned_archive(input: SourceInput<'_>) -> Option<EntryOrigin> {
    match input {
        SourceInput::Archive(origin) => Some(origin.clone()),
        _ => None,
    }
}
