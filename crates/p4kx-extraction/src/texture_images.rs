// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::io::Write;

use p4kx_archive::Checksum;
use p4kx_texture::{
    DdsAlphaTransform, DdsPngEncoding, LocatedMip, MipPayloadLocation, TextureError,
    read_mip_with_location, write_exr, write_png_with_transfer,
};

use crate::buffers::Buffers;
use crate::texture_artifacts::TextureImageTransfer;
use crate::texture_convert::{Source, Step};
use crate::texture_outputs::step_of;
use crate::texture_preview_png::input::{PreviewInputs, TexturePreviewInput};
use crate::texture_siblings::Replay;
use crate::write::{Chunk, Destination, EntryWriter, Outcome, write_error};
use crate::{
    ConflictPolicy, CubemapLayout, ExtractionError, OriginArtifactDigest, OriginDigestWriter,
    SourceUses, Texture, TextureArtifacts, TextureImage, TextureImageArtifact, TextureImageFormat,
    TextureImagePlan,
};

/// The extensions of the image files a texture is exported to.
pub(crate) const IMAGE_EXTENSIONS: [&str; 2] = ["png", "exr"];

/// The decoded images of one mip, ready to encode.
enum Images {
    /// 8-bit RGBA with the alpha tail merged in, written as PNG.
    Png(
        Vec<p4kx_texture::RgbaImage>,
        DdsPngEncoding,
        DdsAlphaTransform,
    ),
    /// Linear floating-point RGB of a high-range texture, written as OpenEXR.
    Exr(Vec<p4kx_texture::HdrImage>),
}

impl Images {
    const fn len(&self) -> usize {
        match self {
            Self::Png(images, _, _) => images.len(),
            Self::Exr(images) => images.len(),
        }
    }

    const fn png_encoding(&self) -> Option<DdsPngEncoding> {
        match self {
            Self::Png(_, encoding, _) => Some(*encoding),
            Self::Exr(_) => None,
        }
    }

    const fn alpha_transform(&self) -> DdsAlphaTransform {
        match self {
            Self::Png(_, _, transform) => *transform,
            Self::Exr(_) => DdsAlphaTransform::Preserve,
        }
    }

    fn encode(&self, index: usize, out: &mut Vec<u8>) -> Result<(), TextureError> {
        let missing = || TextureError::UnsupportedImage { images: 0 };
        match self {
            Self::Png(images, encoding, _) => write_png_with_transfer(
                images.get(index).ok_or_else(missing)?,
                encoding.transfer,
                out,
            ),
            Self::Exr(images) => write_exr(images.get(index).ok_or_else(missing)?, out),
        }
    }
}

enum ImageWrite {
    Written(OriginArtifactDigest),
    Kept,
    Cancelled,
}

impl EntryWriter<'_> {
    /// Publishes the selected mip's images and records the inputs of each committed image.
    pub(crate) fn write_images(
        &self,
        source: &Source<'_>,
        header: &Destination,
        level: u32,
        buffers: &mut Buffers,
        artifacts: &mut Option<TextureArtifacts>,
    ) -> Result<Step, ExtractionError> {
        let dds = source.texture.header();
        let format = TextureImageFormat::for_header(dds);
        let destinations = TextureImagePlan::new(dds, level, CubemapLayout::Faces, format)
            .map_err(|error| source.texture_error(error))?
            .map(|image| {
                let name = image_name(header, &image.suffix, image.format.extension());
                let destination = Destination::new(header.folder.clone(), &name)?;
                Ok((image, destination))
            })
            .collect::<Result<Vec<_>, ExtractionError>>()?;
        let skip = self.policy == ConflictPolicy::Skip;
        if skip && destinations.iter().all(|(_, file)| file.existing.is_some()) {
            return Ok(Step::Kept);
        }
        if self.tracker.should_stop() {
            return Ok(Step::Cancelled);
        }
        let uses = SourceUses::with_compact_use_refs();
        let mut written = Vec::new();
        let (images, inputs) = Self::decode_level(source, level, &buffers.data)?;
        let transfer = TextureImageTransfer::capture(
            &inputs,
            inputs.source_transfer(),
            images.png_encoding(),
            images.alpha_transform(),
            &uses,
        )
        .map_err(|error| write_error(header.file.clone(), std::io::Error::other(error)))?;
        let step = self.write_image_set(
            source,
            level,
            &destinations,
            (&images, &inputs),
            buffers,
            (&uses, &mut written),
        );
        if !written.is_empty() {
            *artifacts = Some(transfer.finish(written, uses.freeze()?));
        }
        step
    }

    fn write_image_set(
        &self,
        source: &Source<'_>,
        level: u32,
        destinations: &[(TextureImage, Destination)],
        (images, inputs): (&Images, &PreviewInputs<'_, MipPayloadLocation>),
        buffers: &mut Buffers,
        (uses, written): (&SourceUses, &mut Vec<TextureImageArtifact>),
    ) -> Result<Step, ExtractionError> {
        let unsupported = || {
            source.texture_error(TextureError::UnsupportedImage {
                images: source.texture.header().images(level),
            })
        };
        if images.len() != destinations.len() {
            return Err(unsupported());
        }
        for (index, (image, destination)) in destinations.iter().enumerate() {
            if self.policy == ConflictPolicy::Skip && destination.existing.is_some() {
                continue;
            }
            let path = destination
                .file
                .strip_prefix(self.target)
                .map_err(|_| ExtractionError::UnsafePath)?
                .to_str()
                .ok_or(ExtractionError::UnsafePath)?
                .replace('\\', "/");
            let evidence = inputs
                .register_image(uses, level, index as u64)?
                .ok_or_else(unsupported)?;
            let mut contributions = Vec::new();
            evidence.append_pixel_uses(&mut contributions)?;
            if let Some(encoding) = images.png_encoding() {
                evidence.append_output_transfer_uses(&mut contributions, encoding)?;
                evidence
                    .append_alpha_transform_uses(&mut contributions, images.alpha_transform())?;
            }
            let origin = uses.origin(&contributions, evidence.is_complete());
            match self.write_image_file(source, (images, index), destination, buffers)? {
                ImageWrite::Written(digest) => written.push(TextureImageArtifact {
                    path,
                    digest,
                    level,
                    layer: image.layer,
                    face: image.face,
                    slice: image.slice,
                    origin,
                }),
                ImageWrite::Kept => {}
                ImageWrite::Cancelled => return Ok(Step::Cancelled),
            }
        }
        Ok(if written.is_empty() {
            Step::Kept
        } else {
            Step::Written
        })
    }

    fn write_image_file(
        &self,
        source: &Source<'_>,
        (images, index): (&Images, usize),
        destination: &Destination,
        buffers: &mut Buffers,
    ) -> Result<ImageWrite, ExtractionError> {
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
            return Ok(if matches!(outcome, Outcome::Cancelled) {
                ImageWrite::Cancelled
            } else {
                ImageWrite::Kept
            });
        }
        if self.tracker.should_stop() {
            return Ok(ImageWrite::Cancelled);
        }
        let (temporary, mut file, ownership) = self.temporary(&destination.folder)?;
        let mut sink = OriginDigestWriter::new(&mut file);
        let digest = sink.write_all(encoded).and_then(|()| sink.finish());
        drop(file);
        let digest = match digest {
            Ok(digest) => digest,
            Err(error) => {
                return crate::output_cleanup::extraction(
                    Err(write_error(destination.file.clone(), error)),
                    &ownership,
                    &temporary,
                    self.cancel,
                );
            }
        };
        let placed = self.put_in_place(
            &temporary,
            &ownership,
            &destination.file,
            Ok(Chunk::End),
            Outcome::Converted,
        )?;
        Ok(match step_of(&placed) {
            Step::Written => ImageWrite::Written(digest),
            Step::Kept => ImageWrite::Kept,
            Step::Cancelled => ImageWrite::Cancelled,
        })
    }

    /// Decodes every image of mip `level`, replaying the part in `captured` when it holds exactly
    /// that mip: a high-range texture to floating-point RGB, any other to RGBA with the alpha
    /// tail merged in.
    fn decode_level<'a>(
        source: &Source<'a>,
        level: u32,
        captured: &[u8],
    ) -> Result<(Images, PreviewInputs<'a, MipPayloadLocation>), ExtractionError> {
        let texture = &source.texture;
        let hdr = texture.header().format().is_high_range();
        let alpha = if hdr {
            None
        } else {
            texture
                .checked_alpha_tail(level)
                .map_err(ExtractionError::Texture)?
        };
        let read = Self::read_level(source, level, captured)?;
        let color = TexturePreviewInput::of_read(texture.parts(), read.raw_header, read.payload);
        if hdr {
            let images = read
                .mip
                .decode_surfaces_hdr()
                .map_err(|error| source.texture_error(error))?;
            return Ok((Images::Exr(images), PreviewInputs { color, alpha: None }));
        }
        let mut images = texture
            .decode_mip_data(&read.mip)
            .map_err(ExtractionError::Texture)?;
        let encoding = DdsPngEncoding::of(
            read.mip.header().format(),
            read.raw_header.transfer().effective,
        );
        let transform = DdsAlphaTransform::for_png(read.raw_header.alpha_association(), encoding);
        if transform != DdsAlphaTransform::Preserve {
            for image in &mut images {
                transform.image(encoding.transfer, image);
            }
        }
        let alpha = Texture::merge_alpha_input(alpha, level, &mut images)
            .map_err(ExtractionError::Texture)?;
        Ok((
            Images::Png(images, encoding, transform),
            PreviewInputs { color, alpha },
        ))
    }

    fn read_level(
        source: &Source<'_>,
        level: u32,
        captured: &[u8],
    ) -> Result<LocatedMip, ExtractionError> {
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
        read_mip_with_location(header, texture.header_size(), &siblings, level)
            .map_err(|error| source.texture_error(error))
    }
}

/// Returns the name of an image next to the header: its name with `extension` for `.dds`, and
/// `suffix` before it for one of several images.
fn image_name(header: &Destination, suffix: &str, extension: &str) -> OsString {
    let mut name = header.file.file_stem().map_or_default(OsString::from);
    name.push(suffix);
    name.push(".");
    name.push(extension);
    name
}
