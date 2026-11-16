// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioSurveyKind, BreakReason, SoundVariant, WwiseEvent};
use p4kx_extraction::{
    Archive, DataForgeOrigin, FieldOrigin, InputScope, ObservedEntry, SourceInput, SourceLocation,
    SourceUseRole, SourceUses,
};

use crate::audio_shapes::VariantShape;
use crate::error::ToolError;
use crate::forge_origins::{EvidenceError, ForgeOrigins};
use crate::forge_shapes::ForgeFieldOrigins;

use super::super::definition_origins::Definition;
use super::super::surveys::{self, SurveyMode};
use super::Selection;

#[path = "audio_resolve_break_origins.rs"]
mod breaks;
#[path = "audio_resolve_path_origins.rs"]
mod path;
#[path = "audio_resolve_summary_origins.rs"]
mod summary;
#[path = "audio_resolve_variant_origins.rs"]
mod variant;

type Evidence<T> = Result<T, EvidenceError>;

pub(super) fn capture(
    page: &Selection<'_, '_>,
    variants: &[SoundVariant<'_>],
    shapes: &[VariantShape],
    issues: usize,
    cursor: bool,
    mode: SurveyMode,
) -> Result<ForgeFieldOrigins, ToolError> {
    let mut output = ForgeOrigins::new(&DataForgeOrigin::Unknown).map_err(EvidenceError::from)?;
    let uses = output.uses.clone();
    let definition = Definition::open(page.archive, page.index, &uses, page.sounds.trigger())?;
    let selected = definition.name(SourceUseRole::Selector)?;
    let no_bank = page
        .sounds
        .breaks()
        .iter()
        .any(|link| matches!(link.reason(), BreakReason::NoSoundbank))
        .then(|| {
            let survey = page.index.survey(page.archive, AudioSurveyKind::Soundbanks);
            surveys::capture(page.archive, &uses, survey, mode)
        })
        .transpose()?;
    let context = Context {
        archive: page.archive,
        uses: &uses,
        definition,
        selected,
        preferred: language(page, &uses, page.sounds.language_choice().preferred(), true)?,
        fallback: language(page, &uses, page.sounds.language_choice().fallback(), false)?,
        no_bank,
        has_preference: page.sounds.language_choice().preferred().is_some(),
    };
    context.header(&mut output, page)?;
    let totals = summary::write(&context, &mut output, page.sounds, cursor, mode)?;
    variant::write(&context, &mut output, variants, shapes, &totals.variants)?;
    let break_basis = (page.page.offset() == 0).then_some(&totals.breaks);
    breaks::write(&context, &mut output, page.listed_breaks, break_basis)?;
    surveys::issues(page.archive, page.index, &mut output, issues, mode)?;
    Ok(output.finish()?)
}

pub(super) struct Context<'a> {
    archive: &'a Archive,
    uses: &'a SourceUses,
    definition: Definition<'a>,
    selected: FieldOrigin,
    preferred: FieldOrigin,
    fallback: FieldOrigin,
    no_bank: Option<FieldOrigin>,
    has_preference: bool,
}

impl Context<'_> {
    fn header(&self, output: &mut ForgeOrigins, page: &Selection<'_, '_>) -> Evidence<()> {
        output.put("archive", self.uses.origin(&[], true))?;
        self.members(
            output,
            "language",
            [
                ("requested", self.preferred.clone()),
                ("fallback", self.fallback.clone()),
            ],
        )?;
        let events_total = self.union([&self.selected, &self.definition.events()?], true)?;
        let mut events = vec![events_total.clone()];
        for (index, event) in page.sounds.trigger().events().iter().enumerate() {
            events.push(self.trigger_event(output, &format!("trigger/events/{index}"), event)?);
        }
        let events = self.union(&events, true)?;
        let path = self.union([&self.selected, &self.definition.path()?], true)?;
        self.members(
            output,
            "trigger",
            [
                ("name", self.definition.name(SourceUseRole::Payload)?),
                ("definition", path),
                ("events_total", events_total),
                ("events", events),
            ],
        )?;
        Ok(())
    }

    fn trigger_event(
        &self,
        output: &mut ForgeOrigins,
        address: &str,
        event: &WwiseEvent,
    ) -> Evidence<FieldOrigin> {
        let name = self.definition.event_name(event, SourceUseRole::Payload)?;
        let localised = self.definition.event_localised(event)?;
        self.members(
            output,
            address,
            [
                ("name", self.union([&self.selected, &name], true)?),
                ("id", self.event_selector(event)?),
                ("localised", self.union([&self.selected, &localised], true)?),
            ],
        )
    }

    fn event_selector(&self, event: &WwiseEvent) -> Evidence<FieldOrigin> {
        let converted = self
            .definition
            .event_name(event, SourceUseRole::Conversion)?;
        self.union([&self.selected, &converted], true)
    }

    fn event_name(&self, event: &WwiseEvent) -> Evidence<FieldOrigin> {
        let name = self.definition.event_name(event, SourceUseRole::Payload)?;
        self.union([&self.selected, &name], true)
    }

    fn union<'field>(
        &self,
        fields: impl IntoIterator<Item = &'field FieldOrigin>,
        complete: bool,
    ) -> Evidence<FieldOrigin> {
        Ok(self.uses.origin_with_origins(fields, complete)?)
    }

    fn members<const N: usize>(
        &self,
        output: &mut ForgeOrigins,
        address: &str,
        fields: [(&str, FieldOrigin); N],
    ) -> Evidence<FieldOrigin> {
        let origin = self.union(fields.iter().map(|(_, field)| field), true)?;
        for (member, field) in fields {
            output.put(&format!("{address}/{member}"), field)?;
        }
        output.put(address, origin.clone())?;
        Ok(origin)
    }

    fn bank(
        &self,
        output: &mut ForgeOrigins,
        address: &str,
        entry: Option<ObservedEntry<'_>>,
        selection: &FieldOrigin,
    ) -> Evidence<FieldOrigin> {
        let path = directory(self.uses, entry, SourceUseRole::Payload)?;
        let language = directory(self.uses, entry, SourceUseRole::Conversion)?;
        let path = self.union([selection, &path], true)?;
        let language = self.union([selection, &language], true)?;
        let bank = self.union([&path, &language], true)?;
        output.put(&format!("{address}/path"), path)?;
        output.put(&format!("{address}/language"), language)?;
        Ok(bank)
    }
}

fn language(
    page: &Selection<'_, '_>,
    uses: &SourceUses,
    name: Option<&str>,
    requested: bool,
) -> Evidence<FieldOrigin> {
    match name {
        Some(name) => directory(
            uses,
            page.index.language_source(page.archive, name),
            SourceUseRole::Conversion,
        ),
        None if requested => Ok(uses.origin(&[], true)),
        None => {
            let paths = uses.observe_root_directory_paths(page.archive)?;
            Ok(uses.origin(paths.as_slice(), paths.is_some()))
        }
    }
}

fn directory(
    uses: &SourceUses,
    entry: Option<ObservedEntry<'_>>,
    role: SourceUseRole,
) -> Evidence<FieldOrigin> {
    let Some(entry) = entry else {
        return Ok(uses.origin(&[], false));
    };
    let scope = uses.observe(
        SourceInput::ArchiveView(entry),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let site = scope.at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        role,
    )?;
    Ok(uses.origin(&[site], entry.ancestors().all(|step| step.is_some())))
}
