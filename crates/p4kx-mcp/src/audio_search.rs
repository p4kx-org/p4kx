// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::audio::{AudioIndex, SearchPage, TriggerHit};
use p4kx_extraction::{Archive, DataForgeOrigin};

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::{DEFAULT_ENTITIES, MAX_ENTITIES, SearchTriggersParams};
use crate::audio_shapes::{IssueShape, TriggerHitShape, TriggerSearchResult};
use crate::budget::{Budget, MAX_RESPONSE_LEN, fill_within, fit_prefix, json_len, structured_len};
use crate::budget_text;
use crate::error::ToolError;
use crate::forge_shapes::ForgeFieldOrigins;
use crate::page::Page;

use super::event;
use super::surveys::SurveyMode;
use super::users::Users;

#[path = "audio_search_origins.rs"]
mod origins;

struct Request<'index> {
    search: Option<String>,
    page: Page,
    found: SearchPage<'index>,
}

struct UsersState<'view, 'forge> {
    users: &'view Users<'forge>,
    input: &'view DataForgeOrigin,
}

/// Returns one page of selected audio definitions and their original source evidence.
pub(crate) fn search(
    opened: &Arc<Opened>,
    params: &SearchTriggersParams,
    page: Page,
) -> Result<TriggerSearchResult, ToolError> {
    let index = opened.audio_index();
    let request = Request::new(&index, params.search.clone(), page)?;
    let wanted = params
        .entities
        .unwrap_or(DEFAULT_ENTITIES)
        .min(MAX_ENTITIES);
    if wanted == 0 || request.found.hits().is_empty() {
        return render(opened.archive(), &index, &request, None, None);
    }
    let search = params.search.clone();
    let worker_index = Arc::clone(&index);
    let held = Arc::clone(opened);
    let completed = opened.with_dataforge_origin(move |forge, _, input| {
        let request = Request::new(&worker_index, search, page)?;
        let users = Users::new(
            forge,
            request.found.hits(),
            usize::try_from(wanted).unwrap_or(usize::MAX),
        );
        Ok(render(
            held.archive(),
            &worker_index,
            &request,
            Some(&UsersState {
                users: &users,
                input,
            }),
            None,
        ))
    });
    match completed {
        Ok(rendered) => rendered,
        Err(error) => render(
            opened.archive(),
            &index,
            &request,
            None,
            Some(&error.message()),
        ),
    }
}

impl<'index> Request<'index> {
    fn new(
        index: &'index AudioIndex,
        search: Option<String>,
        page: Page,
    ) -> Result<Self, ToolError> {
        let found = index.search(
            search.as_deref().unwrap_or_default(),
            page.offset(),
            page.limit(),
        );
        page.check(found.total())?;
        Ok(Self {
            search,
            page,
            found,
        })
    }
}

struct Envelope {
    archive: String,
    search: Option<String>,
    total: usize,
    trigger_count: usize,
    skipped: Option<String>,
    unreadable_records: usize,
    issues: Vec<IssueShape>,
    issues_total: usize,
}

impl Envelope {
    fn read(
        archive: &Archive,
        index: &AudioIndex,
        request: &Request<'_>,
        users: Option<&UsersState<'_, '_>>,
        skipped: Option<&str>,
    ) -> Result<Self, ToolError> {
        let mut text = Budget::new();
        if let Some(reason) = skipped {
            text.admit_text(reason.len())?;
        }
        let (issues, issues_total) = convert::issues(index, &mut text)?;
        Ok(Self {
            archive: budget_text::display(&mut text, &archive.path().display())?,
            search: request
                .search
                .as_deref()
                .map(|value| text.text(value))
                .transpose()?,
            total: request.found.total(),
            trigger_count: index.triggers().len(),
            skipped: skipped.map(str::to_owned),
            unreadable_records: users.map_or(0, |state| state.users.unreadable_records()),
            issues,
            issues_total,
        })
    }

    fn size(&self) -> usize {
        json_len(&(
            &self.archive,
            &self.search,
            self.total,
            self.trigger_count,
            &self.skipped,
            self.unreadable_records,
            &self.issues,
            self.issues_total,
        ))
    }

    fn response(
        &self,
        rows: &[TriggerHitShape],
        next_cursor: Option<String>,
        origins: ForgeFieldOrigins,
    ) -> TriggerSearchResult {
        TriggerSearchResult {
            archive: self.archive.clone(),
            search: self.search.clone(),
            total: self.total,
            trigger_count: self.trigger_count,
            triggers: rows.to_vec(),
            entities_skipped: self.skipped.clone(),
            unreadable_records: self.unreadable_records,
            issues: self.issues.clone(),
            issues_total: self.issues_total,
            next_cursor,
            origins,
        }
    }
}

fn render(
    archive: &Archive,
    index: &AudioIndex,
    request: &Request<'_>,
    users: Option<&UsersState<'_, '_>>,
    skipped: Option<&str>,
) -> Result<TriggerSearchResult, ToolError> {
    let envelope = Envelope::read(archive, index, request, users, skipped)?;
    let hits = request.found.hits();
    let items = hits
        .iter()
        .map(|plan| hit(plan, users.map(|state| state.users)));
    let (rows, _) = fill_within(request.page, envelope.total, envelope.size(), items, |_| {
        "audio trigger".to_owned()
    })?;
    let response = |count, mode| {
        let position_error = || {
            ToolError::Refused(
                "audio search projection has inconsistent source positions".to_owned(),
            )
        };
        let hits = hits.get(..count).ok_or_else(position_error)?;
        let rows = rows.get(..count).ok_or_else(position_error)?;
        let next_cursor = request.page.next_cursor_after(count, envelope.total)?;
        let origins = origins::capture(&origins::Page {
            archive,
            index,
            hits,
            rows,
            filtered: envelope
                .search
                .as_ref()
                .is_some_and(|filter| !filter.is_empty()),
            issues: envelope.issues.len(),
            users,
            skipped: skipped.is_some(),
            cursor: next_cursor.is_some(),
            mode,
        })?;
        Ok(envelope.response(rows, next_cursor, origins))
    };
    let (kept, witness) = fit_prefix(
        rows.len(),
        |count| response(count, SurveyMode::Witness),
        || "audio trigger".to_owned(),
    )?;
    let complete = response(kept, SurveyMode::Complete)?;
    if structured_len(&complete) <= MAX_RESPONSE_LEN {
        Ok(complete)
    } else {
        Ok(witness)
    }
}

fn hit(plan: &TriggerHit<'_>, users: Option<&Users<'_>>) -> Result<TriggerHitShape, ToolError> {
    let mut text = Budget::new();
    let source = plan.trigger();
    let (events, events_total) = event::events(
        plan.events().map(|(hit, matched)| (hit, Some(matched))),
        &mut text,
    )?;
    Ok(TriggerHitShape {
        name: text.text(source.name())?,
        definition: plan
            .definition_path()
            .map(|value| text.text(value))
            .transpose()?,
        matched_trigger: plan.matched_by_trigger(),
        events_total,
        events,
        unmatched: plan
            .unmatched()
            .map(|value| convert::unmatched(value, &mut text))
            .transpose()?,
        entities: users
            .map(|users| users.shape(source.name(), &mut text))
            .transpose()?,
    })
}
