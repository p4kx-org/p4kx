// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::{
    AtlTrigger, AtlTriggerOrigins, AudioIndex, AudioSurveyKind, AudioSurveyProof, LanguageChoice,
    WwiseEvent,
};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, ObservedEntry, SourceCoordinateOmission, SourceInput,
    SourceLocation, SourceUseRole, SourceUses,
};

pub(super) struct Definition<'a> {
    source: AtlTriggerOrigins<'a>,
    pub(super) selection: FieldOrigin,
    pub(super) name: FieldOrigin,
    pub(super) path: FieldOrigin,
    pub(super) topology: FieldOrigin,
}

pub(super) struct Event {
    pub(super) name: FieldOrigin,
    pub(super) id: FieldOrigin,
    pub(super) localised: FieldOrigin,
}

impl<'a> Definition<'a> {
    pub(super) fn open(
        archive: &Archive,
        index: &AudioIndex,
        uses: &'a SourceUses,
        trigger: &'a AtlTrigger,
    ) -> Result<Self> {
        let source = AtlTriggerOrigins::open(archive, index, uses, trigger)?;
        let selection = source.name(SourceUseRole::Selector)?;
        let name = source.name(SourceUseRole::Payload)?;
        let path = uses.origin_with_origins([&selection, &source.path()?], true)?;
        let topology = source.events()?;
        Ok(Self {
            source,
            selection,
            name,
            path,
            topology,
        })
    }

    pub(super) fn event(&self, uses: &SourceUses, event: &WwiseEvent) -> Result<Event> {
        let add_selection =
            |value: FieldOrigin| uses.origin_with_origins([&self.selection, &value], true);
        Ok(Event {
            name: add_selection(self.source.event_name(event, SourceUseRole::Payload)?)?,
            id: add_selection(self.source.event_name(event, SourceUseRole::Conversion)?)?,
            localised: add_selection(self.source.event_localised(event)?)?,
        })
    }
}

pub(super) struct LanguageOrigins {
    pub(super) requested: FieldOrigin,
    pub(super) fallback: FieldOrigin,
    pub(super) has_preference: bool,
}

impl LanguageOrigins {
    pub(super) fn of(
        archive: &Archive,
        index: &AudioIndex,
        uses: &SourceUses,
        choice: &LanguageChoice,
    ) -> Result<Self> {
        let canonical = |name| {
            directory(
                uses,
                index.language_source(archive, name),
                SourceUseRole::Conversion,
            )
        };
        let requested = choice
            .preferred()
            .map_or_else(|| Ok(uses.origin(&[], true)), canonical)?;
        let fallback = choice.fallback().map_or_else(
            || {
                let paths = uses.observe_root_directory_paths(archive)?;
                Ok(uses.origin(paths.as_slice(), paths.is_some()))
            },
            canonical,
        )?;
        Ok(Self {
            requested,
            fallback,
            has_preference: choice.preferred().is_some(),
        })
    }
}

pub(super) fn bank_survey(
    archive: &Archive,
    index: &AudioIndex,
    uses: &SourceUses,
) -> Result<FieldOrigin> {
    match index.survey(archive, AudioSurveyKind::Soundbanks) {
        Some(survey) => Ok(survey.origins(uses, AudioSurveyProof::Complete)?),
        None => Ok(uses.origin(&[], false)),
    }
}

pub(super) fn directory(
    uses: &SourceUses,
    entry: Option<ObservedEntry<'_>>,
    role: SourceUseRole,
) -> Result<FieldOrigin> {
    let Some(entry) = entry else {
        return Ok(uses.origin(&[], false));
    };
    let scope = uses.observe(
        SourceInput::ArchiveView(entry),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let path = scope.at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        role,
    )?;
    Ok(uses.origin(
        &[path],
        entry.ancestors().all(|ancestor| ancestor.is_some()),
    ))
}

pub(super) fn issue(
    uses: &SourceUses,
    entry: Option<ObservedEntry<'_>>,
) -> Result<(FieldOrigin, FieldOrigin)> {
    let path = directory(uses, entry, SourceUseRole::Payload)?;
    let diagnostic = if let Some(entry) = entry {
        let scope = uses.observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let attempted = scope.at_opaque(
            SourceUseRole::Attempted,
            SourceCoordinateOmission::Unavailable,
        )?;
        uses.origin(&[attempted], false)
    } else {
        uses.origin(&[], false)
    };
    Ok((path, diagnostic))
}
