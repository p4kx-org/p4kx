// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::io::Write;

use p4kx_archive::Checksum;
use p4kx_texture::{Mip, TextureError, read_mip, write_exr, write_png};

use crate::buffers::Buffers;
use crate::texture_convert::{Source, Step};
use crate::texture_names::image_suffixes;
use crate::texture_outputs::step_of;
use crate::texture_siblings::Replay;
use crate::write::{Chunk, Destination, EntryWriter, Outcome, write_error};
use crate::{ConflictPolicy, ExtractionError, Texture};

/// The extensions of the image files a texture is exported to.
pub(crate) const IMAGE_EXTENSIONS: [&str; 2] = ["png", "exr"];

/// The decoded images of one mip, ready to encode.
enum Images {
    /// 8-bit RGBA with the alpha tail merged in, written as PNG.
    Png(Vec<p4kx_texture::RgbaImage>),
    /// Linear floating-point RGB of a high-range texture, written as OpenEXR.
    Exr(Vec<p4kx_texture::HdrImage>),
}

impl Images {
    const fn len(&self) -> usize {
        match self {
            Self::Png(images) => images.len(),
            Self::Exr(images) => images.len(),
        }
    }

    fn encode(&self, index: usize, out: &mut Vec<u8>) -> Result<(), TextureError> {
        let missing = || TextureError::UnsupportedImage { images: 0 };
        match self {
            Self::Png(images) => write_png(images.get(index).ok_or_else(missing)?, out),
            Self::Exr(images) => write_exr(images.get(index).ok_or_else(missing)?, out),
        }
    }
}

impl EntryWriter<'_> {
    /// Decodes mip `level` of `source`, its part taken from `buffers.data` when the merge kept it
    /// there, and writes it next to `header` under the Conflict policy: as `<name>.png` with the
    /// alpha tail in its alpha channel, or for a high-range texture as OpenEXR `<name>.exr`, the
    /// range kept; a cubemap's six faces as `<name>_posx` … `<name>_negz`, an array's elements as
    /// `<name>_layerN` and a volume's slices as `<name>_sliceN`.
    pub(crate) fn write_images(
        &self,
        source: &Source<'_>,
        header: &Destination,
        level: u32,
        buffers: &mut Buffers,
    ) -> Result<Step, ExtractionError> {
        let dds = source.texture.header();
        let extension = if dds.format().is_high_range() {
            "exr"
        } else {
            "png"
        };
        let names: Vec<OsString> = image_suffixes(dds, level, false)
            .iter()
            .map(|suffix| image_name(header, suffix, extension))
            .collect();
        let destinations = names
            .iter()
            .map(|name| Destination::new(header.folder.clone(), name))
            .collect::<Result<Vec<_>, _>>()?;
        let skip = self.policy == ConflictPolicy::Skip;
        if skip && destinations.iter().all(|file| file.existing.is_some()) {
            return Ok(Step::Kept);
        }
        if self.tracker.should_stop() {
            return Ok(Step::Cancelled);
        }
        let images = Self::decode_level(source, level, &buffers.data)?;
        if images.len() != destinations.len() {
            return Err(source.texture_error(TextureError::UnsupportedImage {
                images: dds.images(level),
            }));
        }
        let mut written = false;
        for (index, destination) in destinations.iter().enumerate() {
            let step = if skip && destination.existing.is_some() {
                Step::Kept
            } else {
                self.write_image_file(source, (&images, index), destination, buffers)?
            };
            if step == Step::Cancelled {
                return Ok(step);
            }
            written |= step == Step::Written;
        }
        Ok(if written { Step::Written } else { Step::Kept })
    }

    fn write_image_file(
        &self,
        source: &Source<'_>,
        (images, index): (&Images, usize),
        destination: &Destination,
        buffers: &mut Buffers,
    ) -> Result<Step, ExtractionError> {
        let encoded = &mut buffers.output;
        encoded.clear();
        images
            .encode(index, encoded)
            .map_err(|error| source.texture_error(error))?;
        let expected = || {
            (
                encoded.len() as u64,
                Checksum::Crc32c(crc32c::crc32c(encoded)),
            )
        };
        if let Some(outcome) = self.decide(destination, expected, &mut buffers.chunk)? {
            return Ok(step_of(&outcome));
        }
        if self.tracker.should_stop() {
            return Ok(Step::Cancelled);
        }
        let (temporary, mut file, ownership) = self.temporary(&destination.folder)?;
        let copied = file
            .write_all(encoded)
            .map(|()| Chunk::End)
            .map_err(|error| write_error(destination.file.clone(), error));
        drop(file);
        let placed = self.put_in_place(
            &temporary,
            &ownership,
            &destination.file,
            copied,
            Outcome::Converted,
        )?;
        Ok(step_of(&placed))
    }

    /// Decodes every image of mip `level`, replaying the part in `captured` when it holds exactly
    /// that mip: a high-range texture to floating-point RGB, any other to RGBA with the alpha
    /// tail merged in.
    fn decode_level(
        source: &Source<'_>,
        level: u32,
        captured: &[u8],
    ) -> Result<Images, ExtractionError> {
        let texture = &source.texture;
        if texture.header().format().is_high_range() {
            let mip = Self::read_level(source, level, captured)?;
            return mip
                .decode_surfaces_hdr()
                .map(Images::Exr)
                .map_err(|error| source.texture_error(error));
        }
        let alpha = texture
            .checked_alpha_tail(level)
            .map_err(ExtractionError::Texture)?;
        let mip = Self::read_level(source, level, captured)?;
        let images = texture
            .decode_mip_data(&mip)
            .map_err(ExtractionError::Texture)?;
        Texture::with_alpha_tail(alpha, level, images)
            .map(Images::Png)
            .map_err(ExtractionError::Texture)
    }

    fn read_level(
        source: &Source<'_>,
        level: u32,
        captured: &[u8],
    ) -> Result<Mip, ExtractionError> {
        let texture = &source.texture;
        let replayed = (level < source.part_count
            && !captured.is_empty()
            && texture.header().mip_size(level) == Some(captured.len() as u64))
        .then(|| (source.part_count - level, captured));
        let siblings = Replay {
            parts: texture.parts(),
            captured: replayed,
        };
        let header = texture.open_header().map_err(ExtractionError::Texture)?;
        read_mip(header, texture.header_size(), &siblings, level)
            .map_err(|error| source.texture_error(error))
    }
}

/// Returns the name of an image next to the header: its name with `extension` for `.dds`, and
/// `suffix` before it for one of several images.
fn image_name(header: &Destination, suffix: &str, extension: &str) -> OsString {
    let mut name = header
        .file
        .file_stem()
        .map(OsString::from)
        .unwrap_or_default();
    name.push(suffix);
    name.push(".");
    name.push(extension);
    name
}
