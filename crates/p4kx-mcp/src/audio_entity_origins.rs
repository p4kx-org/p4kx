// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    AtlTrigger, AudioSurvey, AudioSurveyKind, EntityTrigger, EventMatch, TriggerOccurrence,
    Unmatched,
};
use p4kx_extraction::{FieldOrigin, SourceUseRole};

use crate::audio_shapes::EntityTriggerShape;
use crate::error::ToolError;
use crate::forge_origins::{EvidenceError, ForgeOrigins};
use crate::forge_shapes::ForgeFieldOrigins;

use super::super::event_origins::AudioEventOrigins;
use super::super::occurrence_origins as occurrence;
use super::super::surveys::{self, SurveyMode};
use super::Listing;
use super::position_error;
use super::record_origins as record;
use p4kx_assembly::audio::AtlTriggerOrigins as Definition;

pub(super) fn capture(
    listing: &Listing<'_, '_, '_>,
    triggers: &[EntityTrigger<'_>],
    occurrences: &[Vec<TriggerOccurrence>],
    rows: &[EntityTriggerShape],
    issue_rows: usize,
    has_cursor: bool,
    mode: SurveyMode,
) -> Result<ForgeFieldOrigins, ToolError> {
    let mut output = ForgeOrigins::new(listing.input).map_err(EvidenceError::from)?;
    let atlas = survey_for_missing_atl(listing, &output, mode)?;
    let bank_survey = listing
        .index
        .survey(listing.archive, AudioSurveyKind::Soundbanks);
    let bank_complete = bank_survey.as_ref().is_some_and(AudioSurvey::complete);
    let bank = if needs_bank_absence(listing, rows) {
        Some(surveys::capture(&output.uses, bank_survey, mode)?)
    } else {
        None
    };
    let events = AudioEventOrigins::new(
        listing.archive,
        listing.index,
        output.uses.clone(),
        bank_complete,
        bank.as_ref(),
    );
    let context = TriggerOrigins {
        listing,
        events,
        atlas: atlas.as_ref(),
    };
    let total = record::header(listing, &mut output)?;
    context.matched(&mut output, &total)?;
    context.triggers(&mut output, triggers, occurrences, rows)?;
    surveys::issues(
        listing.archive,
        listing.index,
        &mut output,
        issue_rows,
        mode,
    )?;
    if has_cursor {
        output.put("next_cursor", total)?;
    }
    Ok(output.finish()?)
}

fn survey_for_missing_atl(
    listing: &Listing<'_, '_, '_>,
    output: &ForgeOrigins,
    mode: SurveyMode,
) -> Result<Option<FieldOrigin>, EvidenceError> {
    if listing
        .audio
        .triggers()
        .iter()
        .any(|trigger| trigger.definition().is_none())
    {
        let survey = listing
            .index
            .survey(listing.archive, AudioSurveyKind::AtlDefinitions);
        Ok(Some(surveys::capture(&output.uses, survey, mode)?))
    } else {
        Ok(None)
    }
}

fn needs_bank_absence(listing: &Listing<'_, '_, '_>, rows: &[EntityTriggerShape]) -> bool {
    listing
        .audio
        .triggers()
        .iter()
        .any(|trigger| trigger.unmatched() == Some(Unmatched::NoSoundbank))
        || rows
            .iter()
            .flat_map(|trigger| &trigger.events)
            .any(|event| event.soundbank_count == 0)
}

struct TriggerOrigins<'listing, 'archive, 'record, 'index, 'survey> {
    listing: &'listing Listing<'archive, 'record, 'index>,
    events: AudioEventOrigins<'survey>,
    atlas: Option<&'survey FieldOrigin>,
}

impl TriggerOrigins<'_, '_, '_, '_, '_> {
    fn matched(&self, output: &mut ForgeOrigins, total: &FieldOrigin) -> Result<(), ToolError> {
        let statuses = self
            .listing
            .audio
            .triggers()
            .iter()
            .map(|trigger| self.status(output, trigger))
            .collect::<Result<Vec<_>, _>>()?;
        let count = output
            .uses
            .origin_with_origins(std::iter::once(total).chain(&statuses), true)
            .map_err(EvidenceError::from)?;
        output.put("matched", count)?;
        Ok(())
    }

    fn triggers(
        &self,
        output: &mut ForgeOrigins,
        triggers: &[EntityTrigger<'_>],
        occurrences: &[Vec<TriggerOccurrence>],
        rows: &[EntityTriggerShape],
    ) -> Result<(), ToolError> {
        if triggers.len() != occurrences.len() || triggers.len() != rows.len() {
            return Err(position_error());
        }
        let mut origins = Vec::new();
        for (position, ((trigger, fields), row)) in
            triggers.iter().zip(occurrences).zip(rows).enumerate()
        {
            let path = format!("triggers/{position}");
            origins.push(self.trigger(output, &path, trigger, fields, row)?);
        }
        let origin = output
            .uses
            .origin_with_origins(&origins, true)
            .map_err(EvidenceError::from)?;
        output.put("triggers", origin)?;
        Ok(())
    }

    fn trigger(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        trigger: &EntityTrigger<'_>,
        occurrences: &[TriggerOccurrence],
        row: &EntityTriggerShape,
    ) -> Result<FieldOrigin, ToolError> {
        let first = occurrences.first().ok_or_else(position_error)?;
        let name =
            occurrence::collect(output, std::slice::from_ref(first), SourceUseRole::Payload)?;
        let location =
            occurrence::collect(output, std::slice::from_ref(first), SourceUseRole::Selector)?;
        let references = occurrence::collect(output, occurrences, SourceUseRole::Conversion)?;
        let [definition, events_total, lookup] =
            self.definition(output, trigger.definition(), &location)?;
        let status = self.status(output, trigger)?;
        let status = output
            .uses
            .origin_with_origins([&status, &references], true)
            .map_err(EvidenceError::from)?;
        let events = self.write_events(output, path, trigger, row, &events_total, &lookup)?;
        for (member, origin) in [
            ("name", &name),
            ("location", &location),
            ("references", &references),
            ("definition", &definition),
            ("events_total", &events_total),
            ("events", &events),
            ("unmatched", &status),
        ] {
            output.put(&format!("{path}/{member}"), origin.clone())?;
        }
        if row.unmatched.is_some() {
            output.put(&format!("{path}/unmatched/kind"), status.clone())?;
            output.put(&format!("{path}/unmatched/message"), status.clone())?;
        }
        let origin = output
            .uses
            .origin_with_origins(
                [
                    &name,
                    &location,
                    &references,
                    &definition,
                    &events_total,
                    &events,
                    &status,
                ],
                true,
            )
            .map_err(EvidenceError::from)?;
        output.put(path, origin.clone())?;
        Ok(origin)
    }

    fn write_events(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        trigger: &EntityTrigger<'_>,
        row: &EntityTriggerShape,
        events_total: &FieldOrigin,
        lookup: &FieldOrigin,
    ) -> Result<FieldOrigin, ToolError> {
        let mut event_origins = Vec::new();
        if let Some(definition) = trigger.definition() {
            if definition.events().len() != row.events.len() {
                return Err(position_error());
            }
            for (ordinal, (event, row)) in trigger.events().zip(&row.events).enumerate() {
                let address = format!("{path}/events/{ordinal}");
                event_origins.push(
                    self.events
                        .write(output, &address, definition, event, row, lookup)?,
                );
            }
        }
        Ok(output
            .uses
            .origin_with_origins(std::iter::once(events_total).chain(&event_origins), true)
            .map_err(EvidenceError::from)?)
    }

    fn definition(
        &self,
        output: &ForgeOrigins,
        trigger: Option<&AtlTrigger>,
        caller: &FieldOrigin,
    ) -> Result<[FieldOrigin; 3], EvidenceError> {
        let Some(trigger) = trigger else {
            let absent = self
                .atlas
                .cloned()
                .unwrap_or_else(|| output.uses.origin(&[], false));
            let absent = output.uses.origin_with_origins([caller, &absent], true)?;
            return Ok([absent.clone(), absent.clone(), absent]);
        };
        let source = Definition::open(
            self.listing.archive,
            self.listing.index,
            &output.uses,
            trigger,
        )?;
        let selected = source.name(SourceUseRole::Selector)?;
        let lookup = output.uses.origin_with_origins([caller, &selected], true)?;
        let definition = output
            .uses
            .origin_with_origins([&lookup, &source.path()?], true)?;
        let list = source.events()?;
        let events = output.uses.origin_with_origins([&lookup, &list], true)?;
        Ok([definition, events, lookup])
    }

    fn status(
        &self,
        output: &ForgeOrigins,
        trigger: &EntityTrigger<'_>,
    ) -> Result<FieldOrigin, ToolError> {
        let Some(definition) = trigger.definition() else {
            return Ok(self
                .atlas
                .cloned()
                .unwrap_or_else(|| output.uses.origin(&[], false)));
        };

        if let Some(event) = trigger.events().find(EventMatch::is_matched) {
            return self.events.matched_origin(definition, event);
        }
        let complete_definition = Definition::open(
            self.listing.archive,
            self.listing.index,
            &output.uses,
            definition,
        )?
        .events()?;
        let checked = trigger
            .events()
            .map(|event| self.events.matched_origin(definition, event))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(output
            .uses
            .origin_with_origins(std::iter::once(&complete_definition).chain(&checked), true)
            .map_err(EvidenceError::from)?)
    }
}
