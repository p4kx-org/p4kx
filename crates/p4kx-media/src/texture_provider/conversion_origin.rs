// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_codec::{DdsAlphaTransform, DdsPngEncoding};
use p4kx_extraction::{DdsHeaderField, InputScope, SourceUseRole};

use super::conversion_mip::Loaded;
use super::origin::{self, TextureHeaderOrigins};
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter};

pub(super) struct ImageOrigins {
    width: OriginContributions,
    height: OriginContributions,
    depth: OriginContributions,
    pub(super) shape: OriginContributions,
    format: OriginContributions,
    channels: OriginContributions,
    transfer: TransferOrigins,
    alpha: AlphaOrigins,
    bytes: OriginContributions,
}

impl ImageOrigins {
    pub(super) fn of(
        color: &Loaded,
        alpha: Option<&Loaded>,
        directory: Option<&OriginContributions>,
        index: u64,
        png: Option<DdsPngEncoding>,
        (alpha_header, has_coverage, transform): (bool, bool, DdsAlphaTransform),
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Width};

        let header = color.origins();
        let raw = &color.mip.raw_header;
        let width = header.fields(raw, &[Width], writer)?.finish();
        let height = header.fields(raw, &[Height], writer)?.finish();
        let depth = header.fields(raw, &[Depth], writer)?.finish();
        let shape = header
            .fields(
                raw,
                &[Width, Height, Depth, MipCount, Faces, ArraySize],
                writer,
            )?
            .finish();
        let format = header.fields(raw, &[Format], writer)?.finish();
        let mut channels = writer.capture_origins();
        channels.include(&format, writer)?;
        if let Some(directory) = directory {
            channels.include(directory, writer)?;
        }
        if let Some(alpha) = alpha {
            channels.include(
                &alpha
                    .origins()
                    .fields(&alpha.mip.raw_header, &[Format], writer)?
                    .finish(),
                writer,
            )?;
        }
        let channels = channels.finish();
        let transfer = TransferOrigins::of(color, &format, png, alpha_header, writer)?;
        let pixels = color.pixels(index, writer)?;
        let alpha_origins = AlphaOrigins::of(
            color,
            &transfer,
            has_coverage.then_some(&pixels),
            transform,
            writer,
        )?;
        let mut bytes = writer.capture_origins();
        bytes.include(&transfer.output, writer)?;
        bytes.include(&channels, writer)?;
        bytes.include(&pixels, writer)?;
        if let Some(transform) = &alpha_origins.transform {
            bytes.include(transform, writer)?;
        }
        if let Some(alpha) = alpha {
            bytes.include(&alpha.pixels(index, writer)?, writer)?;
        }
        Ok(Self {
            width,
            height,
            depth,
            shape,
            format,
            channels,
            transfer,
            alpha: alpha_origins,
            bytes: bytes.finish(),
        })
    }

    pub(super) fn manifest(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut origins = writer.record_origins();
        self.artifact_fields(&mut origins, "", writer)?;
        Ok(origins)
    }

    pub(super) fn record(
        &self,
        input: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut origins = prepared(input)?;
        let independent = OriginContributions::independent(&self.bytes.owner);
        for (field, source) in [
            ("level", &independent),
            ("placement", &self.bytes),
            ("layer", &self.shape),
            ("face", &self.shape),
            ("slice", &self.shape),
            ("source_width", &self.width),
            ("source_height", &self.height),
            ("source_depth", &self.depth),
            ("width", &self.width),
            ("height", &self.height),
            ("format", &self.format),
            ("bit_depth", &self.format),
            ("suffix", &self.shape),
            ("alpha", &self.channels),
            ("channel_transform", &self.format),
            ("source_transfer", &self.transfer.source),
            ("output_transfer", &self.transfer.output),
        ] {
            origin::put(
                &mut origins,
                &format!("/data/payload/{field}"),
                source.clone(),
                writer,
            )?;
        }
        origin::put(
            &mut origins,
            "/subresource/indices",
            self.shape.clone(),
            writer,
        )?;
        if let Some(transform) = &self.transfer.png_transform {
            origin::put(
                &mut origins,
                "/data/payload/png_transform",
                transform.clone(),
                writer,
            )?;
        }
        self.alpha.fields(&mut origins, writer)?;
        self.artifact_fields(&mut origins, "/data/payload/artifact", writer)?;
        Ok(origins)
    }

    fn artifact_fields(
        &self,
        origins: &mut PreparedRecordOrigins,
        base: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let independent = OriginContributions::independent(&self.bytes.owner);
        for (field, source) in [
            ("path", &self.bytes),
            ("size", &self.bytes),
            ("digest/hex", &self.bytes),
            ("digest/algorithm", &independent),
            ("digest/scope", &independent),
        ] {
            origin::put(origins, &format!("{base}/{field}"), source.clone(), writer)?;
        }
        Ok(())
    }
}

struct AlphaOrigins {
    association: OriginContributions,
    coverage: Option<OriginContributions>,
    transform: Option<OriginContributions>,
}

impl AlphaOrigins {
    fn of(
        color: &Loaded,
        transfer: &TransferOrigins,
        coverage: Option<&OriginContributions>,
        transform: DdsAlphaTransform,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let association = color
            .origins()
            .fields(
                &color.mip.raw_header,
                &[DdsHeaderField::AlphaAssociation],
                writer,
            )?
            .finish();
        let transform = if transform == DdsAlphaTransform::Preserve {
            None
        } else {
            let mut contributors = writer.capture_origins();
            contributors.include(&association, writer)?;
            contributors.include(&transfer.output, writer)?;
            Some(contributors.finish())
        };
        Ok(Self {
            association,
            coverage: coverage.cloned(),
            transform,
        })
    }

    fn fields(
        &self,
        origins: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        origin::put(
            origins,
            "/data/payload/alpha_association",
            self.association.clone(),
            writer,
        )?;
        if let Some(coverage) = &self.coverage {
            for name in ["zero", "partial", "full"] {
                origin::put(
                    origins,
                    &format!("/data/payload/alpha_coverage/{name}"),
                    coverage.clone(),
                    writer,
                )?;
            }
        }
        if let Some(transform) = &self.transform {
            origin::put(
                origins,
                "/data/payload/alpha_transform",
                transform.clone(),
                writer,
            )?;
        }
        Ok(())
    }
}

struct TransferOrigins {
    source: OriginContributions,
    output: OriginContributions,
    png_transform: Option<OriginContributions>,
}

impl TransferOrigins {
    fn of(
        color: &Loaded,
        format: &OriginContributions,
        png: Option<DdsPngEncoding>,
        alpha_header: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        use DdsHeaderField::{CryTransfer, DxgiTransfer};

        let header = color.origins();
        let raw = &color.mip.raw_header;
        let source = header
            .fields(raw, &[DxgiTransfer, CryTransfer], writer)?
            .finish();
        let Some(encoding) = png else {
            return Ok(Self {
                source,
                output: format.clone(),
                png_transform: None,
            });
        };
        let mut output = header.fields(raw, encoding.output_transfer_fields(), writer)?;
        let mut transform = writer.capture_origins();
        transform.include(format, writer)?;
        if alpha_header {
            let name = header.entry_field("path", SourceUseRole::Selector, writer)?;
            output.include(&name, writer)?;
            transform.include(&name, writer)?;
        }
        Ok(Self {
            source,
            output: output.finish(),
            png_transform: Some(transform.finish()),
        })
    }
}

impl Loaded {
    const fn origins(&self) -> TextureHeaderOrigins<'_> {
        TextureHeaderOrigins::new(
            &self.header_input,
            self.header_index,
            self.mip.raw_header.header_len() as u64,
        )
    }

    fn pixels(
        &self,
        index: u64,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let spans =
            self.mip.payload.spans(Some(index)).ok_or_else(|| {
                MediaError::Contract("decoded DDS image has no original span".into())
            })?;
        let level = self.mip.mip.level();
        let raw = &self.mip.raw_header;
        let mut capture = writer.capture_origins();
        capture.include(&self.origins().split_layout(raw, writer)?, writer)?;
        let indices = [
            u64::from(level),
            spans.offset,
            spans.length,
            spans.stride,
            spans.count,
        ];
        capture.add(
            self.payload_input.as_ref().unwrap_or(&self.header_input),
            InputScope::Payload,
            OriginSite::new("ddsMipPayload", &indices, "bytes"),
            SourceUseRole::Conversion,
            writer,
        )?;
        Ok(capture.finish())
    }
}

pub(super) fn prepared(input: &MediaOriginInput) -> Result<PreparedRecordOrigins, MediaError> {
    let mut origins = PreparedRecordOrigins::new(&input.owner);
    origins.set_builtin(BuiltinOrigins {
        profile: OriginProfile::TextureConversion,
        input: input.clone(),
        scope: InputScope::Payload,
    })?;
    Ok(origins)
}
