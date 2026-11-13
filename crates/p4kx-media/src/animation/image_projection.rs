// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

#[path = "caf_image_bank.rs"]
mod caf_image_bank;
#[path = "caf_image_clip.rs"]
mod caf_image_clip;
#[path = "caf_image_hashes.rs"]
mod caf_image_hashes;
#[path = "caf_image_record.rs"]
mod caf_image_record;
#[path = "directional_projection.rs"]
mod directional_projection;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ChunkFile, Container};
use p4kx_extraction::model::{CafImage, CafImageError, CafImagePath};

use super::directional_paths::DirectionalPaths;
use super::image_payload::{ImageIndex, ImageLayout, ImagePath};
use super::origin::{self as proof, AnimationOrigins};
use super::payload::{self as wire, AnimationPayload, Family};
use crate::animation::selection_path::{ReferenceSelection, archive_animation_path};
use crate::directory_lookup::animation_banks::AnimationBanks;
use crate::origin_contributions::OriginContributions;
use crate::{
    MediaError, MediaIssue, MediaKind, MediaWriter, Severity, SourceContext, Status, Subresource,
};
use std::cell::OnceCell;

const CAF_IMAGE_TYPE: u32 = 0x3702_75B7;
const DIRECTIONAL_IMAGE_TYPE: u32 = 0x6B8D_F259;
const CAF_IMAGE_VERSION: u32 = 0x902;
const DIRECTIONAL_IMAGE_VERSION: u32 = 0x901;
const FIELD: &str = "/data/payload/";

pub(super) fn write(
    file: &ChunkFile<Vec<u8>>,
    family: Family,
    context: &SourceContext<'_>,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if file.container() != Container::Ivo {
        return Ok(());
    }
    let expected_type = match family {
        Family::CafImage => CAF_IMAGE_TYPE,
        Family::DirectionalBlendImage => DIRECTIONAL_IMAGE_TYPE,
        Family::Caf | Family::Dba | Family::Aim => return Ok(()),
    };
    let banks = OnceCell::new();
    for (index, chunk) in file.chunks().iter().enumerate() {
        writer.checkpoint()?;
        if chunk.chunk_type().id() != expected_type {
            continue;
        }
        let range = chunk.offset()..chunk.offset() + chunk.size();
        let bytes = file.chunk_data(index).ok_or_else(|| {
            MediaError::Contract("validated image chunk has no retained bytes".into())
        })?;
        let source = ImageSource {
            context,
            origins,
            banks: &banks,
        };
        let layout = source.classify(expected_type, chunk.version(), range.start, bytes, writer)?;
        source.write_index(file, index, range.clone(), &layout, writer)?;
        match layout {
            Layout::Caf(image) => {
                let mut metadata = caf_image_clip::DbaMetadataCache::default();
                for (ordinal, record) in image.records().enumerate() {
                    source.write_record(
                        [index as u64, ordinal as u64],
                        record,
                        &mut metadata,
                        writer,
                    )?;
                }
            }
            Layout::DirectionalPaths { paths, .. } => {
                let mut ordinal = 0;
                paths.visit(context, writer, |path, writer| {
                    source.write_directional([index as u64, ordinal], range.start, path, writer)?;
                    ordinal += 1;
                    Ok(())
                })?;
            }
            _ => {}
        }
    }
    Ok(())
}

enum Layout<'a> {
    Caf(CafImage<'a>),
    Directional,
    DirectionalPaths {
        paths: DirectionalPaths<'a>,
        count: usize,
    },
    UnsupportedVersion,
    UnsupportedFormat,
    InvalidFraming,
}

impl Layout<'_> {
    const fn payload(&self) -> ImageLayout {
        match self {
            Self::Caf(image) => ImageLayout::CafRecords {
                header: wire::entry_range(image.header_range()),
                record_count: image.record_count(),
                trailer: wire::entry_range(image.trailer_range()),
            },
            Self::Directional => ImageLayout::DirectionalOpaque,
            Self::DirectionalPaths { count, .. } => ImageLayout::DirectionalPaths {
                declared_paths: *count,
            },
            Self::UnsupportedVersion => ImageLayout::UnsupportedVersion,
            Self::UnsupportedFormat => ImageLayout::UnsupportedFormat,
            Self::InvalidFraming => ImageLayout::InvalidFraming,
        }
    }
    fn issue(&self) -> Option<MediaIssue> {
        let (code, detail) = match self {
            Self::Caf(_) | Self::Directional | Self::DirectionalPaths { .. } => return None,
            Self::UnsupportedVersion => (
                "animation.image-version",
                "Unknown animation image chunk version",
            ),
            Self::UnsupportedFormat => {
                ("animation.image-format", "Unknown CAF image prefix format")
            }
            Self::InvalidFraming => (
                "animation.image-framing",
                "CAF image extent does not match its counted table and trailer",
            ),
        };
        Some(MediaIssue::new(
            code,
            "image-index",
            Severity::Warning,
            detail,
        ))
    }
}

pub(super) struct ImageSource<'a, 'source> {
    context: &'a SourceContext<'source>,
    banks: &'a OnceCell<AnimationBanks>,
    pub(super) origins: &'a AnimationOrigins<'a>,
}

impl ImageSource<'_, '_> {
    fn classify<'a>(
        &self,
        kind: u32,
        version: u32,
        start: u64,
        bytes: &'a [u8],
        writer: &mut MediaWriter<'_>,
    ) -> Result<Layout<'a>, MediaError> {
        match (kind, version) {
            (CAF_IMAGE_TYPE, CAF_IMAGE_VERSION) => Ok(match CafImage::read(bytes, start) {
                Ok(image) => Layout::Caf(image),
                Err(CafImageError::UnsupportedFormat(_)) => Layout::UnsupportedFormat,
                Err(_) => Layout::InvalidFraming,
            }),
            (DIRECTIONAL_IMAGE_TYPE, DIRECTIONAL_IMAGE_VERSION) => {
                self.directional(bytes, start, writer)
            }
            _ => Ok(Layout::UnsupportedVersion),
        }
    }

    fn directional<'a>(
        &self,
        bytes: &'a [u8],
        start: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Layout<'a>, MediaError> {
        let Some(paths) = DirectionalPaths::new(bytes, start) else {
            return Ok(Layout::Directional);
        };
        let Some(count) = paths.declared() else {
            return Ok(Layout::Directional);
        };
        if count == 0 || !paths.begins_with_path() {
            return Ok(Layout::Directional);
        }
        let mut observed = 0;
        paths.visit(self.context, writer, |_, _| {
            observed += 1;
            Ok(())
        })?;
        if observed == count {
            Ok(Layout::DirectionalPaths { paths, count })
        } else {
            Ok(Layout::Directional)
        }
    }

    fn write_index(
        &self,
        file: &ChunkFile<Vec<u8>>,
        index: usize,
        range: Range<u64>,
        layout: &Layout<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.origins.prepared()?;
        let table = self.origins.payload(
            file.table_range(),
            "imageChunkTable",
            SourceUseRole::Selector,
            writer,
        )?;
        proof::control(&mut prepared, table.clone(), writer)?;
        proof::at(&mut prepared, "/data/payload/range", table.clone(), writer)?;
        let layout_proof = self.layout_proof(&range, layout, &table, writer)?;
        proof::at(
            &mut prepared,
            "/data/payload/layout",
            layout_proof.clone(),
            writer,
        )?;
        proof::at(
            &mut prepared,
            "/data/payload/range/basis",
            self.origins.independent(),
            writer,
        )?;
        if matches!(layout, Layout::Caf(_)) {
            for field in ["layout/header/basis", "layout/trailer/basis"] {
                proof::at(
                    &mut prepared,
                    &format!("{FIELD}{field}"),
                    self.origins.independent(),
                    writer,
                )?;
            }
        }
        let issue = layout.issue();
        if issue.is_some() {
            proof::at(&mut prepared, "/issues", layout_proof, writer)?;
        }
        let status = if issue.is_some() {
            Status::Partial
        } else {
            Status::Complete
        };
        prepared.write(
            writer,
            &Subresource::new(wire::IMAGE_INDEX, vec![index as u64]),
            status,
            &AnimationPayload::ImageIndex(ImageIndex {
                range: wire::entry_range(range),
                layout: layout.payload(),
            }),
            issue.as_slice(),
        )
    }

    fn layout_proof(
        &self,
        range: &Range<u64>,
        layout: &Layout<'_>,
        table: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut layout_proof = writer.capture_origins();
        layout_proof.include_original(table)?;
        if matches!(
            layout,
            Layout::DirectionalPaths { .. } | Layout::Directional
        ) {
            layout_proof.include_original(&self.origins.payload(
                range.clone(),
                "directionalPathScan",
                SourceUseRole::Selector,
                writer,
            )?)?;
        } else if !matches!(layout, Layout::Directional | Layout::UnsupportedVersion) {
            layout_proof.include_original(&self.origins.payload(
                range.start..range.end.min(range.start.saturating_add(8)),
                "imagePrefix",
                SourceUseRole::Selector,
                writer,
            )?)?;
        }
        Ok(layout_proof.finish())
    }

    pub(super) fn path<'a>(
        &self,
        path: CafImagePath<'a>,
        expected_extension: &str,
        path_proof: &OriginContributions,
        prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ImagePath<'a>, MediaError> {
        let authored = match path {
            CafImagePath::Text(authored) => authored,
            CafImagePath::Unterminated => return Ok(ImagePath::Unterminated),
            _ => return Ok(ImagePath::InvalidEncoding),
        };
        let archive_path = authored
            .rsplit_once('.')
            .filter(|(_, extension)| extension.eq_ignore_ascii_case(expected_extension))
            .and_then(|_| archive_animation_path(authored));
        let reference = ReferenceSelection {
            authored_path: authored,
            archive_path: archive_path.as_deref(),
            kind: Some(MediaKind::Animation),
            original: path_proof,
            selector: path_proof,
        }
        .resolve_at(
            self.context.directory,
            "/data/payload/path/loose_entry",
            prepared,
            writer,
        )?;
        Ok(ImagePath::Valid {
            authored,
            archive_path,
            loose_entry: Box::new(reference),
        })
    }
}
