// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use p4kx_texture::{MergeReport, MergeSpan, SiblingReader};

use super::disk_mip::DiskRead;
use crate::disk_parts::DiskParts;
use crate::output_file::read_error;
use crate::source::SourceEntry;
use crate::texture_parts::Parts;
use crate::{ConversionError, ObservedFileOrigin, SourceInput, Texture};

pub(super) enum Inputs<'texture> {
    Archive {
        header: SourceEntry<'texture>,
        parts: &'texture BTreeMap<u32, SourceEntry<'texture>>,
    },
    Files {
        header: Option<ObservedFileOrigin>,
        parts: BTreeMap<u32, Vec<Option<ObservedFileOrigin>>>,
    },
}

impl Inputs<'_> {
    pub(super) fn header(&self) -> SourceInput<'_> {
        match self {
            Self::Archive { header, .. } => header
                .observed_entry()
                .map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            Self::Files { header, .. } => file_input(header.as_ref()),
        }
    }

    pub(super) fn part(&self, index: u32, surface: u64) -> SourceInput<'_> {
        match self {
            Self::Archive { parts, .. } => parts
                .get(&index)
                .and_then(SourceEntry::observed_entry)
                .map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            Self::Files { parts, .. } => file_input(
                usize::try_from(surface)
                    .ok()
                    .and_then(|surface| parts.get(&index)?.get(surface)?.as_ref()),
            ),
        }
    }
}

fn file_input(origin: Option<&ObservedFileOrigin>) -> SourceInput<'_> {
    origin.map_or(SourceInput::Unknown, SourceInput::File)
}

pub(super) struct WrittenMerge<'texture> {
    pub(super) report: MergeReport,
    pub(super) inputs: Inputs<'texture>,
    pub(super) spans: Vec<MergeSpan>,
}

pub(super) fn write<'texture>(
    texture: &'texture Texture<'texture>,
    writer: &mut dyn Write,
) -> Result<WrittenMerge<'texture>, ConversionError> {
    let mut spans = Vec::new();
    let mut observe = |span| {
        spans.try_reserve(1).map_err(io::Error::other)?;
        spans.push(span);
        Ok(())
    };
    let (report, inputs) = match texture.parts() {
        Parts::Source { header, parts, .. } => {
            let report = p4kx_texture::merge_with_spans(
                texture.open_header()?,
                texture.header_size(),
                texture.parts(),
                writer,
                &mut observe,
            )
            .map_err(|error| ConversionError::of_texture(texture.name(), error))?;
            (
                report,
                Inputs::Archive {
                    header: *header,
                    parts,
                },
            )
        }
        Parts::Files(parts) => write_disk(texture, parts, writer, &mut observe)?,
    };
    Ok(WrittenMerge {
        report,
        inputs,
        spans,
    })
}

fn write_disk<'texture>(
    texture: &Texture<'_>,
    parts: &DiskParts,
    writer: &mut dyn Write,
    observe: impl FnMut(MergeSpan) -> io::Result<()>,
) -> Result<(MergeReport, Inputs<'texture>), ConversionError> {
    let (path, file) = parts
        .open_with_path(None)
        .map_err(|error| read_error(parts.header(), error))?;
    let mut header = DiskRead::new(path, file);
    let siblings = ObservedSiblings {
        parts,
        reads: RefCell::default(),
    };
    let report = p4kx_texture::merge_with_spans(
        &mut header,
        texture.header_size(),
        &siblings,
        writer,
        observe,
    )
    .map_err(|error| ConversionError::of_texture(texture.name(), error))?;
    Ok((
        report,
        Inputs::Files {
            header: header.origin(),
            parts: siblings.reads.into_inner(),
        },
    ))
}

type Reads = RefCell<BTreeMap<u32, Vec<Option<ObservedFileOrigin>>>>;

struct ObservedSiblings<'parts> {
    parts: &'parts DiskParts,
    reads: Reads,
}

impl SiblingReader for ObservedSiblings<'_> {
    type Part<'r>
        = ObservedPart<'r>
    where
        Self: 'r;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        self.parts.part_len(index)
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        let (path, file) = self.parts.open_with_path(Some(index))?;
        Ok(ObservedPart {
            index,
            reader: DiskRead::new(path, file),
            reads: &self.reads,
        })
    }
}

struct ObservedPart<'r> {
    index: u32,
    reader: DiskRead,
    reads: &'r Reads,
}

impl Read for ObservedPart<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buffer)
    }
}

impl Drop for ObservedPart<'_> {
    fn drop(&mut self) {
        self.reads
            .borrow_mut()
            .entry(self.index)
            .or_default()
            .push(self.reader.origin());
    }
}
