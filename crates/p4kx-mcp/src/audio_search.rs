// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::audio::AudioIndex;

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::{DEFAULT_ENTITIES, MAX_ENTITIES, SearchTriggersParams};
use crate::audio_shapes::{TriggerHitShape, TriggerSearchResult};
use crate::budget::{Budget, fill_within, json_len};
use crate::budget_text;
use crate::error::ToolError;
use crate::page::Page;

use super::event;
use super::hit_plan::HitPlan;
use super::users::Users;

struct Request {
    search: Option<String>,
    page: Page,
    total: usize,
    hits: Vec<HitPlan>,
}

/// Returns one page of actual selected audio definitions.
pub(crate) fn search(
    opened: &Arc<Opened>,
    params: &SearchTriggersParams,
    page: Page,
) -> Result<TriggerSearchResult, ToolError> {
    let index = opened.audio_index();
    let request = {
        let found = index.search(
            params.search.as_deref().unwrap_or_default(),
            page.offset(),
            page.limit(),
        );
        Request {
            search: params.search.clone(),
            page,
            total: found.total(),
            hits: found.hits().iter().map(HitPlan::read).collect(),
        }
    };
    let wanted = params
        .entities
        .unwrap_or(DEFAULT_ENTITIES)
        .min(MAX_ENTITIES);
    if wanted == 0 || request.hits.is_empty() {
        return render(opened, &index, &request, None, None);
    }
    let request = Arc::new(request);
    let worker_request = Arc::clone(&request);
    let worker_index = Arc::clone(&index);
    let held = Arc::clone(opened);
    let completed = opened.with_dataforge(move |forge, _| {
        let users = Users::new(
            forge,
            &worker_request.hits,
            usize::try_from(wanted).unwrap_or(usize::MAX),
        );
        Ok(render(
            &held,
            &worker_index,
            &worker_request,
            Some(&users),
            None,
        ))
    });
    match completed {
        Ok(rendered) => rendered,
        Err(error) => render(opened, &index, &request, None, Some(error.message())),
    }
}

fn render(
    opened: &Opened,
    index: &AudioIndex,
    request: &Request,
    users: Option<&Users<'_>>,
    skipped: Option<String>,
) -> Result<TriggerSearchResult, ToolError> {
    let mut text = Budget::new();
    if let Some(reason) = &skipped {
        text.admit_text(reason.len())?;
    }
    let (issues, issues_total) = convert::issues(index, &mut text)?;
    let mut result = TriggerSearchResult {
        archive: budget_text::display(&mut text, &opened.archive().path().display())?,
        search: request
            .search
            .as_deref()
            .map(|value| text.text(value))
            .transpose()?,
        total: request.total,
        trigger_count: index.triggers().len(),
        triggers: Vec::new(),
        entities_skipped: skipped,
        unreadable_records: users.map_or(0, Users::unreadable_records),
        issues,
        issues_total,
        next_cursor: None,
    };
    let items = request.hits.iter().map(|plan| hit(index, plan, users));
    let (triggers, next) = fill_within(
        request.page,
        request.total,
        json_len(&result),
        items,
        |_| "audio trigger".to_owned(),
    )?;
    result.triggers = triggers;
    result.next_cursor = next;
    Ok(result)
}

fn hit(
    index: &AudioIndex,
    plan: &HitPlan,
    users: Option<&Users<'_>>,
) -> Result<TriggerHitShape, ToolError> {
    let mut text = Budget::new();
    let name = text.text(&plan.name)?;
    let source = index
        .trigger(&plan.name)
        .filter(|source| source.name() == plan.name)
        .ok_or_else(|| {
            ToolError::Refused("selected audio trigger does not belong to the index".to_owned())
        })?;
    let (events, events_total) = event::hit_events(index, source, &plan.matched_events, &mut text)?;
    Ok(TriggerHitShape {
        name,
        definition: index
            .definition_path(source)
            .map(|value| text.text(value))
            .transpose()?,
        matched_trigger: plan.matched_trigger,
        events_total,
        events,
        unmatched: plan
            .unmatched
            .map(|value| convert::unmatched(value, &mut text))
            .transpose()?,
        entities: users
            .map(|users| users.shape(&plan.name, &mut text))
            .transpose()?,
    })
}
