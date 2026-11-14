// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataForge;

use crate::compose::ScreenComposer;
use crate::reader::text;

const UI_ELEMENT: &str = "libs/foundry/records/ui/uielements/buildingblocks";
const FALLBACK: &str = "UI/fonts/Shared/fonts_en.gfx";

/// The selected font movie path and its authored DataForge selector.
#[derive(Debug)]
#[non_exhaustive]
pub struct ScreenFontFile {
    /// The selected path used by the Archive font reader.
    pub path: String,
    /// Original Record position when its fontFile supplied the path.
    pub record_index: Option<u32>,
}

impl ScreenComposer<'_, '_> {
    /// Returns the path of the movie that holds the fonts of the screens, as the `fontFile` of
    /// the `UIElement` Record `uielements/buildingblocks` names it, `UI/fonts/Shared/fonts_en.gfx`
    /// when the Archive has no such Record; the Archive holds it below `Data/`.
    #[must_use]
    pub fn font_file(&self) -> String {
        self.font_file_source().path
    }

    /// Returns the actual authored selector when the Record supplies the font movie.
    #[must_use]
    pub fn font_file_source(&self) -> ScreenFontFile {
        ScreenFontFile::read(self.store.forge())
    }
}

impl ScreenFontFile {
    /// Returns the font movie path selected by the original DataForge field.
    #[must_use]
    pub fn read(forge: &DataForge<'_>) -> Self {
        forge
            .records_at_path(UI_ELEMENT)
            .next()
            .and_then(|record| record.tree().ok())
            .and_then(|tree| {
                let path = text(tree.root(), "fontFile")?.to_owned();
                (!path.is_empty()).then_some(Self {
                    path,
                    record_index: Some(tree.record_index()),
                })
            })
            .unwrap_or_else(|| Self {
                path: FALLBACK.to_owned(),
                record_index: None,
            })
    }
}
