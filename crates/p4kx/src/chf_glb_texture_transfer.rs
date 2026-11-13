// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::texture_codec::DdsAlphaTransform;
use p4kx_extraction::{
    ArchiveTextureReads, FieldOrigin, FieldPath, OriginOutputAddress, SourceUses,
};
use p4kx_extraction::{TextureComponents, TextureReadUse};

use super::ImageOrigin;
use super::artifact::Fields;

pub(super) struct InputOrigins {
    declared: FieldOrigin,
    colour_encoding: Option<FieldOrigin>,
    alpha_association: Option<FieldOrigin>,
    alpha_transform: Option<FieldOrigin>,
}

pub(super) fn capture(
    reads: &[TextureReadUse],
    textures: &ArchiveTextureReads<'_, '_>,
    uses: &SourceUses,
) -> Result<Vec<InputOrigins>> {
    reads
        .iter()
        .copied()
        .filter(|read| read.components != TextureComponents::Dimensions)
        .map(|read| {
            let measured = textures.transfer_uses(read, uses)?;
            let complete = measured.is_some() && !read.fallback;
            let evidence = measured.unwrap_or_default();
            Ok(InputOrigins {
                declared: uses.origin(&evidence.declared, complete),
                colour_encoding: read
                    .transfer
                    .map(|_| uses.origin(&evidence.decoded, complete)),
                alpha_association: read
                    .alpha_association
                    .map(|_| uses.origin(&evidence.alpha_association, complete)),
                alpha_transform: (read.alpha_transform != DdsAlphaTransform::Preserve)
                    .then(|| uses.origin(&evidence.alpha_transform, complete)),
            })
        })
        .collect()
}

pub(super) fn addresses(fields: &mut Fields, image: &ImageOrigin, uses: &SourceUses) -> Result<()> {
    let index = image.index;
    let base = format!("/images/{index}/extras/p4kx_transfer");
    let mut add = |suffix: &str, origin| -> Result<()> {
        fields.push((
            OriginOutputAddress::GlbJsonField {
                path: FieldPath::parse(&format!("{base}/{suffix}"))?,
            },
            origin,
        ));
        Ok(())
    };
    add("output", uses.origin(&[], true))?;
    for (input, evidence) in image.transfer.iter().enumerate() {
        add(
            &format!("inputs/{input}/declared"),
            evidence.declared.clone(),
        )?;
        for (name, origin) in [
            ("alpha_association", &evidence.alpha_association),
            ("alpha_transform", &evidence.alpha_transform),
        ] {
            if let Some(origin) = origin {
                add(&format!("inputs/{input}/{name}"), origin.clone())?;
            }
        }
        if let Some(encoding) = &evidence.colour_encoding {
            for field in ["transfer", "transform"] {
                add(
                    &format!("inputs/{input}/colour_encoding/{field}"),
                    encoding.clone(),
                )?;
            }
        }
    }
    Ok(())
}
