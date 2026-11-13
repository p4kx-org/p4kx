// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{
    DdsAlphaAssociation, LocatedMip, Mip, MipByteSpans, MipPayloadLocation, RgbaImage,
    TextureError, merge_alpha, read_mip_with_location,
};

use super::disk_mip;
use super::texture::Texture;
use crate::ConversionError;
use crate::texture_parts::Parts;
use crate::texture_preview_png::input::{DiskSelection, PreviewInputs, TexturePreviewInput};

type ObservedMip<'a> = (RgbaImage, PreviewInputs<'a, MipByteSpans>, Option<Vec<u8>>);

impl<'a> Texture<'a> {
    /// Reads mip `level`, 0 being the largest, from its part or from the header.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] naming the part when the mip's part is missing, truncated,
    /// oversized or unreadable, and another one when the level does not exist.
    pub fn read_mip(&self, level: u32) -> Result<Mip, ConversionError> {
        self.read_mip_with_source(level).map(|read| read.mip)
    }

    fn read_mip_with_source(&self, level: u32) -> Result<LocatedMip, ConversionError> {
        let header = self.open_header()?;
        read_mip_with_location(header, self.header_size(), self.parts(), level)
            .map_err(|error| ConversionError::of_texture(self.name(), error))
    }

    /// Reads and decodes mip `level`, 0 being the largest, to 8-bit RGBA with the alpha tail
    /// merged into the alpha channel.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] wrapping [`TextureError::UnsupportedImage`] when the mip
    /// holds several images, and the errors of [`Texture::decode_surfaces`].
    pub fn decode_mip(&self, level: u32) -> Result<RgbaImage, ConversionError> {
        self.decode_mip_with_inputs(level).map(|(image, _)| image)
    }

    pub(crate) fn decode_mip_with_inputs(
        &self,
        level: u32,
    ) -> Result<(RgbaImage, PreviewInputs<'a, MipByteSpans>), ConversionError> {
        self.decode_mip_and_opacity(level, false)
            .map(|(image, inputs, _)| (image, inputs))
    }

    pub(crate) fn decode_mip_and_opacity(
        &self,
        level: u32,
        retain_opacity: bool,
    ) -> Result<ObservedMip<'a>, ConversionError> {
        let several = || {
            ConversionError::of_texture(
                self.name(),
                TextureError::UnsupportedImage {
                    images: self.header().images(level),
                },
            )
        };
        if self.header().images(level) > 1 {
            return Err(several());
        }
        let (mut images, inputs, opacity) =
            self.decode_surfaces_observing(level, |images, association, has_tail| {
                (retain_opacity && has_tail && association == DdsAlphaAssociation::Premultiplied)
                    .then(|| {
                        images
                            .iter()
                            .flat_map(|image| image.pixels().as_chunks::<4>().0)
                            .map(|pixel| pixel[3])
                            .collect()
                    })
            })?;
        match (images.pop(), images.is_empty()) {
            (Some(image), true) => Ok((image, inputs.selected(None).ok_or_else(several)?, opacity)),
            _ => Err(several()),
        }
    }

    /// Reads and decodes every image of mip `level`, a cubemap's six faces in DDS order, an
    /// array's elements or a volume's slices, to 8-bit RGBA with the alpha tail merged into the
    /// alpha channel.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] wrapping [`TextureError::UnsupportedDecode`] before reading
    /// any mip when the format of the texture or its alpha tail has no decoder, the error of
    /// opening the alpha tail when it is broken, [`TextureError::AlphaTailMips`] when the tail
    /// lacks the level, [`TextureError::AlphaTailSize`] when the tail does not fit the texture,
    /// and the errors of [`Texture::read_mip`]. A tail that cannot be used is an error, never
    /// silently dropped.
    pub fn decode_surfaces(&self, level: u32) -> Result<Vec<RgbaImage>, ConversionError> {
        self.decode_surfaces_with_inputs(level)
            .map(|(images, _)| images)
    }

    pub(crate) fn decode_surfaces_with_inputs(
        &self,
        level: u32,
    ) -> Result<(Vec<RgbaImage>, PreviewInputs<'a, MipPayloadLocation>), ConversionError> {
        self.decode_surfaces_observing(level, |_, _, _| ())
            .map(|(images, inputs, ())| (images, inputs))
    }

    fn decode_surfaces_observing<Observation>(
        &self,
        level: u32,
        observe: impl FnOnce(&[RgbaImage], DdsAlphaAssociation, bool) -> Observation,
    ) -> Result<
        (
            Vec<RgbaImage>,
            PreviewInputs<'a, MipPayloadLocation>,
            Observation,
        ),
        ConversionError,
    > {
        let alpha = self.checked_alpha_tail(level)?;
        let (read, color) = if let Parts::Files(parts) = self.parts() {
            let disk = disk_mip::read(parts, self.header_size(), level, self.name())?;
            let color = TexturePreviewInput::from_disk(
                disk.mip.raw_header,
                disk.mip.payload,
                disk.header,
                disk.part,
                DiskSelection {
                    raw: self.raw_header(),
                    input: self.disk_header.clone(),
                },
            );
            (disk.mip, color)
        } else {
            let read = self.read_mip_with_source(level)?;
            let color = TexturePreviewInput::of_read(self.parts(), read.raw_header, read.payload);
            (read, color)
        };
        let mut images = self.decode_mip_data(&read.mip)?;
        let observation = observe(
            &images,
            read.raw_header.alpha_association(),
            alpha.is_some(),
        );
        let alpha = Self::merge_alpha_input(alpha, level, &mut images)?;
        Ok((images, PreviewInputs { color, alpha }, observation))
    }

    /// Decodes every image of `mip`, read from this texture, to 8-bit RGBA without the alpha
    /// tail.
    pub(crate) fn decode_mip_data(&self, mip: &Mip) -> Result<Vec<RgbaImage>, ConversionError> {
        mip.decode_surfaces()
            .map_err(|error| ConversionError::of_texture(self.name(), error))
    }

    /// Checks that the texture and its alpha tail decode at `level` before any mip is read, and
    /// opens the tail.
    pub(crate) fn checked_alpha_tail(&self, level: u32) -> Result<Option<Self>, ConversionError> {
        self.check_decodable()?;
        let alpha = self.alpha_tail().transpose()?;
        if let Some(alpha) = &alpha {
            alpha.check_decodable()?;
            if level >= alpha.header().mip_count() && level < self.header().mip_count() {
                return Err(ConversionError::of_texture(
                    alpha.name(),
                    TextureError::AlphaTailMips {
                        level,
                        mip_count: alpha.header().mip_count(),
                    },
                ));
            }
        }
        Ok(alpha)
    }

    pub(crate) fn merge_alpha_input(
        alpha: Option<Self>,
        level: u32,
        images: &mut [RgbaImage],
    ) -> Result<Option<TexturePreviewInput<'a, MipPayloadLocation>>, ConversionError> {
        let Some(alpha) = alpha else {
            return Ok(None);
        };
        let (tails, inputs) = alpha.decode_surfaces_with_inputs(level)?;
        alpha.merge_decoded_alpha(level, images, &tails)?;
        Ok(Some(inputs.color))
    }

    fn merge_decoded_alpha(
        &self,
        level: u32,
        images: &mut [RgbaImage],
        tails: &[RgbaImage],
    ) -> Result<(), ConversionError> {
        if tails.len() != images.len() {
            return Err(ConversionError::of_texture(
                self.name(),
                TextureError::UnsupportedImage {
                    images: self.header().images(level),
                },
            ));
        }
        for (image, tail) in images.iter_mut().zip(tails) {
            merge_alpha(image, tail, self.header().format())
                .map_err(|error| ConversionError::of_texture(self.name(), error))?;
        }
        Ok(())
    }

    fn check_decodable(&self) -> Result<(), ConversionError> {
        let format = self.header().format();
        if format.is_decodable() {
            Ok(())
        } else {
            Err(ConversionError::of_texture(
                self.name(),
                TextureError::UnsupportedDecode(format),
            ))
        }
    }
}
