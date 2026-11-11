// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_gfx::{AssetOrigins, TagSite};

use super::ui_bitmap_decode::{BitmapPixels, BitmapRead};
use crate::ui_external::ExternalPath;
use crate::{SourceUse, SourceUseRole, SourceUses, UiMovieEvidence, UiSourceSite as Site};

use SourceUseRole::{Conversion, Selector};

pub(crate) fn contributions(
    movie: &UiMovieEvidence,
    origins: &AssetOrigins,
    registry: &SourceUses,
    order: usize,
    bitmap: &BitmapRead<'_>,
) -> io::Result<(Vec<SourceUse>, bool)> {
    let definition = *origins
        .bitmaps
        .get(order)
        .ok_or_else(|| invalid("drawn bitmap has no physical definition"))?;
    let base = *origins
        .bitmaps
        .get(bitmap.order)
        .ok_or_else(|| invalid("decoded bitmap has no base definition"))?;
    let mut uses = vec![decoded(movie, Site::Body(base), "baseBitmapId", Selector)?];
    if order != bitmap.order {
        uses.push(decoded(
            movie,
            Site::Body(definition),
            "atlasImageId",
            Selector,
        )?);
        uses.push(decoded(
            movie,
            Site::Body(definition),
            "cropRectangle",
            Conversion,
        )?);
    }
    let complete = match &bitmap.pixels {
        BitmapPixels::Embedded { used_jpeg_tables } => {
            uses.push(decoded(
                movie,
                Site::Body(base),
                "embeddedBitmap",
                Conversion,
            )?);
            if *used_jpeg_tables {
                let tables = origins
                    .jpeg_tables
                    .ok_or_else(|| invalid("bitmap decoder used unobserved JPEG tables"))?;
                uses.push(decoded(
                    movie,
                    Site::Body(tables),
                    "jpegTables",
                    Conversion,
                )?);
            }
            true
        }
        BitmapPixels::External { selection, inputs } => {
            external_selectors(movie, &mut uses, base, *selection)?;
            let dds = inputs.register(registry, 0).map_err(io::Error::other)?;
            dds.append_pixel_uses(&mut uses).map_err(io::Error::other)?;
            uses.extend(dds.alpha_lookup_uses().map_err(io::Error::other)?);
            dds.is_complete()
        }
    };
    movie.append_decoding(&mut uses);
    Ok((uses, complete && movie.is_bound()))
}

pub(crate) fn external_selectors(
    movie: &UiMovieEvidence,
    uses: &mut Vec<SourceUse>,
    definition: TagSite,
    selected: ExternalPath,
) -> io::Result<()> {
    uses.push(decoded(
        movie,
        Site::Body(definition),
        selected.field(),
        Selector,
    )?);
    if matches!(selected, ExternalPath::ExportName) {
        uses.push(decoded(
            movie,
            Site::Body(definition),
            "fileName",
            Selector,
        )?);
    }
    Ok(())
}

fn decoded(
    movie: &UiMovieEvidence,
    site: Site<'_>,
    field: &str,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    movie.decoded(site, field, role).map_err(io::Error::other)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
