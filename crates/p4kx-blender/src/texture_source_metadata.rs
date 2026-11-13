// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DdsSourceInfo, FieldOrigins, FieldOriginsSeed, OriginReaderLimits, SourceInventory, SourceUses,
};
use p4kx_gltf::{LoadedTexture, TextureLoadError, TextureLoader};
use p4kx_texture::{DdsPngTransform, DdsTransfer};
use schemars::JsonSchema;
use serde::de::DeserializeSeed as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::texture_encoding::TextureMode;
use crate::texture_field_origins;
use crate::{PackageError, PackageResult, PackageWriteContext, TextureChannelTransform};

/// Same-read DDS source metadata and provenance for one written texture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct TextureSourceMetadata {
    /// Shared raw and normalized DDS header projection.
    pub dds: DdsSourceInfo,
    /// The mip actually written by the package loader.
    pub mip_level: u32,
    /// RGB transfer proved for the output PNG, independent of importer fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_transfer: Option<DdsTransfer>,
    /// Channel processing used to represent the DDS samples in the PNG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png_transform: Option<DdsPngTransform>,
    /// Additional channel handling selected by the material role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_transform: Option<TextureChannelTransform>,
    /// Source origins relative to this metadata object; /image refers to the encoded PNG.
    #[serde(deserialize_with = "read_field_origins")]
    pub field_origins: FieldOrigins,
    /// Read and directory inputs referenced by the immediate origins.
    #[schemars(with = "SourceInventory")]
    pub provenance: Value,
}

type Loaded = (
    LoadedTexture,
    Option<Box<TextureSourceMetadata>>,
    DdsTransfer,
);

pub(crate) fn load(
    loader: &dyn TextureLoader,
    path: &str,
    mip: u32,
    mode: TextureMode,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Result<Loaded, TextureLoadError>> {
    let Some(source) = context.texture_source else {
        return Ok(loader
            .load(path, mip)
            .map(|image| (image, None, DdsTransfer::Unknown)));
    };
    context.check()?;
    let observed = match source.load_observed(path, mip) {
        Ok(observed) => observed,
        Err(error) => return Ok(Err(error)),
    };
    context.check()?;
    let uses = SourceUses::with_compact_use_refs();
    let evidence = observed.decoded.evidence(&uses).map_err(metadata_error)?;
    let dds = observed.metadata;
    let decoded = observed.decoded.png_encoding().map_err(metadata_error)?;
    let transfer = mode.tag(decoded.transfer);
    let encoding = mode.origin_encoding(decoded);
    let mut field_origins =
        texture_field_origins::capture(&dds, &evidence, observed.alpha_limits_level, &uses)?;
    texture_field_origins::insert(
        &mut field_origins,
        "/image",
        texture_field_origins::image_origin(&evidence, encoding, &uses)?,
    )?;
    texture_field_origins::output_fields(&mut field_origins, &evidence, encoding, &uses)?;
    let provenance =
        serde_json::to_value(uses.freeze().map_err(metadata_error)?).map_err(metadata_error)?;
    let metadata = TextureSourceMetadata {
        dds,
        mip_level: observed.level,
        output_transfer: Some(transfer),
        png_transform: Some(encoding.transform),
        channel_transform: Some(mode.channel_transform()),
        field_origins,
        provenance,
    };
    let image = LoadedTexture::new(observed.decoded.into_image(), observed.has_alpha);
    Ok(Ok((image, Some(Box::new(metadata)), decoded.transfer)))
}

pub(super) fn metadata_error<E: std::error::Error + Send + Sync + 'static>(
    error: E,
) -> PackageError {
    PackageError::TextureEvidence(Box::new(error))
}

pub(crate) fn read_field_origins<'de, D: serde::Deserializer<'de>>(
    source: D,
) -> Result<FieldOrigins, D::Error> {
    // One fixed DDS header, one selected mip, and two possible physical texture branches.
    FieldOriginsSeed::new(OriginReaderLimits {
        fields: 512,
        metadata_bytes: 1 << 20,
        path_bytes: 256,
        field_uses: 128,
        reference_bytes: 64,
    })
    .deserialize(source)
}
