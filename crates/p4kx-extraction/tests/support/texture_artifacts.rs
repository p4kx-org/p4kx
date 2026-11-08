// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use exr::prelude::{ReadChannels, ReadLayers, Vec2};
use p4kx_extraction::{
    ConversionError, CubemapLayout, DdsHeader, ExtractionError, TextureError, TextureFormat,
    TextureImage, TextureImageFormat, TextureImagePlan,
};
use sha2::{Digest, Sha256};

pub(crate) type CheckResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) struct ExpectedImage {
    pub(crate) image: TextureImage,
    pub(crate) path: PathBuf,
}

pub(crate) fn expected(
    root: &Path,
    header_path: &str,
    header: &DdsHeader,
    level: u32,
) -> CheckResult<Vec<ExpectedImage>> {
    let format = TextureImageFormat::for_header(header);
    let output = root
        .join(header_path.replace('\\', "/"))
        .with_extension(format.extension());
    Ok(
        TextureImagePlan::new(header, level, CubemapLayout::Faces, format)?
            .map(|image| ExpectedImage {
                path: image.path(&output),
                image,
            })
            .collect(),
    )
}

pub(crate) fn check(root: &Path, expected: &[ExpectedImage]) -> CheckResult {
    let paths: BTreeSet<_> = expected.iter().map(|image| image.path.clone()).collect();
    if paths.len() != expected.len() {
        return Err(io::Error::other("planned image paths collide").into());
    }
    let actual: BTreeSet<_> = image_paths(root)?.into_iter().collect();
    if paths != actual {
        return Err(io::Error::other(format!(
            "image set differs: missing {:?}, unexpected {:?}",
            paths.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&paths).collect::<Vec<_>>()
        ))
        .into());
    }
    for image in expected {
        match image.image.format {
            TextureImageFormat::Png => check_png(image)?,
            TextureImageFormat::Exr => check_exr(image)?,
            format => return Err(io::Error::other(format!("unverified format {format:?}")).into()),
        }
    }
    Ok(())
}

fn image_paths(root: &Path) -> CheckResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                folders.push(entry.path());
            } else if entry.path().extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("png") || extension.eq_ignore_ascii_case("exr")
            }) {
                paths.push(entry.path());
            }
        }
    }
    Ok(paths)
}

fn check_png(expected: &ExpectedImage) -> CheckResult {
    let mut reader = png::Decoder::new(BufReader::new(File::open(&expected.path)?)).read_info()?;
    let info = reader.info();
    if (info.width, info.height) != (expected.image.width, expected.image.height)
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err(io::Error::other(format!(
            "wrong PNG shape or format: {}",
            expected.path.display()
        ))
        .into());
    }
    let size = reader
        .output_buffer_size()
        .ok_or("PNG buffer size overflow")?;
    let mut pixels = vec![0; size];
    reader.next_frame(&mut pixels)?;
    reader.finish()?;
    Ok(())
}

fn check_exr(expected: &ExpectedImage) -> CheckResult {
    let metadata = exr::meta::MetaData::read_from_file(&expected.path, true)?;
    let header = metadata
        .headers
        .first()
        .ok_or("OpenEXR has no image header")?;
    let channels: BTreeSet<_> = header
        .channels
        .list
        .iter()
        .map(|channel| channel.name.to_string())
        .collect();
    let rgb = BTreeSet::from(["B".to_owned(), "G".to_owned(), "R".to_owned()]);
    let rgba = BTreeSet::from([
        "A".to_owned(),
        "B".to_owned(),
        "G".to_owned(),
        "R".to_owned(),
    ]);
    if (channels != rgb && channels != rgba)
        || metadata.headers.len() != 1
        || header.layer_size
            != Vec2(
                expected.image.width as usize,
                expected.image.height as usize,
            )
        || header
            .channels
            .list
            .iter()
            .any(|channel| channel.sample_type != exr::meta::attribute::SampleType::F32)
    {
        return Err(io::Error::other(format!(
            "wrong OpenEXR shape or channel type: {}",
            expected.path.display()
        ))
        .into());
    }
    exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |resolution, _| (resolution, vec![[0.0_f32; 4]; resolution.area()]),
            |(resolution, pixels): &mut (Vec2<usize>, Vec<[f32; 4]>),
             position: Vec2<usize>,
             rgba: (f32, f32, f32, f32)| {
                if let Some(pixel) =
                    pixels.get_mut(position.y() * resolution.width() + position.x())
                {
                    *pixel = rgba.into();
                }
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_file(&expected.path)?;
    Ok(())
}

pub(crate) fn snapshot(root: &Path) -> CheckResult<BTreeMap<PathBuf, [u8; 32]>> {
    image_paths(root)?
        .into_iter()
        .map(|path| {
            let digest = Sha256::digest(fs::read(&path)?).into();
            Ok((path, digest))
        })
        .collect()
}

pub(crate) fn unsupported(error: &ExtractionError, source_format: TextureFormat) -> bool {
    let ExtractionError::Texture(ConversionError::Texture { source, .. }) = error else {
        return false;
    };
    match source {
        TextureError::UnsupportedDecode(format) | TextureError::UnsupportedFormat(format) => {
            *format == source_format && !format.is_decodable() && !format.is_high_range()
        }
        _ => false,
    }
}
