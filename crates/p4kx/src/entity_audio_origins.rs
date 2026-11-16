// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_assembly::audio::{
    AudioIndex, AudioSurveyKind, AudioSurveyProof, EntityAudio, EntityTrigger, LanguageChoice,
    PositionedTriggerReference, TriggerOccurrence, TriggerReference, TriggerScope, TriggerSounds,
    Unmatched, positioned_trigger_references_in, trigger_references,
};
use p4kx_extraction::{
    Archive, DataForgeOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope, Record,
    RecordJsonLocation, SourceInput, SourceInventory, SourceScope, SourceUseRole, SourceUses,
    TreeBuilder,
};
use serde::Serialize;

use crate::wwise_json;

#[path = "entity_audio_event_origins.rs"]
mod event_origins;
#[path = "entity_audio_header_origins.rs"]
mod header_origins;

pub(super) struct Inspection {
    occurrences: Option<Vec<Vec<TriggerOccurrence>>>,
    complete: bool,
}

pub(super) fn inspect(entity: &Record<'_>) -> Result<(Vec<TriggerReference>, Inspection)> {
    let tree = TreeBuilder::new().build_positioned_partial(entity)?;
    let Ok(positioned) = positioned_trigger_references_in(&tree, TriggerScope::default()) else {
        return Ok((
            trigger_references(tree.tree()),
            Inspection {
                occurrences: None,
                complete: false,
            },
        ));
    };
    let (references, occurrences) = positioned
        .into_iter()
        .map(PositionedTriggerReference::into_parts)
        .unzip();
    let complete = tree.tree().errors().next().is_none();
    Ok((
        references,
        Inspection {
            occurrences: Some(occurrences),
            complete,
        },
    ))
}

pub(super) struct Input<'a, 'index> {
    pub(super) archive: &'a Archive,
    pub(super) index: &'a AudioIndex,
    pub(super) original: &'a DataForgeOrigin,
    pub(super) record: &'a Record<'a>,
    pub(super) inspection: &'a Inspection,
    pub(super) resolved: &'a EntityAudio<'index>,
    pub(super) sounds: &'a [Option<TriggerSounds<'index>>],
    pub(super) language: Option<&'a LanguageChoice>,
}

#[derive(Serialize)]
pub(super) struct Evidence {
    #[serde(rename = "fieldOrigins")]
    fields: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn capture(input: &Input<'_, '_>) -> Result<Evidence> {
    let mut output = Output::new(input.original)?;
    let record = header_origins::write(&mut output, input)?;
    let surveys = Surveys::capture(input, &output)?;
    let triggers = input.resolved.triggers();
    if let Some(originals) = &input.inspection.occurrences {
        ensure!(
            triggers.len() == originals.len(),
            "audio trigger positions differ from the Entity list"
        );
    }
    let mut rows = Vec::with_capacity(triggers.len());
    for (ordinal, trigger) in triggers.iter().enumerate() {
        let occurrences = input
            .inspection
            .occurrences
            .as_deref()
            .and_then(|list| list.get(ordinal));
        rows.push(output.trigger(
            input,
            &surveys,
            ordinal,
            trigger,
            occurrences.map(Vec::as_slice),
        )?);
    }
    output.counts(&record, &rows)?;
    header_origins::issues(&mut output, input.archive, input.index)?;
    Ok(Evidence {
        fields: output.fields,
        provenance: output.uses.freeze()?,
    })
}

struct Output {
    uses: SourceUses,
    scope: SourceScope,
    dataforge_complete: bool,
    fields: FieldOrigins,
}

struct TriggerEvidence {
    origin: FieldOrigin,
    count: FieldOrigin,
    status: FieldOrigin,
}

impl Output {
    fn new(original: &DataForgeOrigin) -> Result<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let (source, dataforge_complete) = match original {
            DataForgeOrigin::Archive(entry) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|ancestor| ancestor.is_some()),
            ),
            DataForgeOrigin::File(file) => (SourceInput::File(file), true),
            _ => (SourceInput::Unknown, false),
        };
        let scope = uses.observe(source, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            dataforge_complete,
            fields: FieldOrigins::new(),
        })
    }

    fn trigger(
        &mut self,
        input: &Input<'_, '_>,
        surveys: &Surveys,
        ordinal: usize,
        trigger: &EntityTrigger<'_>,
        occurrences: Option<&[TriggerOccurrence]>,
    ) -> Result<TriggerEvidence> {
        let address = format!("/triggers/{ordinal}");
        let occurrences = occurrences.unwrap_or_default();
        let first = occurrences.first().map_or(&[][..], std::slice::from_ref);
        let name = self.occurrence(first, SourceUseRole::Payload)?;
        let location = self.occurrence(first, SourceUseRole::Selector)?;
        let references = self.occurrence(occurrences, SourceUseRole::Conversion)?;
        let selected = event_origins::write(self, input, surveys, trigger, &address, &location)?;
        let mut origin = self.members(
            &address,
            [
                ("name", name),
                ("location", location.clone()),
                ("references", references.clone()),
                ("definition", selected.definition),
                ("events", selected.events),
                ("unmatched", selected.status.clone()),
            ],
        )?;
        if trigger.unmatched().is_some() {
            self.put(
                &format!("{address}/unmatched/kind"),
                selected.status.clone(),
            )?;
            self.put(
                &format!("{address}/unmatched/message"),
                selected.status.clone(),
            )?;
        }
        if let Some(sounds) = input.sounds.get(ordinal).and_then(Option::as_ref) {
            let path = FieldPath::parse(&format!("{address}/sounds"))?;
            let (nested, root) = wwise_json::capture_nested(
                input.archive,
                input.index,
                sounds,
                &self.uses,
                &path,
                &location,
            )?;
            for (pointer, evidence) in nested.iter() {
                self.fields.insert(pointer.clone(), evidence.clone())?;
            }
            origin = self.union([&origin, &root], true)?;
            self.fields.insert(path, root)?;
        }
        self.put(&address, origin.clone())?;
        Ok(TriggerEvidence {
            origin,
            count: references,
            status: selected.status,
        })
    }

    fn occurrence(
        &self,
        occurrences: &[TriggerOccurrence],
        value_role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let mut observed = Vec::new();
        let mut complete = self.dataforge_complete && !occurrences.is_empty();
        for occurrence in occurrences {
            complete &= occurrence.complete();
            for source in occurrence.sources() {
                let Some(location) = RecordJsonLocation::of_use(source) else {
                    complete = false;
                    continue;
                };
                let role = if location.role == SourceUseRole::Payload {
                    value_role
                } else {
                    location.role
                };
                observed.push(self.scope.at(location.location, role)?);
            }
        }
        Ok(self.uses.origin(&observed, complete))
    }

    fn counts(&mut self, record: &FieldOrigin, rows: &[TriggerEvidence]) -> Result<()> {
        let count = self.union(
            std::iter::once(record).chain(rows.iter().map(|row| &row.count)),
            true,
        )?;
        let status = self.union(
            std::iter::once(&count).chain(rows.iter().map(|row| &row.status)),
            true,
        )?;
        let list = self.union(
            std::iter::once(&count).chain(rows.iter().map(|row| &row.origin)),
            true,
        )?;
        let counts = self.members(
            "/counts",
            [
                ("triggers", count),
                ("matched", status.clone()),
                ("unmatched", status),
            ],
        )?;
        self.put("/counts", counts)?;
        self.put("/triggers", list)
    }

    fn union<'a>(
        &self,
        origins: impl IntoIterator<Item = &'a FieldOrigin>,
        complete: bool,
    ) -> Result<FieldOrigin> {
        Ok(self.uses.origin_with_origins(origins, complete)?)
    }

    fn members<const N: usize>(
        &mut self,
        address: &str,
        fields: [(&str, FieldOrigin); N],
    ) -> Result<FieldOrigin> {
        let aggregate = self.union(fields.iter().map(|(_, origin)| origin), true)?;
        for (name, origin) in fields {
            self.put(&format!("{address}/{name}"), origin)?;
        }
        Ok(aggregate)
    }

    fn put(&mut self, path: &str, origin: FieldOrigin) -> Result<()> {
        Ok(self.fields.insert(FieldPath::parse(path)?, origin)?)
    }
}

struct Surveys {
    definitions: Option<FieldOrigin>,
    banks: Option<FieldOrigin>,
}

impl Surveys {
    fn capture(input: &Input<'_, '_>, output: &Output) -> Result<Self> {
        let triggers = input.resolved.triggers();
        let definitions = triggers
            .iter()
            .any(|trigger| trigger.definition().is_none());
        let banks = triggers.iter().any(|trigger| {
            trigger.unmatched() == Some(Unmatched::NoSoundbank)
                || trigger.events().any(|event| !event.is_matched())
        });
        let observe = |kind| {
            let Some(survey) = input.index.survey(input.archive, kind) else {
                return Ok(output.uses.origin(&[], false));
            };
            survey.origins(&output.uses, AudioSurveyProof::Complete)
        };
        Ok(Self {
            definitions: definitions
                .then(|| observe(AudioSurveyKind::AtlDefinitions))
                .transpose()?,
            banks: banks
                .then(|| observe(AudioSurveyKind::Soundbanks))
                .transpose()?,
        })
    }
}
