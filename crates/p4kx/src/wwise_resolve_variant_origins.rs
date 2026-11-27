// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::{SoundVariant, WemLocation, WemSelection};
use p4kx_extraction::{FieldOrigin, FieldPath};

use super::{Capture, field};

pub(super) struct VariantOrigins {
    pub(super) row: FieldOrigin,
    pub(super) route: FieldOrigin,
    pub(super) media: FieldOrigin,
    pub(super) location: FieldOrigin,
}

pub(super) fn write(
    output: &mut Capture<'_>,
    row: &FieldPath,
    variant: &SoundVariant<'_>,
) -> Result<VariantOrigins> {
    let event = output.definition.event(&output.uses, variant.event())?;
    let route = output.path(
        Some(row),
        variant.path().len(),
        variant.path_sources(),
        &event.id,
    )?;
    let bank = output.bank(
        &field(row, "soundbank")?,
        variant
            .path_sources()
            .last()
            .and_then(|step| step.entry(output.archive)),
        &route.path,
    )?;
    let object = output.union(
        std::iter::once(&route.path).chain(route.end.iter().map(|step| &step.id)),
        route.end.is_some(),
    )?;
    let terminal = variant.terminal_media_origins(output.archive, &output.uses)?;
    let source = output.union([&route.path, &terminal.source_kind], true)?;
    let media = output.union([&route.path, &terminal.wem_id], true)?;
    let localised = output.union([&route.path, &terminal.localised], true)?;
    let location = location(output, row, variant, [&media, &source, &localised])?;
    let language = if variant.is_localised() {
        output.union([&localised, &location.language], true)?
    } else {
        localised.clone()
    };
    let fallback = if output.language.has_preference && variant.is_localised() {
        output.union([&language, &output.language.requested], true)?
    } else {
        localised.clone()
    };
    let leaves = output.group(
        row,
        [
            ("event", event.name),
            ("sound_id", object),
            ("wem_id", media.clone()),
            ("source", source),
            ("localised", localised),
            ("language", language),
            ("fallback", fallback),
        ],
    )?;
    let result = output.union([&leaves, &route.path, &bank, &location.root], true)?;
    output.fields.insert(row.clone(), result.clone())?;
    Ok(VariantOrigins {
        row: result,
        route: route.path,
        media,
        location: location.root,
    })
}

struct Location {
    root: FieldOrigin,
    language: FieldOrigin,
}

fn location(
    output: &mut Capture<'_>,
    row: &FieldPath,
    variant: &SoundVariant<'_>,
    terminal: [&FieldOrigin; 3],
) -> Result<Location> {
    let rule = match variant.wem_selection() {
        WemSelection::LoosePreferredLanguage => Some(&output.language.requested),
        WemSelection::LooseFallbackLanguage => Some(&output.language.fallback),
        _ => None,
    };
    let Some(location) = variant.location() else {
        let certain = matches!(
            variant.wem_selection(),
            WemSelection::NotApplicable | WemSelection::UnsupportedStorage
        );
        let root = output.union(terminal.into_iter().chain(rule), certain)?;
        output.put(row, "location", root.clone())?;
        return Ok(Location {
            language: root.clone(),
            root,
        });
    };
    let sites = variant.wem_location_origins(output.archive, &output.uses)?;
    let selected = output.union(terminal.into_iter().chain(rule).chain([&sites.kind]), true)?;
    let address = field(row, "location")?;
    let member = match location {
        WemLocation::Soundbank { .. } => "soundbank",
        WemLocation::Entry { .. } => "entry",
        _ => {
            let root = output.union([&selected], false)?;
            output.fields.insert(address, root.clone())?;
            return Ok(Location {
                language: root.clone(),
                root,
            });
        }
    };
    let declared = output.group(
        &address,
        [
            ("kind", selected.clone()),
            (member, output.union([&selected, &sites.path], true)?),
            ("len", output.union([&selected, &sites.len], true)?),
        ],
    )?;
    let root = if let WemLocation::Soundbank { .. } = location {
        let span = sites
            .offset
            .unwrap_or_else(|| output.uses.origin(&[], false));
        let offset = output.union([&selected, &span], true)?;
        output.put(&address, "offset", offset.clone())?;
        output.union([&declared, &offset], true)?
    } else {
        declared
    };
    let language = output.union([&selected, &sites.language], true)?;
    output.fields.insert(address, root.clone())?;
    Ok(Location { root, language })
}
