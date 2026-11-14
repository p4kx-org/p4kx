// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::texture_preview::{self, PreviewHeaderEvidence};
use p4kx_extraction::{
    ConversionError, DdsHeaderField, DdsSourceInfo, FieldOrigin, FieldOrigins, FieldPath,
    SourceInventory, SourceUseRole, SourceUses, Texture,
};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Origins {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(crate) fn capture(
    texture: &Texture<'_>,
    dds: &DdsSourceInfo,
    tail: Option<&Result<Texture<'_>, ConversionError>>,
    tail_metadata: Option<&DdsSourceInfo>,
) -> Result<Origins> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    let header = texture.preview_header_evidence(&uses)?;
    texture_preview::dds_info_origins(
        &mut fields,
        &FieldPath::parse("/source_metadata")?,
        dds,
        &header,
        &uses,
    )?;
    insert(&mut fields, "/texture", header.path_origin(&uses)?)?;
    header_fields(&mut fields, &header, &uses)?;
    tail_fields(&mut fields, tail, tail_metadata, &uses)?;
    let provenance = uses.freeze()?;
    Ok(Origins {
        field_origins: fields,
        provenance,
    })
}

fn header_fields(
    fields: &mut FieldOrigins,
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
) -> Result<()> {
    use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Width};
    for (path, word) in [
        ("/format", Format),
        ("/width", Width),
        ("/height", Height),
        ("/depth", Depth),
        ("/mip_count", MipCount),
        ("/faces", Faces),
        ("/cubemap", Faces),
        ("/array_size", ArraySize),
    ] {
        insert(fields, path, capture_words(header, &[word], uses)?)?;
    }
    insert(fields, "/parts", part_origin(header, uses)?)
}

fn tail_fields(
    fields: &mut FieldOrigins,
    tail: Option<&Result<Texture<'_>, ConversionError>>,
    metadata: Option<&DdsSourceInfo>,
    uses: &SourceUses,
) -> Result<()> {
    match (tail, metadata) {
        (Some(Ok(tail)), Some(dds)) => {
            let header = tail.preview_header_evidence(uses)?;
            let base = FieldPath::parse("/alpha_tail/source_metadata")?;
            texture_preview::dds_info_origins(fields, &base, dds, &header, uses)?;
            insert(
                fields,
                "/alpha_tail/format",
                capture_words(&header, &[DdsHeaderField::Format], uses)?,
            )?;
            insert(fields, "/alpha_tail/parts", part_origin(&header, uses)?)?;
            insert(fields, "/alpha_tail/error", uses.origin(&[], true))?;
        }
        (Some(Err(_)), _) => {
            for path in [
                "/alpha_tail/format",
                "/alpha_tail/parts",
                "/alpha_tail/error",
            ] {
                insert(fields, path, uses.origin(&[], false))?;
            }
        }
        _ => insert(fields, "/alpha_tail", uses.origin(&[], false))?,
    }
    Ok(())
}

fn part_origin(header: &PreviewHeaderEvidence, uses: &SourceUses) -> Result<FieldOrigin> {
    use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};
    let mut sources = header.header_uses(
        &[
            Prefix, Format, Width, Height, Depth, MipCount, Faces, ArraySize,
        ],
        SourceUseRole::Conversion,
    )?;
    sources.push(header.size_use()?);
    Ok(uses.origin(&sources, true))
}

fn capture_words(
    header: &PreviewHeaderEvidence,
    words: &[DdsHeaderField],
    uses: &SourceUses,
) -> Result<FieldOrigin> {
    let mut fields = header.header_uses(&[DdsHeaderField::Prefix], SourceUseRole::Selector)?;
    fields.extend(header.header_uses(words, SourceUseRole::Conversion)?);
    Ok(uses.origin(&fields, true))
}

fn insert(fields: &mut FieldOrigins, path: &str, origin: FieldOrigin) -> Result<()> {
    Ok(fields.insert(FieldPath::parse(path)?, origin)?)
}
