// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{SoundVariant, WemLocationOrigins, WemSelection};
use p4kx_extraction::FieldOrigin;

use crate::audio_shapes::{LocationShape, VariantShape};
use crate::forge_origins::ForgeOrigins;

use super::path::Chain;
use super::{Context, Evidence};

pub(super) fn write(
    context: &Context<'_>,
    output: &mut ForgeOrigins,
    variants: &[SoundVariant<'_>],
    shapes: &[VariantShape],
    total: &FieldOrigin,
) -> Evidence<()> {
    let mut listed = Vec::new();
    if variants.is_empty() && shapes.is_empty() {
        return output.put("variants", total.clone());
    }
    for (index, (variant, shape)) in variants.iter().zip(shapes).enumerate() {
        let address = format!("variants/{index}");
        listed
            .push(Facts::read(context, variant)?.write(context, output, &address, variant, shape)?);
    }
    let origin = context.union(&listed, variants.len() == shapes.len())?;
    output.put("variants", origin)
}

pub(super) struct Facts {
    chain: Chain,
    pub(super) id: FieldOrigin,
    source: FieldOrigin,
    localised: FieldOrigin,
    language: FieldOrigin,
    fallback: FieldOrigin,
    location: Location,
}

impl Facts {
    pub(super) fn read(context: &Context<'_>, variant: &SoundVariant<'_>) -> Evidence<Self> {
        let chain = Chain::read(
            context,
            variant.event(),
            variant.path().len(),
            variant.path_sources(),
        )?;
        let terminal = variant.terminal_media_origins(context.archive, context.uses)?;
        let id = context.union([&chain.route, &terminal.wem_id], true)?;
        let source = context.union([&chain.route, &terminal.source_kind], true)?;
        let localised = context.union([&chain.route, &terminal.localised], true)?;
        let location = Location::read(context, variant, &id, &source, &localised)?;
        let language = if variant.is_localised() {
            context.union([&localised, &location.language], true)?
        } else {
            localised.clone()
        };
        let fallback = if context.has_preference && variant.is_localised() {
            context.union([&language, &context.preferred], true)?
        } else {
            localised.clone()
        };
        Ok(Self {
            chain,
            id,
            source,
            localised,
            language,
            fallback,
            location,
        })
    }

    pub(super) const fn route(&self) -> &FieldOrigin {
        &self.chain.route
    }

    pub(super) const fn location(&self) -> &FieldOrigin {
        &self.location.origin
    }

    fn write(
        self,
        context: &Context<'_>,
        output: &mut ForgeOrigins,
        address: &str,
        variant: &SoundVariant<'_>,
        shape: &VariantShape,
    ) -> Evidence<FieldOrigin> {
        self.chain
            .write(context, output, &format!("{address}/path"))?;
        let bank = context.bank(
            output,
            &format!("{address}/soundbank"),
            variant
                .path_sources()
                .last()
                .and_then(|source| source.entry(context.archive)),
            &self.chain.route,
        )?;
        let location = self.location.write(
            context,
            output,
            &format!("{address}/location"),
            shape.location.as_ref(),
        )?;
        context.members(
            output,
            address,
            [
                ("event", context.event_name(variant.event())?),
                ("soundbank", bank),
                (
                    "sound_id",
                    context.union([&self.chain.route, &self.chain.last_id], true)?,
                ),
                ("wem_id", self.id),
                ("source", self.source),
                ("localised", self.localised),
                ("language", self.language),
                ("fallback", self.fallback),
                ("location", location),
                ("path", self.chain.route),
            ],
        )
    }
}

struct Location {
    origin: FieldOrigin,
    language: FieldOrigin,
    selection: FieldOrigin,
    intrinsic: WemLocationOrigins,
}

impl Location {
    fn read(
        context: &Context<'_>,
        variant: &SoundVariant<'_>,
        id: &FieldOrigin,
        source: &FieldOrigin,
        localised: &FieldOrigin,
    ) -> Evidence<Self> {
        let language = match variant.wem_selection() {
            WemSelection::LoosePreferredLanguage => Some(&context.preferred),
            WemSelection::LooseFallbackLanguage => Some(&context.fallback),
            _ => None,
        };
        let intrinsic = variant.wem_location_origins(context.archive, context.uses)?;
        let selected_location = variant.location().is_some().then_some(&intrinsic.kind);
        let selection = context.union(
            [id, source, localised]
                .into_iter()
                .chain(language)
                .chain(selected_location),
            true,
        )?;
        let complete_absence = matches!(
            variant.wem_selection(),
            WemSelection::NotApplicable | WemSelection::UnsupportedStorage
        );
        let origin = if variant.location().is_some() {
            context.union(
                [
                    &selection,
                    &intrinsic.kind,
                    &intrinsic.path,
                    &intrinsic.language,
                    &intrinsic.len,
                ]
                .into_iter()
                .chain(&intrinsic.offset),
                true,
            )?
        } else {
            context.union([&selection], complete_absence)?
        };
        let language = if variant.location().is_some() {
            context.union([&selection, &intrinsic.language], true)?
        } else {
            origin.clone()
        };
        Ok(Self {
            origin,
            language,
            selection,
            intrinsic,
        })
    }

    fn write(
        self,
        context: &Context<'_>,
        output: &mut ForgeOrigins,
        address: &str,
        shape: Option<&LocationShape>,
    ) -> Evidence<FieldOrigin> {
        if let Some(shape) = shape {
            for (member, field) in [
                ("kind", self.intrinsic.kind),
                ("path", self.intrinsic.path),
                ("language", self.intrinsic.language),
                ("len", self.intrinsic.len),
            ] {
                output.put(
                    &format!("{address}/{member}"),
                    context.union([&self.selection, &field], true)?,
                )?;
            }
            if shape.offset.is_some() {
                let offset = self
                    .intrinsic
                    .offset
                    .unwrap_or_else(|| context.uses.origin(&[], false));
                output.put(
                    &format!("{address}/offset"),
                    context.union([&self.selection, &offset], true)?,
                )?;
            }
        }
        Ok(self.origin)
    }
}
