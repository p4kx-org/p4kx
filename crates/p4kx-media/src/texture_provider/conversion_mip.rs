// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::path::Path;

use p4kx_extraction::texture_codec::{self, LocatedMip, SiblingReader};
use p4kx_extraction::{ArchiveError, DdsHeader, ExtractionError, MAX_DDS_HEADER_LEN, TextureError};

use super::family_inputs::{FamilyInputs, InputRefusal, MipBranch};
use crate::origin_input::MediaOriginInput;
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaWriter, SourceContext, Subresource, error};

pub(super) struct Loaded {
    pub(super) mip: LocatedMip,
    pub(super) header_input: MediaOriginInput,
    pub(super) header_index: usize,
    pub(super) payload_input: Option<MediaOriginInput>,
}

#[derive(Debug)]
pub(super) enum ReadFailure {
    Dds(TextureError),
    Member {
        branch: MipBranch,
        number: Option<u32>,
        refusal: InputRefusal,
    },
}

pub(super) struct MipRead<'source, 'scope> {
    pub(super) context: &'source SourceContext<'scope>,
    pub(super) family: &'source FamilyInputs,
    pub(super) subresource: &'source Subresource,
    pub(super) level: u32,
}

impl MipRead<'_, '_> {
    pub(super) fn owner(
        &self,
        branch: MipBranch,
        count: u32,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Result<Loaded, ReadFailure>, MediaError> {
        let context = self.context;
        let reader = RequiredReader::open(
            context.payload,
            context.entry.path(),
            context.source.size,
            context.directory.input_state()?.budget,
            writer.checkpoint,
            writer.limits(),
        )?;
        let header = context.origin_input(writer)?;
        self.read(
            reader,
            HeaderInput {
                size: context.source.size,
                index: context.entry.index(),
                origin: header,
            },
            branch,
            count,
            writer,
        )
    }

    pub(super) fn alpha(
        &self,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Result<Option<Loaded>, ReadFailure>, MediaError> {
        let header = match self.family.alpha_header(self.context.directory) {
            Ok(header) => header,
            Err(InputRefusal::Missing) => return Ok(Ok(None)),
            Err(refusal) => {
                return Ok(Err(ReadFailure::Member {
                    branch: MipBranch::Alpha,
                    number: None,
                    refusal,
                }));
            }
        };
        let result = self.context.with_required_input(
            &header.token,
            self.subresource,
            writer,
            |input, writer| {
                let raw = match read_header(&mut *input) {
                    Ok(raw) => raw,
                    Err(cause) => {
                        return classify(cause, self.context.entry.path()).map(|e| e.map(Some));
                    }
                };
                let count = match raw.part_count(header.size) {
                    Ok(count) => count,
                    Err(cause) => return Ok(Err(ReadFailure::Dds(cause))),
                };
                io::Seek::rewind(input)
                    .map_err(|cause| error::io(Path::new(self.context.entry.path()), cause))?;
                let origin = input.origin_input(writer)?;
                self.read(
                    input,
                    HeaderInput {
                        size: header.size,
                        index: header.token.position,
                        origin,
                    },
                    MipBranch::Alpha,
                    count,
                    writer,
                )
                .map(|result| result.map(Some))
            },
        );
        required_result(result, MipBranch::Alpha, None)
    }

    fn read(
        &self,
        reader: impl Read,
        header: HeaderInput,
        branch: MipBranch,
        count: u32,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Result<Loaded, ReadFailure>, MediaError> {
        if self.level >= count {
            return self.located(reader, header, &NoSibling, None);
        }
        let number = count - self.level;
        let part = match self.family.part(branch, number, self.context.directory) {
            Ok(part) => part,
            Err(refusal) => {
                return Ok(Err(ReadFailure::Member {
                    branch,
                    number: Some(number),
                    refusal,
                }));
            }
        };
        let result = self.context.with_required_input(
            &part.token,
            self.subresource,
            writer,
            |input, writer| {
                let origin = input.origin_input(writer)?;
                let sibling = OneSibling {
                    number,
                    size: part.size,
                    reader: std::cell::RefCell::new(Some(input)),
                };
                self.located(reader, header, &sibling, Some(origin))
            },
        );
        required_result(result, branch, Some(number))
    }

    fn located(
        &self,
        reader: impl Read,
        header: HeaderInput,
        siblings: &impl SiblingReader,
        payload_input: Option<MediaOriginInput>,
    ) -> Result<Result<Loaded, ReadFailure>, MediaError> {
        match texture_codec::read_mip_with_location(reader, header.size, siblings, self.level) {
            Ok(mip) => Ok(Ok(Loaded {
                mip,
                header_input: header.origin,
                header_index: header.index,
                payload_input,
            })),
            Err(cause) => classify(cause, self.context.entry.path()),
        }
    }
}

struct HeaderInput {
    index: usize,
    size: u64,
    origin: MediaOriginInput,
}

struct NoSibling;

impl SiblingReader for NoSibling {
    type Part<'a> = io::Empty;

    fn part_len(&self, _index: u32) -> io::Result<Option<u64>> {
        Ok(None)
    }

    fn open_part(&self, _index: u32) -> io::Result<Self::Part<'_>> {
        Err(io::ErrorKind::NotFound.into())
    }
}

struct OneSibling<R> {
    number: u32,
    size: u64,
    reader: std::cell::RefCell<Option<R>>,
}

impl<R: Read> SiblingReader for OneSibling<R> {
    type Part<'a>
        = R
    where
        Self: 'a;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        Ok((self.number == index).then_some(self.size))
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        if self.number != index {
            return Err(io::ErrorKind::NotFound.into());
        }
        self.reader
            .borrow_mut()
            .take()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }
}

fn classify<T>(cause: TextureError, path: &str) -> Result<Result<T, ReadFailure>, MediaError> {
    match cause {
        TextureError::ReadHeader(source) | TextureError::ReadPart { source, .. } => {
            Err(error::io(Path::new(path), source))
        }
        other => Ok(Err(ReadFailure::Dds(other))),
    }
}
fn read_header(reader: impl Read) -> Result<DdsHeader, TextureError> {
    let mut prefix = Vec::new();
    reader
        .take(MAX_DDS_HEADER_LEN as u64)
        .read_to_end(&mut prefix)
        .map_err(TextureError::ReadHeader)?;
    DdsHeader::parse(&prefix)
}

fn required_result<T>(
    result: Result<Result<T, ReadFailure>, MediaError>,
    branch: MipBranch,
    number: Option<u32>,
) -> Result<Result<T, ReadFailure>, MediaError> {
    match result {
        Err(cause) => read_refusal(&cause).map_or_else(
            || Err(cause),
            |refusal| {
                Ok(Err(ReadFailure::Member {
                    branch,
                    number,
                    refusal,
                }))
            },
        ),
        success => success,
    }
}

fn read_refusal(cause: &MediaError) -> Option<InputRefusal> {
    match cause {
        MediaError::Source { cause, .. } => read_refusal(cause),
        MediaError::Extraction { cause, .. } => match cause.as_ref() {
            ExtractionError::Archive(cause) => Some(archive_refusal(cause)),
            ExtractionError::Read(_) | ExtractionError::ChecksumMismatch { .. } => {
                Some(InputRefusal::Unreadable)
            }
            _ => None,
        },
        _ => None,
    }
}

fn archive_refusal(cause: &ArchiveError) -> InputRefusal {
    if cause.is_key_required() {
        InputRefusal::KeyRequired
    } else if cause.is_wrong_key() {
        InputRefusal::WrongKey
    } else {
        InputRefusal::Unreadable
    }
}
