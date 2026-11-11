// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_extraction::gfx::{AssetOrigins, BitmapSource, FontOrigins, ObservedAssets, TagSite};

use super::{Conversion, Payload, Projection, Selector, Site, aligned};

impl Projection {
    pub(super) fn definitions(&mut self, observed: &ObservedAssets<'_>) -> Result<()> {
        let assets = &observed.assets;
        let origins = &observed.origins;
        for (path, field) in [
            ("/shapes", "shapes"),
            ("/morph_shapes", "morphShapes"),
            ("/bitmaps", "bitmaps"),
            ("/fonts", "fonts"),
            ("/imports", "imports"),
            ("/symbol_classes", "symbolClasses"),
            ("/scaling_grids", "scalingGrids"),
        ] {
            self.logical(
                path,
                Site::Survey(origins.timeline.scan),
                field,
                Conversion,
                !origins.truncated,
            )?;
        }
        self.exporter(observed)?;
        self.shape_definitions(observed)?;
        for (index, (bitmap, &tag)) in aligned(&assets.bitmaps, &origins.bitmaps)?.enumerate() {
            let row = format!("/bitmaps/{index}");
            self.bitmap(&row, tag, &bitmap.source)?;
            self.export_name(&row, tag, bitmap.id, origins)?;
        }
        for (index, (font, provenance)) in aligned(&assets.fonts, &origins.fonts)?.enumerate() {
            let row = format!("/fonts/{index}");
            self.font(&row, provenance, origins)?;
            self.export_name(&row, provenance.definition, font.id, origins)?;
        }
        self.references(observed)
    }

    fn exporter(&mut self, observed: &ObservedAssets<'_>) -> Result<()> {
        let origins = &observed.origins;
        ensure!(
            origins.exporter.is_some() == observed.assets.exporter_info.is_some(),
            "GFx exporter info differs from its receipt"
        );
        if let Some(tag) = origins.exporter {
            let row = "/exporter";
            self.logical(row, Site::Body(tag), "exporterInfo", Payload, true)?;
            self.datum(
                row,
                Site::Body(tag),
                &[
                    ("version", Payload),
                    ("flags", Conversion),
                    ("bitmap_format", Payload),
                    ("prefix", Payload),
                    ("swf_name", Payload),
                ],
            )
        } else {
            self.logical(
                "/exporter",
                Site::Survey(origins.timeline.scan),
                "exporterInfoLookup",
                Selector,
                !origins.truncated && !origins.failed(1000),
            )
        }
    }

    fn shape_definitions(&mut self, observed: &ObservedAssets<'_>) -> Result<()> {
        let assets = &observed.assets;
        let origins = &observed.origins;
        let shapes = aligned(&assets.shapes, &origins.shapes)?.map(|(shape, &tag)| (shape.id, tag));
        let morphs = aligned(&assets.morph_shapes, &origins.morph_shapes)?
            .map(|(morph, &tag)| (morph.id, tag));
        self.shapes("/shapes", shapes, origins)?;
        self.shapes("/morph_shapes", morphs, origins)
    }

    fn shapes(
        &mut self,
        path: &str,
        rows: impl Iterator<Item = (u16, TagSite)>,
        origins: &AssetOrigins,
    ) -> Result<()> {
        for (index, (id, tag)) in rows.enumerate() {
            let row = format!("{path}/{index}");
            self.logical(&row, Site::Body(tag), "definition", Payload, true)?;
            self.logical(
                &format!("{row}/version"),
                Site::Header(tag),
                "code",
                Conversion,
                true,
            )?;
            self.datum(
                &row,
                Site::Body(tag),
                &[
                    ("id", Payload),
                    ("fill_styles", Conversion),
                    ("line_styles", Conversion),
                    ("edges", Conversion),
                ],
            )?;
            self.rect(&format!("{row}/bounds"), Site::Body(tag), "bounds")?;
            self.export_name(&row, tag, id, origins)?;
        }
        Ok(())
    }

    fn bitmap(&mut self, row: &str, tag: TagSite, bitmap: &BitmapSource<'_>) -> Result<()> {
        self.logical(row, Site::Body(tag), "bitmap", Payload, true)?;
        self.datum(row, Site::Body(tag), &[("id", Payload)])?;
        let kind = if matches!(bitmap, BitmapSource::Jpeg(_)) {
            Site::Body(tag)
        } else {
            Site::Header(tag)
        };
        self.logical(&format!("{row}/kind"), kind, "bitmapKind", Conversion, true)?;
        let authored: &[(&str, super::SourceUseRole)] = match bitmap {
            BitmapSource::Lossless(_) => &[
                ("width", Payload),
                ("height", Payload),
                ("format", Conversion),
            ],
            BitmapSource::External(_) => &[
                ("width", Payload),
                ("height", Payload),
                ("format", Conversion),
                ("file", Payload),
            ],
            BitmapSource::Jpeg(_) => &[("format", Conversion)],
            BitmapSource::SubImage(_) => &[
                ("width", Conversion),
                ("height", Conversion),
                ("image_id", Payload),
            ],
        };
        self.datum(row, Site::Body(tag), authored)?;
        for field in ["width", "height", "format", "file", "image_id"] {
            if !authored.iter().any(|(known, _)| *known == field) {
                self.logical(
                    &format!("{row}/{field}"),
                    Site::Header(tag),
                    "bitmapKind",
                    Selector,
                    true,
                )?;
            }
        }
        Ok(())
    }

    fn font(&mut self, row: &str, source: &FontOrigins, origins: &AssetOrigins) -> Result<()> {
        let definition = Site::Body(source.definition);
        self.logical(row, definition, "font", Payload, true)?;
        self.logical(
            &format!("{row}/format"),
            Site::Header(source.definition),
            "code",
            Conversion,
            true,
        )?;
        self.datum(
            row,
            definition,
            &[
                ("id", Payload),
                ("glyphs", Conversion),
                ("outlines", Conversion),
            ],
        )?;
        for (field, role) in [
            ("name", Payload),
            ("bold", Conversion),
            ("italic", Conversion),
        ] {
            let path = format!("{row}/{field}");
            if let Some(info) = source.info {
                self.font_override(&path, definition, info, field, role)?;
            } else {
                self.logical(&path, definition, field, role, true)?;
            }
        }
        let full_name = format!("{row}/full_name");
        if let Some(name) = source.full_name {
            self.font_override(&full_name, definition, name, "full_name", Payload)
        } else {
            let selector = self.decoded(definition, "id", Selector)?;
            let lookup = self.decoded(
                Site::Survey(origins.timeline.scan),
                "fontNameLookup",
                Selector,
            )?;
            self.bind_decoded(
                &full_name,
                vec![selector, lookup],
                !origins.truncated && !origins.failed(88),
            )
        }
    }

    fn font_override(
        &mut self,
        path: &str,
        definition: Site<'_>,
        tag: TagSite,
        field: &str,
        role: super::SourceUseRole,
    ) -> Result<()> {
        let source = Site::Body(tag);
        let actual = vec![
            self.decoded(definition, "id", Selector)?,
            self.decoded(source, "id", Selector)?,
            self.decoded(source, field, role)?,
        ];
        self.bind_decoded(path, actual, true)
    }

    pub(super) fn export_name(
        &mut self,
        row: &str,
        definition: TagSite,
        id: u16,
        origins: &AssetOrigins,
    ) -> Result<()> {
        let path = format!("{row}/export_name");
        let selector = self.decoded(Site::Body(definition), "id", Selector)?;
        if let Some(export) = origins.exports.get(&id) {
            let source = Site::Item(export.tag, export.ordinal);
            let actual = vec![
                selector,
                self.decoded(source, "id", Selector)?,
                self.decoded(source, "name", Payload)?,
            ];
            self.bind_decoded(&path, actual, true)
        } else {
            let lookup = self.decoded(
                Site::Survey(origins.timeline.scan),
                "firstExportLookup",
                Selector,
            )?;
            self.bind_decoded(
                &path,
                vec![selector, lookup],
                !origins.truncated && !origins.failed(56),
            )
        }
    }

    fn references(&mut self, observed: &ObservedAssets<'_>) -> Result<()> {
        let assets = &observed.assets;
        let origins = &observed.origins;
        for (index, (import, &tag)) in aligned(&assets.imports, &origins.imports)?.enumerate() {
            let row = format!("/imports/{index}");
            self.logical(&row, Site::Body(tag), "import", Payload, true)?;
            self.datum(
                &row,
                Site::Body(tag),
                &[("url", Payload), ("assets", Payload)],
            )?;
            for ordinal in 0..import.assets.len() {
                let row = format!("{row}/assets/{ordinal}");
                self.item(&row, Site::Item(tag, ordinal), "importedAsset")?;
            }
        }
        for (index, (_, item)) in
            aligned(&assets.symbol_classes, &origins.symbol_classes)?.enumerate()
        {
            self.item(
                &format!("/symbol_classes/{index}"),
                Site::Item(item.tag, item.ordinal),
                "symbolClass",
            )?;
        }
        for (index, (_, &tag)) in
            aligned(&assets.scaling_grids, &origins.scaling_grids)?.enumerate()
        {
            let row = format!("/scaling_grids/{index}");
            self.logical(&row, Site::Body(tag), "scalingGrid", Payload, true)?;
            self.datum(&row, Site::Body(tag), &[("id", Payload)])?;
            self.rect(&format!("{row}/splitter"), Site::Body(tag), "splitter")?;
        }
        Ok(())
    }

    fn item(&mut self, path: &str, site: Site<'_>, field: &str) -> Result<()> {
        self.logical(path, site, field, Payload, true)?;
        self.datum(path, site, &[("id", Payload), ("name", Payload)])
    }
}
