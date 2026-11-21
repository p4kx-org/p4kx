// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

/// A failed technical invariant without source values in its display.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Check {
    MinimumParts,
    ModelledShare,
    ExportedParts,
    PalettePresent,
    CdfUnreadable,
    CdfParts,
    CdfModelled,
    HelperIssues,
    LayerEffect,
    ParentTransform,
    SkippedShare,
    BoxShare,
    OriginShare,
    HullShare,
    MeshSharing,
    OutputBytes,
    TexturePixels,
    Duration,
    Images,
    TextureFallbacks,
    TriggerCount,
    TriggerMatchShare,
    TriggerSoundShare,
    VariantCount,
    MediaLocationShare,
    StreamedCount,
    StreamedLocation,
    Containers,
    PlacedModels,
    OutsideShare,
    UnresolvedShare,
    InteriorIssues,
    ContainerPlacement,
}

impl fmt::Display for Check {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "verification_{self:?}")
    }
}

impl std::error::Error for Check {}
