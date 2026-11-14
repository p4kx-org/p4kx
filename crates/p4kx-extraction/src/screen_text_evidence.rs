// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_archive::EntryOrigin;
use p4kx_gfx::{Font, FontOrigins, MovieSource, TagItemSite};

use crate::{
    Archive, GlobalIni, IniEncoding, InputScope, LocalizationEntry, SourceEvidenceError,
    SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses, UiMovie, UiMovieEvidence,
    UiSourceSite,
};

use SourceUseRole::{Conversion, Selector};

#[derive(Debug)]
pub(super) struct ScreenFontMovie {
    fonts: Vec<Font>,
    original: Option<OriginalFonts>,
}

#[derive(Debug)]
struct OriginalFonts {
    record_path: String,
    input: Option<EntryOrigin>,
    read: MovieSource,
    fonts: Vec<FontOrigins>,
    exports: BTreeMap<u16, TagItemSite>,
}

impl ScreenFontMovie {
    pub(super) fn read(
        archive: &Archive,
        record_path: &str,
        observe: bool,
    ) -> Result<Self, String> {
        let under_data = format!("Data/{record_path}");
        let movie = UiMovie::from_archive(archive, record_path)
            .or_else(|_| UiMovie::from_archive(archive, &under_data))
            .map_err(|error| format!("the font movie {record_path} of the screens: {error}"))?;
        if !observe {
            return Ok(Self {
                fonts: movie.movie().assets().fonts,
                original: None,
            });
        }
        let observed = movie.movie().assets_observed();
        let input = match movie.source_input() {
            Some(SourceInput::ArchiveView(entry)) => Some(entry.snapshot()),
            Some(SourceInput::Archive(entry)) => Some(entry.clone()),
            _ => None,
        };
        Ok(Self {
            fonts: observed.assets.fonts,
            original: Some(OriginalFonts {
                record_path: record_path.to_owned(),
                input,
                read: *movie.movie().read_source(),
                fonts: observed.origins.fonts,
                exports: observed.origins.exports,
            }),
        })
    }

    pub(super) fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    pub(super) fn uses_selection(&self, archive: &Archive, record_path: &str) -> bool {
        self.original.as_ref().is_some_and(|original| {
            original.record_path == record_path
                && original
                    .input
                    .as_ref()
                    .is_some_and(|entry| archive.owns_origin(entry))
        })
    }

    pub(super) fn append_uses(
        &self,
        font: usize,
        registry: &SourceUses,
        output: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        let Some(original) = &self.original else {
            return Ok(false);
        };
        let Some(sites) = original.fonts.get(font) else {
            return Ok(false);
        };
        let source = UiMovieEvidence::of_input(
            original.input.as_ref().map(SourceInput::Archive),
            original.read,
            registry,
        )?;
        output.push(source.decoded(
            UiSourceSite::Body(sites.definition),
            "screenFont",
            Conversion,
        )?);
        for (site, field) in [(sites.info, "fontInfo"), (sites.full_name, "fontFullName")] {
            if let Some(site) = site {
                output.push(source.decoded(UiSourceSite::Body(site), field, Selector)?);
            }
        }
        if let Some(export) = self
            .fonts
            .get(font)
            .filter(|font| font.export_name.is_some())
            .and_then(|font| original.exports.get(&font.id))
        {
            output.push(source.decoded(
                UiSourceSite::Item(export.tag, export.ordinal),
                "fontExportName",
                Selector,
            )?);
        }
        source.append_decoding(output);
        Ok(source.is_bound())
    }
}

pub(super) fn localization(
    entry: &LocalizationEntry,
    ini: &GlobalIni<'_>,
    key: &str,
    registry: &SourceUses,
    output: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    let Some(value) = ini.resolve(key) else {
        return Ok(false);
    };
    let text = entry.text();
    let decoded = text.text();
    let Some(offset) = (value.as_ptr() as usize).checked_sub(decoded.as_ptr() as usize) else {
        return Ok(false);
    };
    let Some(end) = offset.checked_add(value.len()) else {
        return Ok(false);
    };
    if decoded.get(offset..end) != Some(value) {
        return Ok(false);
    }
    let original = entry.origin();
    let input = original.map_or(SourceInput::Unknown, SourceInput::Archive);
    let scope = registry.observe(input, InputScope::Payload)?;
    let offset = offset as u64;
    let (namespace, offset) = match (text.encoding(), text.is_lossy()) {
        (IniEncoding::Utf8, false) => ("globalIniBytes", offset),
        (IniEncoding::Utf8Bom, false) => ("globalIniBytes", offset + "\u{feff}".len() as u64),
        _ => {
            output.push(scope.at(
                SourceLocation::Subresource {
                    namespace: "globalIniInputBytes".into(),
                    indices: vec![0, entry.original_byte_len()],
                    field: "decodeInput".into(),
                },
                Conversion,
            )?);
            ("globalIniDecodedBytes", offset)
        }
    };
    output.push(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices: vec![offset, value.len() as u64],
            field: "value".into(),
        },
        Conversion,
    )?);
    Ok(original.is_some())
}
