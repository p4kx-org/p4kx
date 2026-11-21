// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

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
fn a_diffuse_map_is_srgb_and_untouched_and_alpha_follows_the_loader() -> Result<(), Box<dyn Error>>
{
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
