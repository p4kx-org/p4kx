// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "texture/decode.rs"]
mod decode;
#[path = "texture/disk.rs"]
mod disk;
#[path = "texture/disk_mip.rs"]
mod disk_mip;
#[path = "texture/image_receipt.rs"]
pub(crate) mod image_receipt;
#[path = "texture/merge_inputs.rs"]
mod merge_inputs;
#[path = "texture/merge_receipt.rs"]
mod merge_receipt;
#[path = "texture_preview_png/input/header.rs"]
pub(crate) mod preview_header;
#[path = "texture/texture.rs"]
mod texture;

pub(crate) use merge_inputs::{FileMergeInputs, FilePartReads};
pub(crate) use merge_receipt::DdsMergeOrigins;
pub use merge_receipt::MergeOrigins;
pub use p4kx_texture as codec;
pub use texture::Texture;

pub use crate::texture_artifacts::{TextureArtifacts, TextureImageArtifact};
pub use crate::texture_image_plan::{
    TextureImage, TextureImageFace, TextureImageFormat, TextureImagePlan,
};
pub use crate::texture_preview_png::{
    ObservedPreviewPng, PreviewPng, PreviewPngEvidence, PreviewTextureEvidence,
};
