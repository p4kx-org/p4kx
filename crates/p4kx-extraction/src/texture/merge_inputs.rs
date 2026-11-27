// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use p4kx_texture::{MergeReport, MergeSpan, SiblingReader};

use crate::disk_parts::DiskParts;
use crate::observed_file_origin::ObservedFileStream;
use crate::output_file::read_error;
use crate::source::SourceEntry;
use crate::texture_parts::Parts;
use crate::{ConversionError, ObservedFileOrigin, SourceInput, Texture};

pub(super) enum Inputs<'texture> {
    Archive {
        header: SourceEntry<'texture>,
        parts: &'texture BTreeMap<u32, SourceEntry<'texture>>,
    },
    Files(FileMergeInputs),
}

impl Inputs<'_> {
    pub(super) fn header(&self) -> SourceInput<'_> {
        match self {
            Self::Archive { header, .. } => header
                .observed_entry()
                .map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            Self::Files(files) => file_input(files.header.as_ref()),
        }
    }

    pub(super) fn part(&self, index: u32, surface: u64) -> SourceInput<'_> {
        match self {
            Self::Archive { parts, .. } => parts
                .get(&index)
                .and_then(SourceEntry::observed_entry)
                .map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            Self::Files(files) => file_input(
                usize::try_from(surface)
                    .ok()
                    .and_then(|surface| files.parts.0.get(&index)?.get(surface)?.as_ref()),
            ),
        }
    }
}

pub(crate) struct FileMergeInputs {
    pub(crate) header: Option<ObservedFileOrigin>,
    pub(crate) parts: FilePartReads,
}

#[derive(Default)]
pub(crate) struct FilePartReads(BTreeMap<u32, Vec<Option<ObservedFileOrigin>>>);

impl FilePartReads {
    /// Associates a consumed sibling open with its actual surface slot.
    pub(crate) fn record(&mut self, index: u32, origin: Option<ObservedFileOrigin>) {
        self.0.entry(index).or_default().push(origin);
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
    let mut header = ObservedFileStream::new(path, file);
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
        Inputs::Files(FileMergeInputs {
            header: header.origin(),
            parts: siblings.reads.into_inner(),
        }),
    ))
}

struct ObservedSiblings<'parts> {
    parts: &'parts DiskParts,
    reads: RefCell<FilePartReads>,
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
            reader: ObservedFileStream::new(path, file),
            reads: &self.reads,
        })
    }
}

struct ObservedPart<'r> {
    index: u32,
    reader: ObservedFileStream<PathBuf>,
    reads: &'r RefCell<FilePartReads>,
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
            .record(self.index, self.reader.origin());
    }
}
