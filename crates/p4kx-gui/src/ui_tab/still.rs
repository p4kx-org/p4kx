// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, ScreenTexts};
use p4kx_screens::{
    ArchiveScreen, ArchiveScreenError, ArchiveScreenRender, ScreenError, StillRequest,
};

use super::render::{Fail, reason};

/// Draws the screen and returns the observed inputs with any text-loading warnings.
pub(super) fn draw(
    screen: &ArchiveScreen<'_>,
    texts: &ScreenTexts,
    request: &StillRequest<'_>,
    cancel: &CancelToken,
) -> Result<(ArchiveScreenRender, Vec<String>), Fail> {
    let mut warnings = Vec::new();
    let drawn = screen
        .render(texts, request, cancel, &mut warnings)
        .map_err(|error| match error {
            ArchiveScreenError::Draw(ScreenError::Stopped) => Fail::Stopped,
            other => reason(&other),
        })?;
    Ok((drawn, warnings))
}
