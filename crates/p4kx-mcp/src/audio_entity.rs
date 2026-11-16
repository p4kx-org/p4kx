// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_assembly::audio::{
    AudioIndex, EntityAudio, PositionedTriggerReference, TriggerOccurrence, TriggerScope,
    positioned_trigger_references_in,
};
use p4kx_extraction::{Archive, DataForgeOrigin, Record, TreeBuilder};

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::EntitySoundsParams;
use crate::audio_shapes::EntitySoundsResult;
use crate::budget::{Budget, MAX_RESPONSE_LEN, fill_within, fit_prefix, json_len, structured_len};
use crate::budget_text;
use crate::error::ToolError;
use crate::page::Page;

use super::{event, header};

#[path = "audio_entity_event_origins.rs"]
mod event_origins;
#[path = "audio_entity_origins.rs"]
mod origins;
#[path = "audio_entity_record_origins.rs"]
mod record_origins;
#[path = "audio_entity_surveys.rs"]
mod surveys;

use self::surveys::SurveyMode;

/// Returns one page of an actual Entity's audio references with original source evidence.
pub(crate) fn entity_sounds(
    opened: &Arc<Opened>,
    params: &EntitySoundsParams,
    page: Page,
) -> Result<EntitySoundsResult, ToolError> {
    opened.audio_index();
    let held = Arc::clone(opened);
    let identifier = params.entity.clone();
    opened.with_dataforge_origin(move |forge, _, input| {
        let record = Entities::new(forge).find(&identifier)?;
        let tree = TreeBuilder::new().build_positioned(&record)?;
        let tree_complete = tree.tree().errors().next().is_none();
        let (references, occurrences) =
            positioned_trigger_references_in(&tree, TriggerScope::default())?
                .into_iter()
                .map(PositionedTriggerReference::into_parts)
                .unzip();
        let index = held.audio_index();
        Listing {
            archive: held.archive(),
            record: &record,
            input,
            index: &index,
            audio: index.resolve(references),
            occurrences,
            tree_complete,
        }
        .page(page)
    })
}

struct Listing<'archive, 'record, 'index> {
    archive: &'archive Archive,
    record: &'record Record<'record>,
    input: &'record DataForgeOrigin,
    index: &'index AudioIndex,
    audio: EntityAudio<'index>,
    occurrences: Vec<Vec<TriggerOccurrence>>,
    tree_complete: bool,
}

impl Listing<'_, '_, '_> {
    fn page(&self, page: Page) -> Result<EntitySoundsResult, ToolError> {
        let total = self.audio.triggers().len();
        page.check(total)?;
        let mut text = Budget::new();
        let entity = header::full(self.record, &mut text)?;
        let (issues, issues_total) = convert::issues(self.index, &mut text)?;
        let archive = budget_text::display(&mut text, &self.archive.path().display())?;
        let matched = self.audio.matched_count();
        let candidates = page.slice(self.audio.triggers());
        let reserved = json_len(&(&archive, &entity, total, matched, &issues, issues_total));
        let (rows, _) = fill_within(
            page,
            total,
            reserved,
            candidates.iter().map(event::entity_trigger),
            |_| "Entity audio trigger".to_owned(),
        )?;
        let response = |count, mode| {
            let listed = rows.get(..count).ok_or_else(position_error)?;
            let triggers = candidates.get(..count).ok_or_else(position_error)?;
            let occurrence_rows = page
                .slice(&self.occurrences)
                .get(..count)
                .ok_or_else(position_error)?;
            let next_cursor = page.next_cursor_after(count, total)?;
            let evidence = origins::capture(
                self,
                triggers,
                occurrence_rows,
                listed,
                issues.len(),
                next_cursor.is_some(),
                mode,
            )?;
            Ok(EntitySoundsResult {
                archive: archive.clone(),
                entity: entity.clone(),
                total,
                matched,
                triggers: listed.to_vec(),
                issues: issues.clone(),
                issues_total,
                next_cursor,
                origins: evidence,
            })
        };
        let (kept, witness) = fit_prefix(
            rows.len(),
            |count| response(count, SurveyMode::Witness),
            || "Entity audio trigger".to_owned(),
        )?;
        let complete = response(kept, SurveyMode::Complete)?;
        if structured_len(&complete) <= MAX_RESPONSE_LEN {
            Ok(complete)
        } else {
            Ok(witness)
        }
    }
}

fn position_error() -> ToolError {
    ToolError::Refused("Entity audio projection has inconsistent source positions".to_owned())
}
