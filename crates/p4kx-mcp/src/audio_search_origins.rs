// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, AudioSurvey, AudioSurveyKind, EventMatch, TriggerHit};
use p4kx_extraction::{Archive, DataForgeOrigin, FieldOrigin, SourceUseRole};

use crate::audio_shapes::TriggerHitShape;
use crate::error::ToolError;
use crate::forge_origins::{EvidenceError, ForgeOrigins};
use crate::forge_shapes::ForgeFieldOrigins;

use super::super::definition_origins::Definition;
use super::super::event_origins::AudioEventOrigins;
use super::super::surveys::{self, SurveyMode};
use super::UsersState;

#[path = "audio_search_user_origins.rs"]
mod user_origins;

pub(super) struct Page<'a, 'index, 'users, 'forge> {
    pub(super) archive: &'a Archive,
    pub(super) index: &'a AudioIndex,
    pub(super) hits: &'a [TriggerHit<'index>],
    pub(super) rows: &'a [TriggerHitShape],
    pub(super) filtered: bool,
    pub(super) issues: usize,
    pub(super) users: Option<&'a UsersState<'users, 'forge>>,
    pub(super) skipped: bool,
    pub(super) cursor: bool,
    pub(super) mode: SurveyMode,
}

pub(super) fn capture(page: &Page<'_, '_, '_, '_>) -> Result<ForgeFieldOrigins, ToolError> {
    let input = page
        .users
        .map_or(&DataForgeOrigin::Unknown, |state| state.input);
    let mut output = ForgeOrigins::new(input).map_err(EvidenceError::from)?;
    navigation(page, &mut output)?;
    let users = user_origins::census(&mut output, page.users, page.skipped, page.mode)?;
    let bank_survey = page.index.survey(page.archive, AudioSurveyKind::Soundbanks);
    let bank_complete = bank_survey.as_ref().is_some_and(AudioSurvey::complete);
    let bank = page
        .rows
        .iter()
        .flat_map(|r| &r.events)
        .any(|event| event.soundbank_count == 0)
        .then(|| surveys::capture(page.archive, &output.uses, bank_survey, page.mode))
        .transpose()?;
    let events = AudioEventOrigins::new(
        page.archive,
        page.index,
        output.uses.clone(),
        bank_complete,
        bank.as_ref(),
    );
    let context = Projection {
        page,
        events,
        users,
    };
    let mut triggers = Vec::new();
    for (position, (hit, row)) in page.hits.iter().zip(page.rows).enumerate() {
        triggers.push(context.trigger(&mut output, &format!("triggers/{position}"), hit, row)?);
    }
    let triggers = union(&output, &triggers, page.hits.len() == page.rows.len())?;
    output.put("triggers", triggers)?;
    surveys::issues(
        page.archive,
        page.index,
        &mut output,
        page.issues,
        page.mode,
    )?;
    Ok(output.finish()?)
}

fn navigation(page: &Page<'_, '_, '_, '_>, output: &mut ForgeOrigins) -> Result<(), EvidenceError> {
    for local in ["archive", "search"] {
        output.put(local, output.uses.origin(&[], true))?;
    }
    let catalog = page
        .index
        .survey(page.archive, AudioSurveyKind::AtlDefinitions);
    let census = surveys::capture(page.archive, &output.uses, catalog, page.mode)?;
    output.put("trigger_count", census.clone())?;
    output.put("total", census.clone())?;
    if page.cursor {
        output.put("next_cursor", census)?;
    }
    Ok(())
}

struct Projection<'page, 'archive, 'index, 'users, 'forge, 'banks> {
    page: &'page Page<'archive, 'index, 'users, 'forge>,
    events: AudioEventOrigins<'banks>,
    users: FieldOrigin,
}

impl Projection<'_, '_, '_, '_, '_, '_> {
    fn trigger(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        hit: &TriggerHit<'_>,
        row: &TriggerHitShape,
    ) -> Result<FieldOrigin, ToolError> {
        let uses = output.uses.clone();
        let definition =
            Definition::open(self.page.archive, self.page.index, &uses, hit.trigger())?;
        let lookup = self.lookup(output, hit, &definition)?;
        let posted = union(output, [&lookup, &definition.events()?], true)?;
        let events = self.events(output, path, hit, row, &lookup, &posted)?;
        let status = self.status(output, hit, &posted)?;
        if row.unmatched.is_some() {
            output.put(&format!("{path}/unmatched/kind"), status.clone())?;
            output.put(&format!("{path}/unmatched/message"), status.clone())?;
        }
        let entities = user_origins::trigger(
            output,
            &user_origins::Trigger {
                path,
                user_state: self.page.users,
                shape: row.entities.as_ref(),
                lookup: &lookup,
                census: &self.users,
                name: hit.trigger().name(),
                mode: self.page.mode,
            },
        )?;
        let fields = [
            (
                "name",
                union(
                    output,
                    [&lookup, &definition.name(SourceUseRole::Payload)?],
                    true,
                )?,
            ),
            (
                "definition",
                union(output, [&lookup, &definition.path()?], true)?,
            ),
            (
                "matched_trigger",
                union(
                    output,
                    [&lookup, &definition.name(SourceUseRole::Conversion)?],
                    true,
                )?,
            ),
            ("events_total", posted),
            ("events", events),
            ("unmatched", status),
            ("entities", entities),
        ];
        let trigger = union(output, fields.iter().map(|(_, origin)| origin), true)?;
        for (field, origin) in fields {
            output.put(&format!("{path}/{field}"), origin)?;
        }
        output.put(path, trigger.clone())?;
        Ok(trigger)
    }

    fn lookup(
        &self,
        output: &ForgeOrigins,
        hit: &TriggerHit<'_>,
        definition: &Definition<'_>,
    ) -> Result<FieldOrigin, EvidenceError> {
        let name = definition.name(SourceUseRole::Selector)?;
        if !self.page.filtered || hit.matched_by_trigger() {
            return Ok(name);
        }
        let mut consulted = vec![name];
        for (event, matched) in hit.events() {
            consulted.push(definition.event_name(event.event(), SourceUseRole::Selector)?);
            if matched {
                return union(output, &consulted, true);
            }
        }
        union(output, &consulted, false)
    }

    fn events(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        hit: &TriggerHit<'_>,
        row: &TriggerHitShape,
        lookup: &FieldOrigin,
        posted: &FieldOrigin,
    ) -> Result<FieldOrigin, ToolError> {
        let mut events = Vec::new();
        for (position, ((event, _), row)) in hit.events().zip(&row.events).enumerate() {
            events.push(self.events.write(
                output,
                &format!("{path}/events/{position}"),
                hit.trigger(),
                event,
                row,
                lookup,
            )?);
        }
        let complete =
            events.len() == hit.trigger().events().len() && events.len() == row.events.len();
        Ok(union(
            output,
            std::iter::once(posted).chain(&events),
            complete,
        )?)
    }

    fn status(
        &self,
        output: &ForgeOrigins,
        hit: &TriggerHit<'_>,
        posted: &FieldOrigin,
    ) -> Result<FieldOrigin, ToolError> {
        let event = hit
            .events()
            .map(|(event, _)| event)
            .find(EventMatch::is_matched);
        if let Some(event) = event {
            let witness = self.events.matched_origin(hit.trigger(), event)?;
            return Ok(union(output, [posted, &witness], true)?);
        }
        let checked = hit
            .events()
            .map(|(event, _)| self.events.matched_origin(hit.trigger(), event))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(union(
            output,
            std::iter::once(posted).chain(&checked),
            true,
        )?)
    }
}

fn union<'a>(
    output: &ForgeOrigins,
    fields: impl IntoIterator<Item = &'a FieldOrigin>,
    complete: bool,
) -> Result<FieldOrigin, EvidenceError> {
    Ok(output.uses.origin_with_origins(fields, complete)?)
}
