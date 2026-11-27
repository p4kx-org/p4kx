// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_codec::{
    DdsAlphaAssociation, DdsAlphaCoverage, DdsAlphaTransform, DdsPngEncoding, DdsTransfer,
};
use p4kx_extraction::{
    CubemapLayout, DdsHeader, DdsRawHeader, DecimalU64, DxgiFormat, TextureError, TextureFormat,
    TextureImage, TextureImageFormat, TextureImagePlan,
};

use super::conversion_mip::{Loaded, MipRead, ReadFailure};
use super::conversion_origin::{self, ImageOrigins};
use super::conversion_payload::{
    Alpha, CONVERSION, ChannelTransform, Conversion, ConversionPayload, Failure, IMAGE, Image,
    ImageFormat, Outcome, Placement,
};
use super::family_inputs::{FamilyInputs, MipBranch};

use super::header::HeaderRole;
use super::origin::{self, TextureHeaderOrigins};
use crate::origin_contributions::OriginContributions;
use crate::{
    ArtifactExtension, MediaError, MediaIssue, MediaWriter, Severity, SourceContext, Status,
    Subresource,
};
#[path = "conversion_pixels.rs"]
mod conversion_pixels;

use conversion_pixels::Pixels;

pub(super) struct Request<'a> {
    pub(super) level: u32,
    pub(super) role: HeaderRole<'a>,
    pub(super) family: FamilyInputs,
}

impl Request<'_> {
    pub(super) fn run(
        &self,
        context: &SourceContext<'_>,
        header: Option<(&DdsRawHeader, &DdsHeader, Option<u32>)>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let subresource = Subresource::new(CONVERSION, vec![u64::from(self.level)]);
        let result = self.convert(
            context,
            header.map(|(_, header, count)| (header, count)),
            &subresource,
            writer,
        )?;
        let mut origins = conversion_origin::prepared(&context.origin_input(writer)?)?;
        let independent = OriginContributions::independent(&writer.origins.owner);
        origin::put(
            &mut origins,
            "/data/payload/requested_level",
            independent.clone(),
            writer,
        )?;
        origin::put(&mut origins, "/subresource/indices", independent, writer)?;
        let (outcome, status, issue) = match result {
            Ok((images, evidence)) => {
                origin::put(&mut origins, "/data/payload/outcome", evidence, writer)?;
                (Outcome::Written { images }, Status::Complete, None)
            }
            Err(failure) => {
                let evidence =
                    self.failure_origin(context, header.map(|(raw, _, _)| raw), &failure, writer)?;
                origin::put(
                    &mut origins,
                    "/data/payload/outcome",
                    evidence.clone(),
                    writer,
                )?;
                origin::put(&mut origins, "/issues", evidence, writer)?;
                let issue = MediaIssue::new(
                    "texture.conversion.unavailable",
                    "requested_level",
                    Severity::Error,
                    "The requested DDS mip could not be converted.",
                );
                (Outcome::Failed { failure }, Status::Partial, Some(issue))
            }
        };
        origins.write(
            writer,
            &subresource,
            status,
            &ConversionPayload::Conversion(Conversion {
                requested_level: self.level,
                outcome,
            }),
            issue.as_slice(),
        )
    }

    fn failure_origin(
        &self,
        context: &SourceContext<'_>,
        raw: Option<&DdsRawHeader>,
        failure: &Failure,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut evidence = writer.capture_origins();
        if let Some(raw) = raw {
            let input = context.origin_input(writer)?;
            let header =
                TextureHeaderOrigins::new(&input, context.entry.index(), raw.header_len() as u64);
            evidence.include(&header.split_layout(raw, writer)?, writer)?;
        } else {
            evidence.include(&OriginContributions::unknown(&writer.origins.owner), writer)?;
        }
        if matches!(failure, Failure::Member { .. }) {
            evidence.include(self.family.evidence(), writer)?;
        }
        if matches!(
            failure,
            Failure::InvalidHeader | Failure::Decode | Failure::InconsistentLayout
        ) {
            evidence.include(&OriginContributions::unknown(&writer.origins.owner), writer)?;
        }
        Ok(evidence.finish())
    }

    fn convert(
        &self,
        context: &SourceContext<'_>,
        header: Option<(&DdsHeader, Option<u32>)>,
        subresource: &Subresource,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Result<(DecimalU64, OriginContributions), Failure>, MediaError> {
        let Some((header, count)) = header else {
            return Ok(Err(Failure::InvalidHeader));
        };
        let Some(count) = count else {
            return Ok(Err(if header.format().is_decodable() {
                Failure::InconsistentLayout
            } else {
                Failure::UnsupportedDecoder
            }));
        };
        if !header.format().is_decodable() {
            return Ok(Err(Failure::UnsupportedDecoder));
        }
        let format = TextureImageFormat::for_header(header);
        let plan = match TextureImagePlan::new(header, self.level, CubemapLayout::Faces, format) {
            Ok(plan) => plan,
            Err(cause) => return Ok(Err(dds_failure(&cause))),
        };
        let reader = MipRead {
            context,
            family: &self.family,
            subresource,
            level: self.level,
        };
        let branch = if self.role.alpha {
            MipBranch::Alpha
        } else {
            MipBranch::Color
        };
        let color = match reader.owner(branch, count, writer)? {
            Ok(color) => color,
            Err(failure) => return Ok(Err(read_failure(failure))),
        };
        let alpha = if format == TextureImageFormat::Png && !self.role.alpha {
            match reader.alpha(writer)? {
                Ok(alpha) => alpha,
                Err(failure) => return Ok(Err(read_failure(failure))),
            }
        } else {
            None
        };
        self.write_images(context, &color, alpha.as_ref(), plan, writer)
    }

    fn write_images(
        &self,
        context: &SourceContext<'_>,
        color: &Loaded,
        alpha: Option<&Loaded>,
        plan: TextureImagePlan,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Result<(DecimalU64, OriginContributions), Failure>, MediaError> {
        let mut all = writer.capture_origins();
        let directory = self.family.evidence();
        let count = DecimalU64::new(plan.len() as u64);
        let source = color.mip.mip.header();
        let source_transfer = color.mip.raw_header.transfer().effective;
        let png = if self.role.alpha {
            DdsPngEncoding::data_channels()
        } else {
            DdsPngEncoding::of(source.format(), source_transfer)
        };
        for (position, image) in plan.enumerate() {
            writer.checkpoint()?;
            let index = position as u64;
            let png = (image.format == TextureImageFormat::Png).then_some(png);
            let alpha_association = color.mip.raw_header.alpha_association();
            let transform = png.map_or(DdsAlphaTransform::Preserve, |encoding| {
                DdsAlphaTransform::for_png(alpha_association, encoding)
            });
            let decoded = match Pixels::decode(color, alpha, &image, index, png, transform) {
                Ok(decoded) => decoded,
                Err(cause) => return Ok(Err(dds_failure(&cause))),
            };
            let uses_family = color.payload_input.is_some()
                || image.format == TextureImageFormat::Png && !self.role.alpha;
            let coverage = color.mip.mip.alpha_coverage(index);
            let origins = ImageOrigins::of(
                color,
                alpha,
                uses_family.then_some(directory),
                index,
                png,
                (self.role.alpha, coverage.is_some(), transform),
                writer,
            )?;
            let alpha = decoded.alpha(alpha.is_some());
            let subresource = Subresource::new(IMAGE, vec![u64::from(self.level), index]);
            all.include(&origins.shape, writer)?;
            ImageWrite {
                image,
                source,
                decoded,
                source_transfer,
                png,
                alpha,
                alpha_association,
                coverage,
                transform,
            }
            .publish(context, &color.header_input, &subresource, &origins, writer)?;
        }
        Ok(Ok((count, all.finish())))
    }
}

struct ImageWrite<'a> {
    image: TextureImage,
    source: &'a DdsHeader,
    decoded: Pixels,
    source_transfer: DdsTransfer,
    png: Option<DdsPngEncoding>,
    alpha: Alpha,
    alpha_association: DdsAlphaAssociation,
    coverage: Option<DdsAlphaCoverage>,
    transform: DdsAlphaTransform,
}

impl ImageWrite<'_> {
    fn publish(
        self,
        context: &SourceContext<'_>,
        input: &crate::origin_input::MediaOriginInput,
        subresource: &Subresource,
        origins: &ImageOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let image = self.image;
        let transfer = self.png.map_or(DdsTransfer::Linear, |png| png.transfer);
        let occurrence = writer.converted_occurrence_with_origins(
            ArtifactExtension::new(image.format.extension())?,
            &origins.manifest(writer)?,
            |sink| {
                self.decoded.encode(transfer, sink).map_err(|cause| {
                    let cause = match cause {
                        TextureError::Write(source) => source,
                        other => std::io::Error::other(other),
                    };
                    crate::error::io(std::path::Path::new(context.entry.path()), cause)
                })
            },
        )?;
        origins.record(input, writer)?.write(
            writer,
            subresource,
            Status::Complete,
            &ConversionPayload::Image(Image {
                level: image.level,
                layer: image.layer,
                face: image.face,
                slice: image.slice,
                source_width: self.source.width(),
                source_height: self.source.height(),
                source_depth: self.source.depth(),
                width: image.width,
                height: image.height,
                format: ImageFormat::of(image.format),
                bit_depth: image.format.bit_depth(),
                suffix: image.suffix,
                alpha: self.alpha,
                alpha_association: self.alpha_association,
                alpha_coverage: self.coverage,
                alpha_transform: (self.transform != DdsAlphaTransform::Preserve)
                    .then_some(self.transform),
                channel_transform: channel_transform(image.format, self.source.format()),
                source_transfer: Some(self.source_transfer),
                output_transfer: Some(transfer),
                png_transform: self.png.map(|png| png.transform),
                artifact: occurrence.artifact,
                placement: if occurrence.created {
                    Placement::Written
                } else {
                    Placement::KeptIdentical
                },
            }),
            &[],
        )
    }
}
fn channel_transform(image: TextureImageFormat, source: TextureFormat) -> ChannelTransform {
    if image == TextureImageFormat::Exr {
        return ChannelTransform::LinearFloat;
    }
    match source {
        TextureFormat::Dxgi(DxgiFormat::BC5_UNORM | DxgiFormat::BC5_SNORM) => {
            ChannelTransform::ReconstructedBc5Normal
        }
        _ => ChannelTransform::Rgba8Decode,
    }
}

fn read_failure(failure: ReadFailure) -> Failure {
    match failure {
        ReadFailure::Dds(cause) => dds_failure(&cause),
        ReadFailure::Member {
            branch,
            number,
            refusal,
        } => Failure::Member {
            branch: branch.into(),
            part: number,
            disposition: refusal.into(),
        },
    }
}

const fn dds_failure(cause: &TextureError) -> Failure {
    match cause {
        TextureError::NotDds
        | TextureError::TruncatedHeader { .. }
        | TextureError::InvalidHeader(_) => Failure::InvalidHeader,
        TextureError::UnsupportedDecode(_)
        | TextureError::UnsupportedFormat(_)
        | TextureError::NotHighRange(_) => Failure::UnsupportedDecoder,
        TextureError::NoSuchMip { .. } | TextureError::AlphaTailMips { .. } => {
            Failure::UnavailableMip
        }
        TextureError::TailMismatch { .. }
        | TextureError::TruncatedPart { .. }
        | TextureError::OversizedPart { .. }
        | TextureError::UnknownTrailer { .. }
        | TextureError::AlphaTailSize { .. } => Failure::InconsistentLayout,
        _ => Failure::Decode,
    }
}
