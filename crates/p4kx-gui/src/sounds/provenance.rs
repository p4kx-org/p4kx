// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    AtlTriggerOrigins, AudioIndex, AudioSurveyKind, AudioSurveyProof, BreakReason, HircPathSource,
    TriggerSounds, WwiseEvent,
};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, ObservedEntry, SourceEvidenceError, SourceInput,
    SourceInventory, SourceLocation, SourceUseRole, SourceUses,
};

use super::view::SoundList;

#[path = "provenance_rows.rs"]
mod rows;

pub use self::rows::{SoundBreakProvenance, SoundVariantProvenance};

#[derive(Debug)]
pub(super) struct ResolvedSoundList {
    pub(super) list: SoundList,
    pub(super) provenance: Result<SoundListProvenance, SourceEvidenceError>,
}

/// Original source receipts for one completed Sounds inspection.
#[derive(Debug)]
#[non_exhaustive]
pub struct SoundListProvenance {
    /// The authored trigger name.
    pub trigger: FieldOrigin,
    /// The visited routes counted in the variant heading.
    pub variant_count: FieldOrigin,
    /// The failures counted in the broken-link heading.
    pub break_count: FieldOrigin,
    variants: Vec<SoundVariantProvenance>,
    breaks: Vec<SoundBreakProvenance>,
    inventory: SourceInventory,
}

impl SoundListProvenance {
    /// Returns the variant receipts in the displayed order.
    #[must_use]
    pub fn variants(&self) -> &[SoundVariantProvenance] {
        &self.variants
    }

    /// Returns the broken-link receipts in the displayed order.
    #[must_use]
    pub fn breaks(&self) -> &[SoundBreakProvenance] {
        &self.breaks
    }

    /// Returns the physical sources and source-use coordinates shared by the receipts.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    pub(super) fn capture(
        archive: &Archive,
        index: &AudioIndex,
        sounds: &TriggerSounds<'_>,
    ) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::with_compact_use_refs();
        let context = Context::open(archive, index, sounds, &uses)?;
        let mut routes = Vec::with_capacity(sounds.variants().len());
        let variants = sounds
            .variants()
            .iter()
            .map(|variant| {
                let (row, route) = rows::variant(&context, variant)?;
                routes.push(route);
                Ok(row)
            })
            .collect::<Result<Vec<_>, SourceEvidenceError>>()?;
        let breaks = sounds
            .breaks()
            .iter()
            .map(|link| rows::broken_link(&context, link))
            .collect::<Result<Vec<_>, _>>()?;
        let topology = context.definition.events()?;
        let sources = [&context.selected, &topology];
        let break_count = context.union(
            sources
                .into_iter()
                .chain(breaks.iter().map(|link| &link.reason)),
            true,
        )?;
        let variant_count = context.union(
            sources.into_iter().chain(&routes).chain([&break_count]),
            !sounds.truncated_by_limits(),
        )?;
        Ok(Self {
            trigger: context.definition.name(SourceUseRole::Payload)?,
            variant_count,
            break_count,
            variants,
            breaks,
            inventory: uses.freeze()?,
        })
    }
}

struct Context<'a> {
    archive: &'a Archive,
    uses: &'a SourceUses,
    definition: AtlTriggerOrigins<'a>,
    selected: FieldOrigin,
    preferred: FieldOrigin,
    fallback: FieldOrigin,
    no_bank: Option<FieldOrigin>,
    has_preference: bool,
}

impl<'a> Context<'a> {
    fn open(
        archive: &'a Archive,
        index: &AudioIndex,
        sounds: &TriggerSounds<'a>,
        uses: &'a SourceUses,
    ) -> Result<Self, SourceEvidenceError> {
        let definition = AtlTriggerOrigins::open(archive, index, uses, sounds.trigger())?;
        let selected = definition.name(SourceUseRole::Selector)?;
        let no_bank = if sounds
            .breaks()
            .iter()
            .any(|link| matches!(link.reason(), BreakReason::NoSoundbank))
        {
            index
                .survey(archive, AudioSurveyKind::Soundbanks)
                .map(|survey| survey.origins(uses, AudioSurveyProof::Complete))
                .transpose()?
        } else {
            None
        };
        let choice = sounds.language_choice();
        Ok(Self {
            archive,
            uses,
            definition,
            selected,
            preferred: language(archive, index, uses, choice.preferred(), true)?,
            fallback: language(archive, index, uses, choice.fallback(), false)?,
            no_bank,
            has_preference: choice.preferred().is_some(),
        })
    }

    fn union<'field>(
        &self,
        fields: impl IntoIterator<Item = &'field FieldOrigin>,
        complete: bool,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        self.uses.origin_with_origins(fields, complete)
    }

    fn route(
        &self,
        event: &WwiseEvent,
        path_len: usize,
        path_sources: &[HircPathSource],
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut fields = vec![self.event_selector(event)?];
        for source in path_sources {
            let origin = source.origins(self.archive, self.uses)?;
            fields.extend([origin.id, origin.kind, origin.inbound]);
        }
        self.union(&fields, path_len == path_sources.len())
    }

    fn event_selector(&self, event: &WwiseEvent) -> Result<FieldOrigin, SourceEvidenceError> {
        let id = self
            .definition
            .event_name(event, SourceUseRole::Conversion)?;
        self.union([&self.selected, &id], true)
    }

    fn event(&self, event: &WwiseEvent) -> Result<FieldOrigin, SourceEvidenceError> {
        let name = self.definition.event_name(event, SourceUseRole::Payload)?;
        self.union([&self.selected, &name], true)
    }

    fn bank(
        &self,
        entry: Option<ObservedEntry<'_>>,
        route: &FieldOrigin,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let directory = directory(self.uses, entry, role)?;
        self.union([route, &directory], true)
    }
}

fn language(
    archive: &Archive,
    index: &AudioIndex,
    uses: &SourceUses,
    name: Option<&str>,
    requested: bool,
) -> Result<FieldOrigin, SourceEvidenceError> {
    match name {
        Some(name) => directory(
            uses,
            index.language_source(archive, name),
            SourceUseRole::Conversion,
        ),
        None if requested => Ok(uses.origin(&[], true)),
        None => {
            let paths = uses.observe_root_directory_paths(archive)?;
            Ok(uses.origin(paths.as_slice(), paths.is_some()))
        }
    }
}

fn directory(
    uses: &SourceUses,
    entry: Option<ObservedEntry<'_>>,
    role: SourceUseRole,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let Some(entry) = entry else {
        return Ok(uses.origin(&[], false));
    };
    let scope = uses.observe(
        SourceInput::ArchiveView(entry),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let site = scope.at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        role,
    )?;
    Ok(uses.origin(&[site], entry.ancestors().all(|step| step.is_some())))
}
