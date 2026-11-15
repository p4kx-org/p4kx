// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::{DataForge, Guid, RecordTree, Value};
use p4kx_gfx::Rgba;

use crate::canvas::{Canvas, StyleRef};
use crate::reader::{as_struct, child, float, int, items, reference};

const COLOUR_SLOT_MARKERS: [&str; 3] = ["Base", "Accent1", "Bright"];

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ColorSpec {
    Solid(Rgba),
    Slot { name: String, alpha: f32 },
    Record { guid: Guid, alpha: f32 },
    Other(String),
}

impl ColorSpec {
    pub(crate) fn parse(value: &Value<'_>) -> Option<Self> {
        let structure = as_struct(value)?;
        Some(match structure.type_name() {
            "BuildingBlocks_ColorSolid" => solid(structure).map_or_else(
                || Self::Other("BuildingBlocks_ColorSolid".to_owned()),
                Self::Solid,
            ),
            "BuildingBlocks_ColorStyle" => Self::Slot {
                name: crate::reader::text(structure, "color")?.to_owned(),
                alpha: float(structure, "alpha").unwrap_or(1.0),
            },
            "BuildingBlocks_ColorRecordReference" => Self::Record {
                guid: reference(structure, "color")?.guid(),
                alpha: float(structure, "alpha").unwrap_or(1.0),
            },
            other => Self::Other(other.to_owned()),
        })
    }
}

fn solid(structure: &p4kx_dataforge::StructValue<'_>) -> Option<Rgba> {
    let colour = child(structure, "color")?;
    let channel = |name| int(colour, name).and_then(|value| u8::try_from(value).ok());
    Some(Rgba {
        red: channel("r")?,
        green: channel("g")?,
        blue: channel("b")?,
        alpha: channel("a").unwrap_or(u8::MAX),
    })
}

pub(crate) fn slot_names(forge: &DataForge<'_>) -> Vec<String> {
    for candidate in forge.enums() {
        let names: Vec<&str> = candidate.options().filter_map(Result::ok).collect();
        if COLOUR_SLOT_MARKERS
            .iter()
            .all(|marker| names.contains(marker))
        {
            return names.into_iter().map(str::to_owned).collect();
        }
    }
    Vec::new()
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaletteSite {
    pub(crate) record_index: u32,
    pub(crate) slot: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Palette {
    slots: HashMap<String, Rgba>,
    #[cfg(test)]
    pub(crate) sources: HashMap<String, PaletteSite>,
}

impl Palette {
    pub(crate) fn overlay(&mut self, other: Self) {
        self.slots.extend(other.slots);
        #[cfg(test)]
        self.sources.extend(other.sources);
    }

    pub(crate) fn of_canvas(
        forge: &DataForge<'_>,
        slot_names: &[String],
        canvas: &Canvas,
        brand: Option<&str>,
        inherited: &Self,
        cache: &mut HashMap<Guid, Self>,
    ) -> Self {
        let own = cache.entry(canvas.guid).or_insert_with(|| {
            let chosen = canvas.pick_brand(brand).map(|chosen| &chosen.style);
            let styles: Vec<&StyleRef> = canvas.shared.iter().chain(chosen).collect();
            Self::of_styles(forge, slot_names, &styles)
        });
        let mut palette = inherited.clone();
        palette.overlay(own.clone());
        palette
    }

    pub(crate) fn of_styles(
        forge: &DataForge<'_>,
        slot_names: &[String],
        styles: &[&StyleRef],
    ) -> Self {
        let mut palette = Self::default();
        for style in styles {
            if let Some(tree) = forge
                .record_by_guid(style.guid)
                .and_then(|record| record.tree().ok())
            {
                palette.overlay(Self::from_style(forge, &tree, slot_names));
            }
        }
        palette
    }

    pub(crate) fn from_style(
        forge: &DataForge<'_>,
        style: &RecordTree<'_>,
        slot_names: &[String],
    ) -> Self {
        let mut palette = Self::default();
        for (slot, value) in items(style.root(), "colorStyles").iter().enumerate() {
            let Some(name) = slot_names.get(slot) else {
                break;
            };
            if let Some(colour) = resolved_entry(forge, value) {
                palette.slots.insert(name.clone(), colour);
                #[cfg(test)]
                palette.sources.insert(
                    name.clone(),
                    PaletteSite {
                        record_index: style.record_index(),
                        slot,
                    },
                );
            }
        }
        palette
    }

    pub(crate) fn resolve(&self, spec: &ColorSpec, inherited: f32) -> Result<Rgba, String> {
        let (base, alpha) = match spec {
            ColorSpec::Solid(colour) => (*colour, 1.0),
            ColorSpec::Slot { name, alpha } => (
                *self
                    .slots
                    .get(name)
                    .ok_or_else(|| format!("colour slot {name} is not in the palette"))?,
                *alpha,
            ),
            ColorSpec::Record { .. } => {
                return Err("colour that is a reference to a colour Record".to_owned());
            }
            ColorSpec::Other(kind) => return Err(format!("colour of kind {kind}")),
        };
        Ok(scale_alpha(base, alpha * inherited))
    }
}

fn resolved_entry(forge: &DataForge<'_>, value: &Value<'_>) -> Option<Rgba> {
    match ColorSpec::parse(value)? {
        ColorSpec::Solid(colour) => Some(colour),
        ColorSpec::Record { guid, alpha } => {
            let record = forge.record_by_guid(guid)?.tree().ok()?;
            let inner = record.field("color")?;
            let ColorSpec::Solid(colour) = ColorSpec::parse(inner)? else {
                return None;
            };
            Some(scale_alpha(colour, alpha))
        }
        ColorSpec::Slot { .. } | ColorSpec::Other(_) => None,
    }
}

pub(crate) fn scale_alpha(colour: Rgba, factor: f32) -> Rgba {
    let factor = factor.clamp(0.0, 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the product of a byte and a factor from 0 to 1 is a byte"
    )]
    let alpha = (f32::from(colour.alpha) * factor).round() as u8;
    Rgba { alpha, ..colour }
}
