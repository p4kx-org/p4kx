// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::collect::Collected;
use crate::manifest::{ColourSpace, TEXTURES_FOLDER, Texture};
use crate::textures::{Outcome, id_of};

pub(super) fn textures(collected: &Collected, outcomes: &[Outcome]) -> Vec<Texture> {
    collected
        .wanted
        .iter()
        .zip(outcomes)
        .enumerate()
        .filter_map(|(position, (wanted, outcome))| match outcome {
            Outcome::Written {
                width,
                height,
                colour_space,
                encoding,
                alpha,
                source_metadata,
                format,
                dds_asset,
                dds_fallback,
                smoothness,
                ..
            } => {
                let id = id_of(position);
                Some(Texture {
                    file: format!("{TEXTURES_FOLDER}/{id}.{}", format.extension()),
                    id,
                    source: wanted.path.clone(),
                    source_rendition: Some(wanted.mode.rendition()),
                    format: *format,
                    bit_depth: 8,
                    colour_space: *colour_space,
                    encoding: Some(*encoding),
                    alpha: *alpha,
                    ddna_pair: smoothness
                        .as_ref()
                        .map(|value| Box::new(value.pairing.clone())),
                    source_metadata: source_metadata.clone(),
                    dds_asset: dds_asset.clone(),
                    dds_fallback: *dds_fallback,
                    width: *width,
                    height: *height,
                })
            }
            Outcome::Failed { .. } => None,
        })
        .chain(outcomes.iter().filter_map(companion_texture))
        .collect()
}

fn companion_texture(outcome: &Outcome) -> Option<Texture> {
    let Outcome::Written {
        width,
        height,
        smoothness: Some(value),
        ..
    } = outcome
    else {
        return None;
    };
    Some(Texture {
        id: value.id.clone(),
        file: format!("{TEXTURES_FOLDER}/{}.dds", value.id),
        source: value.source.clone(),
        source_rendition: Some(crate::TextureRendition::SeparateSmoothness),
        format: crate::TextureFormat::Dds,
        bit_depth: 8,
        colour_space: ColourSpace::Linear,
        encoding: Some(crate::TextureEncoding::Data),
        alpha: crate::AlphaMeaning::None,
        ddna_pair: None,
        source_metadata: None,
        dds_asset: Some(Box::new(value.asset.clone())),
        dds_fallback: None,
        width: *width,
        height: *height,
    })
}
