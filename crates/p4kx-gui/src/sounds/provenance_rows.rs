// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{BreakReason, BrokenLink, SoundVariant, WemSelection};
use p4kx_extraction::{FieldOrigin, SourceEvidenceError, SourceUseRole};

use super::Context;

/// Original contributors to each displayed value of a resolved sound.
#[derive(Debug)]
#[non_exhaustive]
pub struct SoundVariantProvenance {
    /// The authored Wwise event name.
    pub event: FieldOrigin,
    /// The file name derived from the selected bank's path.
    pub soundbank: FieldOrigin,
    /// The selected bank's original path.
    pub soundbank_path: FieldOrigin,
    /// The terminal HIRC media identifier.
    pub wem_id: FieldOrigin,
    /// The terminal source kind.
    pub kind: FieldOrigin,
    /// The embedded DIDX length or loose Entry size.
    pub size: FieldOrigin,
    /// The selected media language and its fallback marker.
    pub language: FieldOrigin,
    /// Whether the preferred language fell back to a different language.
    pub fallback: FieldOrigin,
    /// The media-admission facts behind the playback state.
    pub playable: FieldOrigin,
}

/// Original contributors to each displayed value of a broken link.
#[derive(Debug)]
#[non_exhaustive]
pub struct SoundBreakProvenance {
    /// The authored Wwise event name.
    pub event: FieldOrigin,
    /// The name of the bank associated with the failure.
    pub soundbank: FieldOrigin,
    /// The original route and failed selector or attempted read.
    pub reason: FieldOrigin,
}

pub(super) fn variant(
    context: &Context<'_>,
    variant: &SoundVariant<'_>,
) -> Result<(SoundVariantProvenance, FieldOrigin), SourceEvidenceError> {
    let route = context.route(
        variant.event(),
        variant.path().len(),
        variant.path_sources(),
    )?;
    let entry = variant
        .path_sources()
        .last()
        .and_then(|source| source.entry(context.archive));
    let terminal = variant.terminal_media_origins(context.archive, context.uses)?;
    let wem_id = context.union([&route, &terminal.wem_id], true)?;
    let kind = context.union([&route, &terminal.source_kind], true)?;
    let localised = context.union([&route, &terminal.localised], true)?;
    let location = Location::read(context, variant, &wem_id, &kind, &localised)?;
    let fallback = if context.has_preference && variant.is_localised() {
        context.union([&location.language, &context.preferred], true)?
    } else {
        localised
    };
    let language = if variant.used_fallback() {
        context.union([&location.language, &fallback], true)?
    } else {
        location.language
    };
    let row = SoundVariantProvenance {
        event: context.event(variant.event())?,
        soundbank: context.bank(entry, &route, SourceUseRole::Conversion)?,
        soundbank_path: context.bank(entry, &route, SourceUseRole::Payload)?,
        wem_id,
        kind,
        size: location.size,
        language,
        fallback,
        playable: location.playable,
    };
    Ok((row, route))
}

struct Location {
    size: FieldOrigin,
    language: FieldOrigin,
    playable: FieldOrigin,
}

impl Location {
    fn read(
        context: &Context<'_>,
        variant: &SoundVariant<'_>,
        id: &FieldOrigin,
        kind: &FieldOrigin,
        localised: &FieldOrigin,
    ) -> Result<Self, SourceEvidenceError> {
        let policy = match variant.wem_selection() {
            WemSelection::LoosePreferredLanguage => Some(&context.preferred),
            WemSelection::LooseFallbackLanguage => Some(&context.fallback),
            _ => None,
        };
        let location = variant.wem_location_origins(context.archive, context.uses)?;
        let selected_location = variant.location().is_some().then_some(&location.kind);
        let selection = context.union(
            [id, kind, localised]
                .into_iter()
                .chain(policy)
                .chain(selected_location),
            true,
        )?;
        if variant.location().is_some() {
            let size = context.union([&selection, &location.len], true)?;
            let language = if variant.is_localised() {
                context.union([&selection, &location.language], true)?
            } else {
                localised.clone()
            };
            let playable = context.union([&selection, &location.path], true)?;
            return Ok(Self {
                size,
                language,
                playable,
            });
        }
        let proved_absent = matches!(
            variant.wem_selection(),
            WemSelection::NotApplicable | WemSelection::UnsupportedStorage
        );
        let absent = context.union([&selection], proved_absent)?;
        let language = if variant.is_localised() {
            absent.clone()
        } else {
            localised.clone()
        };
        Ok(Self {
            size: absent.clone(),
            language,
            playable: absent,
        })
    }
}

pub(super) fn broken_link(
    context: &Context<'_>,
    link: &BrokenLink<'_>,
) -> Result<SoundBreakProvenance, SourceEvidenceError> {
    let route = context.route(link.event(), link.path().len(), link.path_sources())?;
    let selected = link.failed_edge_origin(context.archive, context.uses)?;
    let reason = match link.reason() {
        BreakReason::NoSoundbank => {
            let event = context.event_selector(link.event())?;
            context.union(
                std::iter::once(&event).chain(&context.no_bank),
                context.no_bank.is_some(),
            )?
        }
        BreakReason::Missing { .. }
        | BreakReason::OtherSoundbank { .. }
        | BreakReason::Cycle { .. } => context.union([&route, &selected], true)?,
        BreakReason::NoLocalisation { .. } => context.union(
            [&route, &selected, &context.preferred, &context.fallback],
            false,
        )?,
        BreakReason::Unreadable(_) => context.union([&route, &selected], false)?,
        BreakReason::Malformed(_)
        | BreakReason::UnknownLayout { .. }
        | BreakReason::TooManyObjects { .. } => context.union([&route], false)?,
        _ => route.clone(),
    };
    let soundbank = if link.soundbank().is_some() {
        context.bank(
            link.entry(context.archive),
            &route,
            SourceUseRole::Conversion,
        )?
    } else {
        reason.clone()
    };
    Ok(SoundBreakProvenance {
        event: context.event(link.event())?,
        soundbank,
        reason,
    })
}
