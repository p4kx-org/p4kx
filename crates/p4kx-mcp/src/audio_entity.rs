// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::Entities;

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::EntitySoundsParams;
use crate::audio_shapes::EntitySoundsResult;
use crate::budget::{Budget, fill_within, json_len};
use crate::budget_text;
use crate::error::ToolError;
use crate::page::Page;

use super::{event, header};

/// Returns one page of an actual Entity's audio references.
pub(crate) fn entity_sounds(
    opened: &Arc<Opened>,
    params: &EntitySoundsParams,
    page: Page,
) -> Result<EntitySoundsResult, ToolError> {
    opened.audio_index();
    let held = Arc::clone(opened);
    let identifier = params.entity.clone();
    opened.with_dataforge(move |forge, _| {
        let record = Entities::new(forge).find(&identifier)?;
        let index = held.audio_index();
        let audio = index.entity_audio(&record)?;
        let total = audio.triggers().len();
        page.check(total)?;
        let mut text = Budget::new();
        let entity = header::full(&record, &mut text)?;
        let (issues, issues_total) = convert::issues(&index, &mut text)?;
        let mut result = EntitySoundsResult {
            archive: budget_text::display(&mut text, &held.archive().path().display())?,
            entity,
            total,
            matched: audio.matched_count(),
            triggers: Vec::new(),
            issues,
            issues_total,
            next_cursor: None,
        };
        let items = page
            .slice(audio.triggers())
            .iter()
            .map(event::entity_trigger);
        let (triggers, next) = fill_within(page, total, json_len(&result), items, |_| {
            "Entity audio trigger".to_owned()
        })?;
        result.triggers = triggers;
        result.next_cursor = next;
        Ok(result)
    })
}
