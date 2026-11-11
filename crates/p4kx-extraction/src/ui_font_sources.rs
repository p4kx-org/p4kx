// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::rc::Rc;

use p4kx_archive::ObservedEntry;
use p4kx_gfx::{AssetOrigins, FontOrigins, MovieSource, TagItemSite, TagSite};

use crate::{SourceInput, SourceUse, SourceUseRole, SourceUses, UiMovieEvidence, UiSourceSite};

use SourceUseRole::{Conversion, Selector};
use UiSourceSite as Site;

pub(crate) struct LoadedSources<'archive> {
    entry: Option<ObservedEntry<'archive>>,
    read: MovieSource,
    fonts: Vec<FontOrigins>,
    exports: Vec<TagItemSite>,
    imports: Vec<TagSite>,
}

impl<'archive> LoadedSources<'archive> {
    pub(crate) fn new(
        entry: Option<ObservedEntry<'archive>>,
        read: MovieSource,
        origins: AssetOrigins,
    ) -> Self {
        Self {
            entry,
            read,
            fonts: origins.fonts,
            exports: origins.export_items,
            imports: origins.imports,
        }
    }
}

pub(crate) struct FontHop<'archive> {
    pub(crate) movie: Rc<LoadedSources<'archive>>,
    pub(crate) export_index: usize,
    pub(crate) forwarding: Option<(usize, usize)>,
}

pub(crate) struct FontRoute<'archive> {
    pub(crate) font_index: usize,
    pub(crate) hops: Vec<FontHop<'archive>>,
}

pub(crate) struct ImportedFontSource<'archive> {
    pub(crate) import_index: usize,
    pub(crate) item_index: usize,
    pub(crate) route: FontRoute<'archive>,
}

#[derive(Clone, Copy)]
pub(crate) enum FontUse {
    Outline {
        glyph: usize,
        by_code: bool,
    },
    Layout {
        glyph: Option<usize>,
        used_advance: bool,
    },
}

pub(crate) struct FrameFonts<'archive> {
    local_count: usize,
    imported: Vec<Option<ImportedFontSource<'archive>>>,
}

impl<'archive> FrameFonts<'archive> {
    pub(crate) const fn new(
        local_count: usize,
        imported: Vec<Option<ImportedFontSource<'archive>>>,
    ) -> Self {
        Self {
            local_count,
            imported,
        }
    }

    pub(crate) fn register(
        &self,
        uses: &SourceUses,
        primary: &UiMovieEvidence,
        origins: &AssetOrigins,
        font: usize,
        input: FontUse,
    ) -> io::Result<(Vec<SourceUse>, bool)> {
        if let Some(index) = font.checked_sub(self.local_count) {
            let imported = self
                .imported
                .get(index)
                .and_then(Option::as_ref)
                .ok_or_else(inconsistent)?;
            imported.register(uses, primary, origins, input)
        } else {
            let font = origins.fonts.get(font).ok_or_else(inconsistent)?;
            let mut evidence = Vec::new();
            font_uses(&mut evidence, primary, font, input)?;
            primary.append_decoding(&mut evidence);
            Ok((evidence, primary.is_bound()))
        }
    }
}

impl ImportedFontSource<'_> {
    fn register(
        &self,
        uses: &SourceUses,
        primary: &UiMovieEvidence,
        origins: &AssetOrigins,
        input: FontUse,
    ) -> io::Result<(Vec<SourceUse>, bool)> {
        let tag = *origins
            .imports
            .get(self.import_index)
            .ok_or_else(inconsistent)?;
        let mut evidence = vec![observe(
            primary,
            Site::Item(tag, self.item_index),
            "importedFont",
            Selector,
        )?];
        primary.append_decoding(&mut evidence);
        let mut complete = primary.is_bound();
        for hop in self.route.hops.iter().rev() {
            complete &= hop.register(&mut evidence, uses, self.route.font_index, input)?;
        }
        Ok((evidence, complete))
    }
}

impl FontHop<'_> {
    fn register(
        &self,
        evidence: &mut Vec<SourceUse>,
        uses: &SourceUses,
        font: usize,
        input: FontUse,
    ) -> io::Result<bool> {
        let movie = UiMovieEvidence::of_input(
            self.movie.entry.map(SourceInput::ArchiveView),
            self.movie.read,
            uses,
        )
        .map_err(io::Error::other)?;
        let export = self
            .movie
            .exports
            .get(self.export_index)
            .ok_or_else(inconsistent)?;
        evidence.push(observe(
            &movie,
            Site::Item(export.tag, export.ordinal),
            "fontExport",
            Selector,
        )?);
        if let Some((tag, item)) = self.forwarding {
            let tag = *self.movie.imports.get(tag).ok_or_else(inconsistent)?;
            evidence.push(observe(
                &movie,
                Site::Item(tag, item),
                "forwardedFont",
                Selector,
            )?);
        } else {
            let font = self.movie.fonts.get(font).ok_or_else(inconsistent)?;
            font_uses(evidence, &movie, font, input)?;
        }
        movie.append_decoding(evidence);
        Ok(movie.is_bound())
    }
}

fn font_uses(
    evidence: &mut Vec<SourceUse>,
    movie: &UiMovieEvidence,
    font: &FontOrigins,
    input: FontUse,
) -> io::Result<()> {
    let code = match input {
        FontUse::Outline { glyph, by_code } => {
            evidence.push(observe(
                movie,
                Site::Item(font.definition, glyph),
                "glyphOutline",
                Conversion,
            )?);
            by_code.then_some(glyph)
        }
        FontUse::Layout {
            glyph,
            used_advance,
        } => {
            evidence.push(observe(
                movie,
                Site::Body(font.definition),
                "layoutFont",
                Conversion,
            )?);
            if used_advance && let Some(glyph) = glyph {
                evidence.push(observe(
                    movie,
                    Site::Item(font.definition, glyph),
                    "glyphAdvance",
                    Conversion,
                )?);
            }
            glyph
        }
    };
    if let Some(code) = code.and_then(|glyph| font.glyph_code_override(glyph)) {
        evidence.push(observe(
            movie,
            Site::Item(code.tag, code.ordinal),
            "glyphCode",
            Selector,
        )?);
    }
    Ok(())
}

fn observe(
    movie: &UiMovieEvidence,
    site: Site<'_>,
    field: &str,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    movie.decoded(site, field, role).map_err(io::Error::other)
}

fn inconsistent() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "selected font has no observed source",
    )
}
