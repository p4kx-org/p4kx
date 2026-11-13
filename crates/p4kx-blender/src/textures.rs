// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::manifest::TEXTURES_FOLDER;
use crate::package_receipt::PackageFile;
use crate::texture_ddna::SmoothnessWritten;
use crate::texture_dds::{self, SourceFallback};
use crate::texture_encoding::TextureMode;
use crate::texture_source_metadata;
use p4kx_extraction::texture_preview::{ModelDdsRefusal, ObservedModelDds};
use p4kx_extraction::{OriginArtifactDigest, OriginDigestWriter};
use p4kx_gltf::TextureLoader;
use p4kx_texture::{DdsTransfer, RgbaImage, TextureError, write_png_with_transfer};
use rayon::prelude::*;

use crate::{
    AlphaMeaning, ColourSpace, DdsTextureAsset, DdsTextureFallback, PackageError, PackageResult,
    PackageTextureContainer, PackageWriteContext, TextureEncoding, TextureFormat,
};

#[path = "texture_output_ddna.rs"]
mod ddna;

fn source_disposition(error: &ModelDdsRefusal) -> Result<Outcome, DdsTextureFallback> {
    match DdsTextureFallback::source_error(error) {
        SourceFallback::Png(reason) => Err(reason),
        SourceFallback::Refused(reason) => {
            Ok(TextureOutput::failed(error.to_string(), Some(reason)))
        }
    }
}

/// How many textures are decoded and encoded at once; a texture can hold 64 megapixels.
const BATCH: usize = 4;

/// A texture file some material names.
#[derive(Debug, Clone)]
pub(crate) struct Wanted {
    pub(crate) path: String,
    pub(crate) file: String,
    pub(crate) material: String,
    pub(crate) mode: TextureMode,
}

/// What became of one wanted texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Written {
        width: u32,
        height: u32,
        artifact: OriginArtifactDigest,
        colour_space: ColourSpace,
        encoding: TextureEncoding,
        alpha: AlphaMeaning,
        source_metadata: Option<Box<crate::TextureSourceMetadata>>,
        format: TextureFormat,
        dds_asset: Option<Box<DdsTextureAsset>>,
        dds_fallback: Option<DdsTextureFallback>,
        smoothness: Option<Box<SmoothnessWritten>>,
    },
    Failed {
        reason: String,
        dds: Option<DdsTextureFallback>,
    },
}

pub(crate) fn package_files(
    position: usize,
    outcome: &Outcome,
) -> impl Iterator<Item = PackageFile> {
    let Outcome::Written {
        artifact,
        format,
        smoothness,
        ..
    } = outcome
    else {
        return [None, None].into_iter().flatten();
    };
    let copy = |id: &str, format: TextureFormat, digest: &OriginArtifactDigest| {
        PackageFile::new(
            format!("{TEXTURES_FOLDER}/{id}.{}", format.extension()),
            digest,
        )
    };
    [
        Some(copy(&id_of(position), *format, artifact)),
        smoothness
            .as_deref()
            .map(|value| copy(&value.id, TextureFormat::Dds, &value.asset.artifact)),
    ]
    .into_iter()
    .flatten()
}

/// Returns the file name of the texture at position `index` of the wanted ones.
pub(crate) fn id_of(index: usize) -> String {
    format!("t{:04}", index + 1)
}

/// Writes one PNG or supported DDS file for each successfully loaded texture.
///
/// A texture that cannot be loaded or encoded is a [`Outcome::Failed`] and leaves no file.
pub(crate) fn write_all(
    wanted: &[Wanted],
    loader: &dyn TextureLoader,
    mip: u32,
    folder: &Path,
    cancelled: &PackageWriteContext<'_>,
) -> PackageResult<Vec<Outcome>> {
    let mut outcomes = Vec::with_capacity(wanted.len());
    for (batch, chunk) in wanted.chunks(BATCH).enumerate() {
        cancelled.check()?;
        let done: Vec<Outcome> = chunk
            .par_iter()
            .enumerate()
            .map(|(offset, texture)| {
                cancelled.check()?;
                write_one(
                    texture,
                    batch * BATCH + offset,
                    wanted.len(),
                    loader,
                    mip,
                    folder,
                    cancelled,
                )
            })
            .collect::<PackageResult<_>>()?;
        outcomes.extend(done);
    }
    Ok(outcomes)
}

fn write_one(
    texture: &Wanted,
    position: usize,
    primary_count: usize,
    loader: &dyn TextureLoader,
    mip: u32,
    folder: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Outcome> {
    let output = TextureOutput {
        texture,
        id: &id_of(position),
        smoothness_id: &id_of(primary_count + position),
        folder,
        context,
    };
    let fallback = match context.texture_container {
        PackageTextureContainer::Png => None,
        PackageTextureContainer::BlenderDds => match output.direct_dds(mip)? {
            Ok(written) => return Ok(written),
            Err(reason) => Some(reason),
        },
    };
    output.png(loader, mip, fallback)
}

struct TextureOutput<'a, 'source> {
    texture: &'a Wanted,
    id: &'a str,
    smoothness_id: &'a str,
    folder: &'a Path,
    context: &'a PackageWriteContext<'source>,
}

impl TextureOutput<'_, '_> {
    fn direct_dds(&self, mip: u32) -> PackageResult<Result<Outcome, DdsTextureFallback>> {
        let Some(source) = self.context.texture_source else {
            return Ok(Err(DdsTextureFallback::UnavailableSource));
        };
        self.context.check()?;
        let observed = match source.load_dds(&self.texture.path, mip) {
            Ok(observed) => observed,
            Err(ModelDdsRefusal::SplitAlpha)
                if self.texture.mode == (TextureMode::Normal { gloss: true }) =>
            {
                let observed = match source.load_ddna_dds(&self.texture.path, mip) {
                    Ok(observed) => observed,
                    Err(error) => return Ok(source_disposition(&error)),
                };
                self.context.check()?;
                return self.write_ddna_pair(observed);
            }
            Err(error) => return Ok(source_disposition(&error)),
        };
        self.context.check()?;
        let mode = self.texture.mode;
        if let Some(reason) = texture_dds::unsupported(&observed.read, mode) {
            return Ok(Err(reason));
        }
        self.write_dds(observed).map(Ok)
    }

    fn write_dds(&self, observed: ObservedModelDds<'_>) -> PackageResult<Outcome> {
        let mode = self.texture.mode;
        let width = observed.read.mip.width();
        let height = observed.read.mip.height();
        let transfer = observed
            .metadata
            .transfer
            .map_or(DdsTransfer::Unknown, |source| source.effective);
        let coverage = observed.read.mip.alpha_coverage(0);
        let has_alpha = if mode.uses_colour_samples() {
            texture_dds::alpha_has_samples(&observed, coverage)
        } else {
            observed.has_alpha
        };
        let alpha = Self::alpha(
            mode.uses_alpha(
                has_alpha,
                observed.read.raw_header.alpha_association(),
                false,
            ),
            mode,
        );
        let written = self.write(TextureFormat::Dds, |writer| observed.read.write_dds(writer))?;
        let artifact = match written {
            Ok(written) => written,
            Err(error) => {
                return Ok(Self::failed(
                    error.to_string(),
                    Some(DdsTextureFallback::InvalidInput),
                ));
            }
        };
        let dds_asset = DdsTextureAsset::bind(observed, artifact.clone(), coverage)?;
        Ok(Outcome::Written {
            width,
            height,
            artifact,
            colour_space: mode.legacy_space(),
            encoding: mode.encoding(transfer),
            alpha,
            source_metadata: None,
            format: TextureFormat::Dds,
            dds_asset: Some(Box::new(dds_asset)),
            dds_fallback: None,
            smoothness: None,
        })
    }

    fn png(
        &self,
        loader: &dyn TextureLoader,
        mip: u32,
        fallback: Option<DdsTextureFallback>,
    ) -> PackageResult<Outcome> {
        let mode = self.texture.mode;
        let (decoded, source_metadata, transfer) = match texture_source_metadata::load(
            loader,
            &self.texture.path,
            mip,
            mode,
            self.context,
        )? {
            Ok(decoded) => decoded,
            Err(error) => return Ok(Self::failed(error.to_string(), fallback)),
        };
        let alpha = Self::alpha(decoded.has_alpha, mode);
        let image = if matches!(mode, TextureMode::Normal { .. }) {
            match flip_green(decoded.image) {
                Ok(image) => image,
                Err(error) => return Ok(Self::failed(error.to_string(), fallback)),
            }
        } else {
            decoded.image
        };
        let written = self.write(TextureFormat::Png, |writer| {
            write_png_with_transfer(&image, mode.tag(transfer), writer)
        })?;
        let artifact = match written {
            Ok(artifact) => artifact,
            Err(error) => return Ok(Self::failed(error.to_string(), fallback)),
        };
        Ok(Outcome::Written {
            width: image.width(),
            height: image.height(),
            artifact,
            colour_space: mode.legacy_space(),
            encoding: mode.encoding(transfer),
            alpha,
            source_metadata,
            format: TextureFormat::Png,
            dds_asset: None,
            dds_fallback: fallback,
            smoothness: None,
        })
    }

    fn write(
        &self,
        format: TextureFormat,
        encode: impl FnOnce(&mut dyn Write) -> Result<(), TextureError>,
    ) -> PackageResult<Result<OriginArtifactDigest, TextureError>> {
        let path = self
            .folder
            .join(format!("{}.{}", self.id, format.extension()));
        let (file, ownership) = self.context.create_file(&path)?.into_parts();
        let mut sink = BufWriter::new(file);
        let mut writer = OriginDigestWriter::new(&mut sink);
        match encode(&mut writer) {
            Ok(()) => {}
            Err(TextureError::Write(source)) => return Err(PackageError::write(&path, source)),
            Err(error) => {
                drop(sink);
                self.context.cleanup_file(&ownership, &path)?;
                return Ok(Err(error));
            }
        }
        let artifact = writer
            .finish()
            .map_err(|source| PackageError::write(&path, source))?;
        ownership.ensure_writable(&path, self.context.cancel_flag())?;
        sink.get_ref()
            .sync_all()
            .map_err(|source| PackageError::write(&path, source))?;
        Ok(Ok(artifact))
    }

    const fn alpha(has_samples: bool, mode: TextureMode) -> AlphaMeaning {
        match (has_samples, mode) {
            (false, _) => AlphaMeaning::None,
            (true, TextureMode::Normal { gloss: true }) => AlphaMeaning::Smoothness,
            (true, _) => AlphaMeaning::Opacity,
        }
    }

    const fn failed(reason: String, dds: Option<DdsTextureFallback>) -> Outcome {
        Outcome::Failed { reason, dds }
    }
}

/// Returns `image` with the green channel inverted: the engine's normal maps have Y pointing down
/// the image, glTF's and OpenGL's up.
fn flip_green(image: RgbaImage) -> Result<RgbaImage, TextureError> {
    let (width, height) = (image.width(), image.height());
    let mut pixels = image.into_pixels();
    for [_, green, _, _] in pixels.as_chunks_mut::<4>().0 {
        *green = u8::MAX - *green;
    }
    RgbaImage::new(width, height, pixels)
}

#[cfg(test)]
#[path = "textures_tests.rs"]
mod tests;
