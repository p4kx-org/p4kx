// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::texture_dds::{self, SourceFallback};
use crate::texture_encoding::TextureMode;
use crate::texture_source_metadata;
use p4kx_extraction::OriginDigestWriter;
use p4kx_gltf::TextureLoader;
use p4kx_texture::{DdsTransfer, RgbaImage, TextureError, write_png_with_transfer};
use rayon::prelude::*;

use crate::{
    AlphaMeaning, ColourSpace, DdsTextureAsset, DdsTextureFallback, PackageError, PackageResult,
    PackageTextureContainer, PackageWriteContext, TextureEncoding, TextureFormat,
};

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
        bytes: u64,
        colour_space: ColourSpace,
        encoding: TextureEncoding,
        alpha: AlphaMeaning,
        source_metadata: Option<Box<crate::TextureSourceMetadata>>,
        format: TextureFormat,
        dds_asset: Option<Box<DdsTextureAsset>>,
        dds_fallback: Option<DdsTextureFallback>,
    },
    Failed {
        reason: String,
        dds: Option<DdsTextureFallback>,
    },
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
                    &id_of(batch * BATCH + offset),
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
    id: &str,
    loader: &dyn TextureLoader,
    mip: u32,
    folder: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Outcome> {
    let output = TextureOutput {
        texture,
        id,
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
            Err(error) => {
                return Ok(match DdsTextureFallback::source_error(&error) {
                    SourceFallback::Png(reason) => Err(reason),
                    SourceFallback::Refused(reason) => {
                        Ok(Self::failed(error.to_string(), Some(reason)))
                    }
                });
            }
        };
        self.context.check()?;
        let mode = self.texture.mode;
        if let Some(reason) = texture_dds::unsupported(&observed.read, mode) {
            return Ok(Err(reason));
        }
        let width = observed.read.mip.width();
        let height = observed.read.mip.height();
        let transfer = observed
            .metadata
            .transfer
            .map_or(DdsTransfer::Unknown, |source| source.effective);
        let alpha = Self::alpha(texture_dds::alpha_has_samples(&observed), mode);
        let written = self.write(TextureFormat::Dds, |writer| {
            let mut measured = OriginDigestWriter::new(writer);
            observed.read.write_dds(&mut measured)?;
            measured.finish().map_err(TextureError::Write)
        })?;
        let (artifact, bytes) = match written {
            Ok(written) => written,
            Err(error) => {
                return Ok(Ok(Self::failed(
                    error.to_string(),
                    Some(DdsTextureFallback::InvalidInput),
                )));
            }
        };
        let dds_asset = DdsTextureAsset::bind(observed, artifact)?;
        Ok(Ok(Outcome::Written {
            width,
            height,
            bytes,
            colour_space: mode.legacy_space(),
            encoding: mode.encoding(transfer),
            alpha,
            source_metadata: None,
            format: TextureFormat::Dds,
            dds_asset: Some(Box::new(dds_asset)),
            dds_fallback: None,
        }))
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
        let bytes = match written {
            Ok(((), bytes)) => bytes,
            Err(error) => return Ok(Self::failed(error.to_string(), fallback)),
        };
        Ok(Outcome::Written {
            width: image.width(),
            height: image.height(),
            bytes,
            colour_space: mode.legacy_space(),
            encoding: mode.encoding(transfer),
            alpha,
            source_metadata,
            format: TextureFormat::Png,
            dds_asset: None,
            dds_fallback: fallback,
        })
    }

    fn write<T>(
        &self,
        format: TextureFormat,
        encode: impl FnOnce(&mut dyn Write) -> Result<T, TextureError>,
    ) -> PackageResult<Result<(T, u64), TextureError>> {
        let path = self
            .folder
            .join(format!("{}.{}", self.id, format.extension()));
        let (file, ownership) = self.context.create_file(&path)?.into_parts();
        let mut writer = BufWriter::new(file);
        let encoded = match encode(&mut writer) {
            Ok(encoded) => encoded,
            Err(TextureError::Write(source)) => return Err(PackageError::write(&path, source)),
            Err(error) => {
                drop(writer);
                self.context.cleanup_file(&ownership, &path)?;
                return Ok(Err(error));
            }
        };
        writer
            .flush()
            .map_err(|source| PackageError::write(&path, source))?;
        ownership.ensure_writable(&path, self.context.cancel_flag())?;
        let file = writer.get_ref();
        file.sync_all()
            .map_err(|source| PackageError::write(&path, source))?;
        let bytes = file
            .metadata()
            .map_err(|source| PackageError::write(&path, source))?
            .len();
        Ok(Ok((encoded, bytes)))
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
mod tests {
    use std::error::Error;
    use std::sync::atomic::AtomicBool;

    use p4kx_gltf::{LoadedTexture, TextureLoadError, TextureLoader};
    use p4kx_texture::RgbaImage;

    use super::{Outcome, Wanted, write_all};
    use crate::texture_encoding::TextureMode;
    use crate::{AlphaMeaning, ColourSpace, PackageWriteContext, TextureUse};

    struct One(LoadedTexture);

    impl TextureLoader for One {
        fn load(&self, _path: &str, _mip: u32) -> Result<LoadedTexture, TextureLoadError> {
            Ok(self.0.clone())
        }
    }

    fn wanted(role: TextureUse, gloss: bool) -> Wanted {
        Wanted {
            path: "a/b.dds".to_owned(),
            file: "a/b.tif".to_owned(),
            material: "m".to_owned(),
            mode: TextureMode::of(role, gloss),
        }
    }

    fn written(
        role: TextureUse,
        gloss: bool,
        has_alpha: bool,
    ) -> Result<(Outcome, Vec<u8>), Box<dyn Error>> {
        let image = RgbaImage::new(2, 1, vec![10, 20, 30, 40, 250, 255, 0, 255])?;
        let dir = tempfile::tempdir()?;
        let never = || false;
        let observed = AtomicBool::new(false);
        let outcomes = write_all(
            &[wanted(role, gloss)],
            &One(LoadedTexture::new(image, has_alpha)),
            0,
            dir.path(),
            &PackageWriteContext::standalone(&never, &observed),
        )?;
        let decoder = png::Decoder::new(std::io::Cursor::new(std::fs::read(
            dir.path().join("t0001.png"),
        )?));
        let mut reader = decoder.read_info()?;
        let mut pixels = vec![0; reader.output_buffer_size().ok_or("size")?];
        reader.next_frame(&mut pixels)?;
        Ok((outcomes.into_iter().next().ok_or("outcome")?, pixels))
    }

    #[test]
    fn a_normal_map_has_its_green_flipped_and_keeps_the_smoothness_in_alpha()
    -> Result<(), Box<dyn Error>> {
        let (outcome, pixels) = written(TextureUse::Normal, true, true)?;

        assert_eq!(pixels, [10, 235, 30, 40, 250, 0, 0, 255]);
        assert!(matches!(
            outcome,
            Outcome::Written {
                colour_space: ColourSpace::Linear,
                alpha: AlphaMeaning::Smoothness,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn a_diffuse_map_is_srgb_and_untouched_and_alpha_follows_the_loader()
    -> Result<(), Box<dyn Error>> {
        let (opaque, pixels) = written(TextureUse::Diffuse, false, false)?;
        let (translucent, _) = written(TextureUse::Diffuse, false, true)?;

        assert_eq!(pixels, [10, 20, 30, 40, 250, 255, 0, 255]);
        assert!(matches!(
            opaque,
            Outcome::Written {
                colour_space: ColourSpace::Srgb,
                alpha: AlphaMeaning::None,
                ..
            }
        ));
        assert!(matches!(
            translucent,
            Outcome::Written {
                alpha: AlphaMeaning::Opacity,
                ..
            }
        ));
        Ok(())
    }
}
