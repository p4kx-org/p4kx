// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use crate::{FieldOrigin, OriginArtifactDigest, SourceInventory, TextureImageFace};

/// The converted images published from one texture and their shared source inventory.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct TextureArtifacts {
    /// Images committed by this extraction in DDS storage order.
    pub images: Vec<TextureImageArtifact>,
    /// The inventory in which each image's source use references resolve.
    pub provenance: SourceInventory,
}

/// The byte binding and contributors of one converted image published by bulk extraction.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct TextureImageArtifact {
    /// The image path relative to the extraction target, with forward slash separators.
    pub path: String,
    /// The measured byte length and SHA-256 of the committed image.
    pub digest: OriginArtifactDigest,
    /// The actual selected mip level, with zero denoting the largest.
    pub level: u32,
    /// The zero-based source array layer.
    pub layer: u32,
    /// The source face.
    pub face: TextureImageFace,
    /// The zero-based volume slice at this level.
    pub slice: u32,
    /// The header fields and mip payloads that supplied this image's pixels.
    #[serde(flatten)]
    pub origin: FieldOrigin,
}
