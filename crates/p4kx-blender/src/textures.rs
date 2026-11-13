// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::texture_source_metadata;
use p4kx_gltf::TextureLoader;
use p4kx_texture::{RgbaImage, TextureError, write_png};
use rayon::prelude::*;

use crate::{
    AlphaMeaning, ColourSpace, PackageError, PackageResult, PackageWriteContext, TextureUse,
};

/// How many textures are decoded and encoded at once; a texture can hold 64 megapixels.
const BATCH: usize = 4;

/// A texture file some material names.
#[derive(Debug, Clone)]
pub(crate) struct Wanted {
    pub(crate) path: String,
    pub(crate) file: String,
    pub(crate) material: String,
    pub(crate) role: TextureUse,
    pub(crate) gloss: bool,
}

/// What became of one wanted texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Written {
        width: u32,
        height: u32,
        bytes: u64,
        colour_space: ColourSpace,
        alpha: AlphaMeaning,
        source_metadata: Option<Box<crate::TextureSourceMetadata>>,
    },
    Failed(String),
}

/// Returns the file name of the texture at position `index` of the wanted ones.
pub(crate) fn id_of(index: usize) -> String {
    format!("t{:04}", index + 1)
}

/// Loads every wanted texture at `mip` and writes it as `<folder>/<id>.png`.
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
    let (decoded, source_metadata) =
        match texture_source_metadata::load(loader, &texture.path, mip, context)? {
            Ok(decoded) => decoded,
            Err(error) => return Ok(Outcome::Failed(error.to_string())),
        };
    let alpha = match (decoded.has_alpha, texture.role, texture.gloss) {
        (false, _, _) => AlphaMeaning::None,
        (true, TextureUse::Normal, true) => AlphaMeaning::Smoothness,
        (true, _, _) => AlphaMeaning::Opacity,
    };
    let image = if texture.role == TextureUse::Normal {
        match flip_green(decoded.image) {
            Ok(image) => image,
            Err(error) => return Ok(Outcome::Failed(error.to_string())),
        }
    } else {
        decoded.image
    };
    let path = folder.join(format!("{id}.png"));
    let (file, ownership) = context.create_file(&path)?.into_parts();
    let mut writer = BufWriter::new(file);
    let written = write_png(&image, &mut writer);
    match written {
        Ok(()) => {}
        Err(TextureError::Write(source)) => return Err(PackageError::write(&path, source)),
        Err(error) => {
            drop(writer);
            context.cleanup_file(&ownership, &path)?;
            return Ok(Outcome::Failed(error.to_string()));
        }
    }
    writer
        .flush()
        .map_err(|source| PackageError::write(&path, source))?;
    ownership.ensure_writable(&path, context.cancel_flag())?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|source| PackageError::write(&path, source))?;
    let bytes = writer
        .get_ref()
        .metadata()
        .map_err(|source| PackageError::write(&path, source))?
        .len();
    Ok(Outcome::Written {
        width: image.width(),
        height: image.height(),
        bytes,
        colour_space: if texture.role == TextureUse::Diffuse {
            ColourSpace::Srgb
        } else {
            ColourSpace::Linear
        },
        alpha,
        source_metadata,
    })
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
            role,
            gloss,
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
