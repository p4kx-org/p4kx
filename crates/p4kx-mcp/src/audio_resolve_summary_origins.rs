// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{HircPathSource, TriggerSounds};
use p4kx_extraction::{FieldOrigin, SourceEncryption};

use crate::forge_origins::ForgeOrigins;

use super::super::super::surveys::SurveyMode;
use super::{Context, Evidence, breaks, variant};

pub(super) struct Totals {
    pub(super) variants: FieldOrigin,
    pub(super) breaks: FieldOrigin,
}

pub(super) fn write(
    context: &Context<'_>,
    output: &mut ForgeOrigins,
    sounds: &TriggerSounds<'_>,
    cursor: bool,
    mode: SurveyMode,
) -> Evidence<Totals> {
    let counts = Counts::read(context, sounds, mode)?;
    let topology = context.definition.events()?;
    let base = [&context.selected, &topology];
    let breaks = context.union(
        base.into_iter().chain(&counts.breaks),
        counts.broke_complete,
    )?;
    let visited = context.union(
        base.into_iter().chain(&counts.routes).chain([&breaks]),
        counts.routes_complete,
    )?;
    let measured_locations =
        context.union(std::iter::once(&visited).chain(&counts.locations), true)?;
    let media = context.union(std::iter::once(&visited).chain(&counts.media), true)?;
    context.members(
        output,
        "counts",
        [
            ("variants", visited.clone()),
            ("located", measured_locations.clone()),
            (
                "unlocated",
                context.union([&measured_locations, &media], true)?,
            ),
            ("plugins", media),
            ("breaks", breaks.clone()),
        ],
    )?;
    let cutoff = if let Some(link) = sounds
        .breaks()
        .iter()
        .find(|link| link.reason().truncated_by_limit())
    {
        breaks::Facts::read(context, link)?.reason
    } else {
        visited.clone()
    };
    output.put("truncated_by_limits", cutoff)?;
    if cursor {
        output.put("next_cursor", visited.clone())?;
    }
    Ok(Totals {
        variants: visited,
        breaks,
    })
}

#[derive(Default)]
struct Counts {
    routes: Vec<FieldOrigin>,
    locations: Vec<FieldOrigin>,
    media: Vec<FieldOrigin>,
    breaks: Vec<FieldOrigin>,
    routes_complete: bool,
    broke_complete: bool,
}

impl Counts {
    fn read(context: &Context<'_>, sounds: &TriggerSounds<'_>, mode: SurveyMode) -> Evidence<Self> {
        let variants = sounds.variants();
        let links = sounds.breaks();
        let variant_witness = matches!(mode, SurveyMode::Witness).then(|| {
            variants
                .iter()
                .position(|variant| encrypted(context, variant.path_sources()))
                .unwrap_or(0)
        });
        let break_witness = matches!(mode, SurveyMode::Witness).then(|| {
            links
                .iter()
                .position(|link| encrypted(context, link.path_sources()))
                .unwrap_or(0)
        });
        let mut counts = Self::default();
        for (index, variant) in variants.iter().enumerate() {
            if variant_witness.is_none_or(|witness| index == witness) {
                let proof = variant::Facts::read(context, variant)?;
                counts.routes.push(proof.route().clone());
                counts.locations.push(proof.location().clone());
                counts.media.push(proof.id);
            }
        }
        for (index, link) in links.iter().enumerate() {
            if break_witness.is_none_or(|witness| index == witness) {
                counts
                    .breaks
                    .push(breaks::Facts::read(context, link)?.reason);
            }
        }
        counts.routes_complete = variants.len() == counts.routes.len();
        counts.broke_complete = links.len() == counts.breaks.len();
        Ok(counts)
    }
}

fn encrypted(context: &Context<'_>, sources: &[HircPathSource]) -> bool {
    sources.iter().any(|source| {
        source.entry(context.archive).is_some_and(|entry| {
            entry
                .ancestors()
                .flatten()
                .any(|item| item.encryption() == SourceEncryption::Encrypted)
        })
    })
}
