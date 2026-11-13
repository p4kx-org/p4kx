// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_archive::{Archive, EntryReader};
use p4kx_texture::{
    DdsHeader, DdsRawHeader, LocatedMip, MAX_HEADER_LEN, MipByteSpans, SiblingReader, TextureError,
};
use thiserror::Error;

use super::{ArchiveTextures, MAX_MODEL_TEXTURE_PIXELS, archive_path, format_has_alpha};
use crate::texture_preview_png::input::{PreviewInputs, TexturePreviewInput};
use crate::{
    DdsSourceInfo, Entry, InputScope, PathIndex, PreviewPngEvidence, SourceEvidenceError,
    SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

/// Why the selected package DDS cannot be emitted directly.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ModelDdsRefusal {
    /// No physical candidate supplies a required member.
    #[error("a required DDS member is missing")]
    Missing,
    /// More than one physical member has the required path.
    #[error("a required DDS member has multiple candidates")]
    Ambiguous,
    /// The stored spelling of a candidate cannot establish its identity.
    #[error("a required DDS member has an unsupported stored path")]
    UnsupportedRawPath,
    /// The texture needs its second DDS to derive the alpha samples.
    #[error("the DDS uses a separate alpha or smoothness texture")]
    SplitAlpha,
    /// The material image slot accepts a single two-dimensional surface.
    #[error("the DDS has a cube, array or volume topology")]
    Topology,
    /// The selected mip exceeds the model image extent.
    #[error("the selected DDS mip exceeds the model texture pixel budget")]
    TooLarge,
    /// The archive could not open or completely verify an input.
    #[error("a required DDS member could not be read and verified")]
    Input(#[source] io::Error),
    /// Header parsing, mip selection or part validation failed.
    #[error(transparent)]
    Dds(#[from] TextureError),
}

/// A selected compressed mip and its exact original archive evidence.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedModelDds<'a> {
    /// Original mip data and the normalized DDS encoder.
    pub read: LocatedMip,
    /// Raw and normalized source header read from the verified archive member.
    pub metadata: DdsSourceInfo,
    /// Verified decoded checksum of the exact source header Entry.
    pub header_checksum: crate::Checksum,
    /// Verified decoded checksum of the exact selected split Entry, when separate.
    pub part_checksum: Option<crate::Checksum>,
    /// Whether the original pixel format contains an alpha channel.
    pub has_alpha: bool,
    inputs: PreviewInputs<'a, MipByteSpans>,
    archive: &'a Archive,
    header: &'a Entry,
    part: Option<&'a Entry>,
}

impl ObservedModelDds<'_> {
    /// Registers the directory checksum that the source read verified.
    ///
    /// # Errors
    /// Returns an evidence registration error.
    pub fn checksum_uses(
        &self,
        uses: &SourceUses,
    ) -> Result<(SourceUse, Option<SourceUse>), SourceEvidenceError> {
        let checksum = |entry: &Entry| {
            let observed = self.archive.observed_entry(entry);
            let scope = uses.observe(
                observed.map_or(SourceInput::Unknown, SourceInput::ArchiveView),
                InputScope::ArchiveDirectoryMetadata,
            )?;
            scope.at(
                SourceLocation::Metadata {
                    field: entry.checksum().name().into(),
                },
                SourceUseRole::Payload,
            )
        };
        Ok((checksum(self.header)?, self.part.map(checksum).transpose()?))
    }

    /// Registers the physical header and selected mip used by this asset.
    ///
    /// # Errors
    /// Returns an evidence registration error.
    pub fn evidence(&self, uses: &SourceUses) -> Result<PreviewPngEvidence, SourceEvidenceError> {
        self.inputs.register(uses, self.read.mip.level())
    }
}

impl<'a> ArchiveTextures<'_, 'a> {
    /// Reads one verified 2D DDS mip without decompressing its blocks into pixels.
    ///
    /// # Errors
    /// Returns physical input, split alpha, topology or mip errors.
    pub fn load_dds(&self, path: &str, mip: u32) -> Result<ObservedModelDds<'a>, ModelDdsRefusal> {
        let path = archive_path(path);
        let header = unique(self.index, &path)?.ok_or(ModelDdsRefusal::Missing)?;
        if unique(self.index, &format!("{path}.a"))?.is_some() {
            return Err(ModelDdsRefusal::SplitAlpha);
        }
        let (raw, normalized) = inspect(self.index, header)?;
        let level = single_image_level(&normalized, mip)?;
        let archive = self.index.archive();
        let size = header.uncompressed_size();
        let count = normalized.part_count(size)?;
        let part = if level < count {
            let ordinal = count - level;
            let entry = unique(self.index, &format!("{path}.{ordinal}"))?
                .ok_or(ModelDdsRefusal::Missing)?;
            Some((ordinal, entry))
        } else {
            None
        };
        let siblings = SelectedSibling { archive, part };
        let source = archive
            .open_entry_verified(header)
            .map_err(input)?
            .reject_overlong();
        let read = LocatedMip::read_complete(source, size, &siblings, level)?;
        if read.raw_header != raw {
            return Err(TextureError::InvalidHeader(
                "the DDS changed between header and mip reads",
            )
            .into());
        }
        let metadata = DdsSourceInfo::from_raw(&read.raw_header, size)?;
        let part = part.map(|(_, entry)| entry);
        let color = TexturePreviewInput::from_archive(
            read.raw_header,
            read.payload,
            archive.observed_entry(header),
            part.and_then(|entry| archive.observed_entry(entry)),
        );
        let inputs = PreviewInputs { color, alpha: None }.selected(None).ok_or(
            TextureError::InvalidHeader("the selected DDS mip has no source span"),
        )?;
        Ok(ObservedModelDds {
            has_alpha: format_has_alpha(normalized.format()),
            header_checksum: header.checksum(),
            part_checksum: part.map(Entry::checksum),
            read,
            metadata,
            inputs,
            archive,
            header,
            part,
        })
    }
}

fn single_image_level(header: &DdsHeader, mip: u32) -> Result<u32, ModelDdsRefusal> {
    if header.surfaces() != 1 || header.depth() != 1 {
        return Err(ModelDdsRefusal::Topology);
    }
    let level = mip.min(header.mip_count().saturating_sub(1));
    let extent = |side: u32| u64::from(side.checked_shr(level).unwrap_or(0).max(1));
    if extent(header.width()) * extent(header.height()) > MAX_MODEL_TEXTURE_PIXELS {
        return Err(ModelDdsRefusal::TooLarge);
    }
    Ok(level)
}

struct SelectedSibling<'a> {
    archive: &'a Archive,
    part: Option<(u32, &'a Entry)>,
}

impl<'a> SelectedSibling<'a> {
    fn entry(&self, index: u32) -> Option<&'a Entry> {
        let (ordinal, entry) = self.part?;
        (ordinal == index).then_some(entry)
    }
}

impl SiblingReader for SelectedSibling<'_> {
    type Part<'a>
        = EntryReader<'a>
    where
        Self: 'a;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        Ok(self.entry(index).map(Entry::uncompressed_size))
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        let entry = self.entry(index).ok_or(io::ErrorKind::NotFound)?;
        self.archive
            .open_entry_verified(entry)
            .map(EntryReader::reject_overlong)
            .map_err(io::Error::other)
    }
}

fn unique<'a>(index: &PathIndex<'a>, path: &str) -> Result<Option<&'a Entry>, ModelDdsRefusal> {
    let mut found = index
        .matching_entries_with(path, |_| Ok::<_, io::Error>(()))
        .map_err(input)?;
    let first = found.next().transpose().map_err(input)?;
    if found.next().transpose().map_err(input)?.is_some() {
        return Err(ModelDdsRefusal::Ambiguous);
    }
    if first.is_some_and(|entry| index.archive().raw_path(entry.index()).is_some()) {
        return Err(ModelDdsRefusal::UnsupportedRawPath);
    }
    Ok(first)
}

fn inspect(
    index: &PathIndex<'_>,
    entry: &Entry,
) -> Result<(DdsRawHeader, DdsHeader), ModelDdsRefusal> {
    let mut prefix = Vec::with_capacity(MAX_HEADER_LEN);
    index
        .archive()
        .open_entry(entry)
        .map_err(input)?
        .take(MAX_HEADER_LEN as u64)
        .read_to_end(&mut prefix)
        .map_err(ModelDdsRefusal::Input)?;
    let raw = DdsRawHeader::parse(&prefix)?;
    Ok((raw, DdsHeader::from_raw(&raw)?))
}

fn input(error: impl std::error::Error + Send + Sync + 'static) -> ModelDdsRefusal {
    ModelDdsRefusal::Input(io::Error::other(error))
}
