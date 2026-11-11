// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::io;

use p4kx_gfx::AssetOrigins;

use super::ui_render_frame_evidence::FrameContributions;

use crate::{
    FieldOrigin, InputScope, LocalizationEntry, SourceInput, SourceLocation, SourceUseRole,
    UiMovieEvidence, UiSourceSite,
};

/// A resolved localization value that supplied a rendered text field.
#[derive(Debug)]
#[non_exhaustive]
pub struct UiRenderedLocalization {
    /// Ordinal of the editable text definition in the movie.
    pub definition: usize,
    /// The authored lookup and the actual language Entry supplying its text.
    pub origin: FieldOrigin,
}

pub(crate) fn register(
    contributions: &mut FrameContributions,
    movie: &UiMovieEvidence,
    origins: &AssetOrigins,
    texts: Option<&LocalizationEntry>,
    definitions: BTreeSet<usize>,
) -> io::Result<Vec<UiRenderedLocalization>> {
    if definitions.is_empty() {
        return Ok(Vec::new());
    }
    let texts = texts.ok_or_else(inconsistent)?;
    let original = texts.origin();
    let input = original.map_or(SourceInput::Unknown, SourceInput::Archive);
    let scope = contributions
        .registry()
        .observe(input, InputScope::Payload)
        .map_err(io::Error::other)?;
    definitions
        .into_iter()
        .map(|definition| {
            let declaration = *origins
                .edit_texts
                .get(definition)
                .ok_or_else(inconsistent)?;
            let selector = movie
                .decoded(
                    UiSourceSite::Body(declaration),
                    "localizationKey",
                    SourceUseRole::Selector,
                )
                .map_err(io::Error::other)?;
            let translation = scope
                .at(
                    SourceLocation::LocalizationLookup {
                        language: texts.language(),
                        selector: selector.reference().clone(),
                    },
                    SourceUseRole::Conversion,
                )
                .map_err(io::Error::other)?;
            let mut evidence = vec![selector, translation];
            movie.append_decoding(&mut evidence);
            Ok(UiRenderedLocalization {
                definition,
                origin: contributions.add(evidence, movie.is_bound() && original.is_some()),
            })
        })
        .collect()
}

fn inconsistent() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "drawn translation has no observed lookup",
    )
}
